//! rws's HTTP/2 server with `GrpcProxy` / `H2ReverseProxy` in front of a gRPC
//! echo backend (the `h2` crate's server, gRPC framing by hand): trailers,
//! streaming in both directions, trailers-only errors, pooled connections.
//! The same against tonic is `ci/e2e/grpc.sh`.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;

use crate::app::App;
use crate::core::New;
use crate::proxy::{GrpcProxy, H2ReverseProxy};

/// A length-prefixed gRPC message (uncompressed).
fn message(payload: &[u8]) -> Bytes {
    let mut framed = vec![0u8];
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
    Bytes::from(framed)
}

/// `/test.Echo/Fail` → trailers-only `grpc-status: 13`; `/test.Echo/Plain`
/// → a plain 5-byte text body; `/test.Echo/ServerStream` → three messages;
/// anything else echoes each request DATA frame as it arrives (unary and
/// bidirectional calls), then `grpc-status: 0` and whether `te: trailers`
/// arrived.
async fn grpc_backend() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut connection = h2::server::handshake(tcp).await.unwrap();
                while let Some(Ok((request, respond))) = connection.accept().await {
                    tokio::spawn(serve(request, respond));
                }
            });
        }
    });
    addr
}

async fn serve(request: http::Request<h2::RecvStream>, mut respond: h2::server::SendResponse<Bytes>) {
    let path = request.uri().path().to_string();
    let te = request.headers().get("te").is_some_and(|v| v == "trailers");
    let mut body = request.into_body();
    let grpc = || http::Response::builder().status(200).header("content-type", "application/grpc");
    match path.as_str() {
        "/test.Echo/Fail" => {
            let head = grpc().header("grpc-status", "13").header("grpc-message", "boom").body(()).unwrap();
            respond.send_response(head, true).unwrap();
        }
        "/test.Echo/Plain" => {
            let head = http::Response::builder().status(200).header("content-type", "text/plain").header("content-length", "5").body(()).unwrap();
            let mut send = respond.send_response(head, false).unwrap();
            send.send_data(Bytes::from_static(b"hello"), true).unwrap();
        }
        _ => {
            let mut send = respond.send_response(grpc().body(()).unwrap(), false).unwrap();
            while let Some(chunk) = body.data().await {
                let chunk = chunk.unwrap();
                let _ = body.flow_control().release_capacity(chunk.len());
                send.send_data(chunk, false).unwrap();
            }
            if path == "/test.Echo/ServerStream" {
                for i in 0..3u8 {
                    send.send_data(message(&[b'm', b'0' + i]), false).unwrap();
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
            let mut trailers = http::HeaderMap::new();
            trailers.insert("grpc-status", "0".parse().unwrap());
            trailers.insert("x-te-trailers", if te { "yes" } else { "no" }.parse().unwrap());
            send.send_trailers(trailers).unwrap();
        }
    }
}

/// An h2 client of rws's HTTP/2 server (in memory) running `app`.
async fn client<A>(app: A) -> h2::client::SendRequest<Bytes>
where
    A: crate::application::Application + Send + Sync + Clone + 'static,
{
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(crate::h2_server::handle_connection(server_io, "127.0.0.1:5555".parse().unwrap(), None, app));
    let (send, connection) = h2::client::handshake(client_io).await.unwrap();
    tokio::spawn(connection);
    send
}

async fn grpc_client() -> h2::client::SendRequest<Bytes> {
    let backend = grpc_backend().await;
    client(App::new().wrap(GrpcProxy::new([format!("grpc://{backend}")]))).await
}

fn grpc_call(path: &str) -> http::Request<()> {
    http::Request::post(format!("https://rws{path}"))
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .body(())
        .unwrap()
}

async fn within<F: std::future::Future>(f: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(10), f).await.expect("timed out")
}

/// The whole response: status, body bytes, trailers.
async fn finish(response: h2::client::ResponseFuture) -> (http::response::Parts, Vec<u8>, Option<http::HeaderMap>) {
    let (parts, mut body) = within(response).await.unwrap().into_parts();
    let mut bytes = Vec::new();
    while let Some(chunk) = within(body.data()).await {
        let chunk = chunk.unwrap();
        let _ = body.flow_control().release_capacity(chunk.len());
        bytes.extend_from_slice(&chunk);
    }
    let trailers = within(body.trailers()).await.unwrap();
    (parts, bytes, trailers)
}

#[tokio::test]
async fn unary_call_keeps_the_grpc_status_trailer_and_te() {
    let mut rws = grpc_client().await;
    for _ in 0..3 {
        // Repeated calls reuse the pooled upstream connection.
        let (response, mut send) = within(rws.clone().ready()).await.unwrap().send_request(grpc_call("/test.Echo/Unary"), false).unwrap();
        send.send_data(message(b"ping"), true).unwrap();
        let (parts, body, trailers) = finish(response).await;
        assert_eq!(200, parts.status.as_u16());
        assert_eq!(message(b"ping").to_vec(), body);
        let trailers = trailers.expect("trailers");
        assert_eq!("0", trailers["grpc-status"]);
        assert_eq!("yes", trailers["x-te-trailers"]);
    }
    let _ = &mut rws;
}

#[tokio::test]
async fn server_streaming_delivers_every_message_then_the_status() {
    let rws = grpc_client().await;
    let (response, mut send) = within(rws.ready()).await.unwrap().send_request(grpc_call("/test.Echo/ServerStream"), false).unwrap();
    send.send_data(message(b"go"), true).unwrap();
    let (_, body, trailers) = finish(response).await;
    let mut expected = message(b"go").to_vec();
    for i in 0..3u8 {
        expected.extend_from_slice(&message(&[b'm', b'0' + i]));
    }
    assert_eq!(expected, body);
    assert_eq!("0", trailers.expect("trailers")["grpc-status"]);
}

#[tokio::test]
async fn bidirectional_messages_cross_while_both_bodies_are_open() {
    let rws = grpc_client().await;
    let (response, mut send) = within(rws.ready()).await.unwrap().send_request(grpc_call("/test.Echo/Bidi"), false).unwrap();
    send.send_data(message(b"one"), false).unwrap();
    let (_, mut body) = within(response).await.unwrap().into_parts();
    // The echo of the first message arrives before the second is sent.
    let first = within(body.data()).await.unwrap().unwrap();
    assert_eq!(message(b"one"), first);
    send.send_data(message(b"two"), true).unwrap();
    let second = within(body.data()).await.unwrap().unwrap();
    assert_eq!(message(b"two"), second);
    assert!(within(body.data()).await.is_none());
    assert_eq!("0", within(body.trailers()).await.unwrap().expect("trailers")["grpc-status"]);
}

#[tokio::test]
async fn a_trailers_only_error_keeps_its_status() {
    let rws = grpc_client().await;
    let (response, mut send) = within(rws.ready()).await.unwrap().send_request(grpc_call("/test.Echo/Fail"), false).unwrap();
    send.send_data(message(b"x"), true).unwrap();
    let (parts, body, _) = finish(response).await;
    assert_eq!("13", parts.headers["grpc-status"]);
    assert_eq!("boom", parts.headers["grpc-message"]);
    assert!(body.is_empty());
}

#[tokio::test]
async fn a_small_plain_response_is_buffered_with_its_length() {
    let backend = grpc_backend().await;
    let rws = client(App::new().wrap(H2ReverseProxy::new([format!("h2://{backend}")]))).await;
    let request = http::Request::get("https://rws/test.Echo/Plain").body(()).unwrap();
    let (response, _) = within(rws.ready()).await.unwrap().send_request(request, true).unwrap();
    let (parts, body, _) = finish(response).await;
    assert_eq!(b"hello".to_vec(), body);
    assert_eq!("5", parts.headers["content-length"]);
}

#[tokio::test]
async fn an_unreachable_backend_is_a_502() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let rws = client(App::new().wrap(GrpcProxy::new([format!("grpc://{closed}")]))).await;
    let (response, mut send) = within(rws.ready()).await.unwrap().send_request(grpc_call("/test.Echo/Unary"), false).unwrap();
    send.send_data(message(b"x"), true).unwrap();
    let (parts, _, _) = finish(response).await;
    assert_eq!(502, parts.status.as_u16());
}

#[tokio::test]
async fn the_config_files_grpc_action_streams_over_http2() {
    let backend = grpc_backend().await;
    let toml = format!(
        "[[upstream]]\nname = \"echo\"\nbackends = [\"grpc://{backend}\"]\n\n\
         [[route]]\n[route.match]\npath_prefix = \"/test.Echo\"\n[route.action]\ntype = \"grpc\"\nupstream = \"echo\"\n"
    );
    let (app, _) = crate::proxy_config::builder::build(crate::proxy_config::ProxyConfig::from_str(&toml));
    let rws = client(app).await;
    let (response, mut send) = within(rws.ready()).await.unwrap().send_request(grpc_call("/test.Echo/Bidi"), false).unwrap();
    send.send_data(message(b"one"), false).unwrap();
    let (_, mut body) = within(response).await.unwrap().into_parts();
    assert_eq!(message(b"one"), within(body.data()).await.unwrap().unwrap());
    send.send_data(message(b"two"), true).unwrap();
    assert_eq!(message(b"two"), within(body.data()).await.unwrap().unwrap());
    assert!(within(body.data()).await.is_none());
    assert_eq!("0", within(body.trailers()).await.unwrap().expect("trailers")["grpc-status"]);
}

/// A private CA with a `localhost` server certificate and a client one.
struct Pki {
    ca: crate::test_env::TempPath,
    client_cert: crate::test_env::TempPath,
    client_key: crate::test_env::TempPath,
    server_config: std::sync::Arc<rustls::ServerConfig>,
}

fn pki(tag: &str) -> Pki {
    use rcgen::{CertificateParams, ExtendedKeyUsagePurpose, IsCa, BasicConstraints, Issuer, KeyPair, PKCS_ECDSA_P256_SHA256};
    use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let ca_cert = ca_params.clone().self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let leaf = |names: Vec<String>, usage| {
        let mut params = CertificateParams::new(names).unwrap();
        params.extended_key_usages = vec![usage];
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let cert = params.signed_by(&key, &issuer).unwrap();
        (cert.pem(), key.serialize_pem())
    };
    let (server_cert, server_key) = leaf(vec!["localhost".to_string()], ExtendedKeyUsagePurpose::ServerAuth);
    let (client_cert_pem, client_key_pem) = leaf(vec!["rws-client".to_string()], ExtendedKeyUsagePurpose::ClientAuth);

    let write = |name: &str, contents: &str| {
        let path = crate::test_env::TempPath::new(&format!("h2_upstream_{tag}_{name}"));
        std::fs::write(path.as_str(), contents).unwrap();
        path
    };
    let provider = std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca_cert.der().clone()).unwrap();
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(std::sync::Arc::new(roots), provider.clone()).build().unwrap();
    let chain = CertificateDer::pem_slice_iter(server_cert.as_bytes()).collect::<Result<Vec<_>, _>>().unwrap();
    let key = PrivateKeyDer::from_pem_slice(server_key.as_bytes()).unwrap();
    let mut server_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)
        .unwrap();
    server_config.alpn_protocols = vec![b"h2".to_vec()];
    Pki {
        ca: write("ca.pem", &ca_cert.pem()),
        client_cert: write("client.pem", &client_cert_pem),
        client_key: write("client.key", &client_key_pem),
        server_config: std::sync::Arc::new(server_config),
    }
}

/// The echo backend over TLS, requiring a client certificate.
async fn mtls_backend(config: std::sync::Arc<rustls::ServerConfig>) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(tcp).await else { return };
                let Ok(mut connection) = h2::server::handshake(tls).await else { return };
                while let Some(Ok((request, respond))) = connection.accept().await {
                    tokio::spawn(serve(request, respond));
                }
            });
        }
    });
    port
}

#[tokio::test]
async fn a_private_ca_and_a_client_certificate_reach_an_mtls_backend() {
    let pki = pki("mtls");
    let port = mtls_backend(std::sync::Arc::clone(&pki.server_config)).await;
    let backend = [format!("h2s://localhost:{port}")];
    let plain = || http::Request::get("https://rws/test.Echo/Plain").body(()).unwrap();

    let with_cert = H2ReverseProxy::new(backend.clone())
        .tls_ca_file(pki.ca.as_str())
        .unwrap()
        .tls_client_cert(pki.client_cert.as_str(), pki.client_key.as_str())
        .unwrap();
    let rws = client(App::new().wrap(with_cert)).await;
    let (response, _) = within(rws.ready()).await.unwrap().send_request(plain(), true).unwrap();
    let (parts, body, _) = finish(response).await;
    assert_eq!(200, parts.status.as_u16());
    assert_eq!(b"hello".to_vec(), body);

    // Trusting the CA isn't enough: the backend wants a certificate.
    let without_cert = H2ReverseProxy::new(backend.clone()).tls_ca_file(pki.ca.as_str()).unwrap();
    let rws = client(App::new().wrap(without_cert)).await;
    let (response, _) = within(rws.ready()).await.unwrap().send_request(plain(), true).unwrap();
    assert_eq!(502, finish(response).await.0.status.as_u16());

    // Nor is the certificate without trusting the CA.
    let public_roots = H2ReverseProxy::new(backend).tls_client_cert(pki.client_cert.as_str(), pki.client_key.as_str()).unwrap();
    let rws = client(App::new().wrap(public_roots)).await;
    let (response, _) = within(rws.ready()).await.unwrap().send_request(plain(), true).unwrap();
    assert_eq!(502, finish(response).await.0.status.as_u16());
}

#[test]
fn client_certificate_errors_are_reported() {
    let err = crate::proxy::UpstreamTls::new(None, Some(("/nonexistent/client.pem", "/nonexistent/client.key"))).err().unwrap();
    assert!(err.contains("client certificate"), "{err}");
}

#[tokio::test]
async fn the_http1_reverse_proxy_presents_the_client_certificate_too() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let pki = pki("mtls_h1");
    let mut config = (*pki.server_config).clone();
    config.alpn_protocols.clear();
    let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(tcp).await else { return };
                let mut buf = [0u8; 4096];
                let _ = tls.read(&mut buf).await;
                let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await;
                let _ = tls.shutdown().await;
            });
        }
    });
    let (ca, cert, key) = (pki.ca.as_str().to_string(), pki.client_cert.as_str().to_string(), pki.client_key.as_str().to_string());
    let statuses = tokio::task::spawn_blocking(move || {
        use crate::middleware::Middleware;
        let get = crate::request::Request {
            method: "GET".into(),
            request_uri: "/".into(),
            http_version: "HTTP/1.1".into(),
            headers: vec![],
            body: Default::default(),
        };
        let conn = crate::server::ConnectionInfo {
            client: crate::server::Address { ip: "127.0.0.1".into(), port: 0 },
            server: crate::server::Address { ip: "127.0.0.1".into(), port: 0 },
            request_size: 16000,
            sni_hostname: None,
        };
        let backend = [format!("https://localhost:{port}")];
        let with_cert = crate::proxy::ReverseProxy::new(backend.clone()).tls_ca_file(&ca).unwrap().tls_client_cert(&cert, &key).unwrap();
        let without = crate::proxy::ReverseProxy::new(backend).tls_ca_file(&ca).unwrap();
        let status = |p: &crate::proxy::ReverseProxy| p.handle(&get, &conn, &App::new()).map_or(0, |r| r.status_code);
        (status(&with_cert), status(&without))
    })
    .await
    .unwrap();
    assert_eq!((200, 502), statuses);
}
