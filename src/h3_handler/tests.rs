//! HTTP/3 end to end: a real QUIC client (quinn + h3) against
//! `handle_connection`, in-process, on a loopback UDP socket.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Buf, Bytes};

use crate::application::Application;
use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::test_env::TempPath;

const BIG_FILE_SIZE: usize = 2 * 1024 * 1024 + 123;

#[derive(Clone)]
struct H3App {
    big_file: String,
}

impl Application for H3App {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        match request.request_uri.as_str() {
            "/hello" => Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "hello over h3")),
            "/echo" => {
                let body = format!("{} {}", request.method, String::from_utf8_lossy(&request.body));
                Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &body))
            }
            "/big" => {
                let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
                response.headers.push(Header { name: Header::_CONTENT_LENGTH.into(), value: BIG_FILE_SIZE.to_string().into() });
                response.stream_file = Some(self.big_file.clone());
                Ok(response)
            }
            _ => Ok(Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found")),
        }
    }
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

/// Start a QUIC endpoint serving `app` with a fresh self-signed certificate for
/// `localhost`. Returns its address and the certificate (DER) to trust.
fn start_server<A: Application + Clone + Send + Sync + 'static>(app: A, cert_file: &TempPath, key_file: &TempPath) -> (SocketAddr, Vec<u8>) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(cert_file.as_str(), certified.cert.pem()).unwrap();
    std::fs::write(key_file.as_str(), certified.signing_key.serialize_pem()).unwrap();
    let server_config = crate::tls::create_quinn_server_config(cert_file.as_str(), key_file.as_str()).unwrap();
    let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = endpoint.local_addr().unwrap();
    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            let app = app.clone();
            tokio::spawn(async move {
                if let Ok(conn) = incoming.await {
                    let peer = conn.remote_address();
                    let _ = super::handle_connection(conn, peer, app).await;
                }
            });
        }
    });
    (addr, certified.cert.der().to_vec())
}

type SendRequest = h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>;

async fn connect(addr: SocketAddr, cert_der: Vec<u8>) -> (SendRequest, quinn::Endpoint) {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(cert_der)).unwrap();
    let mut tls = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap();

    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic)));
    let conn = endpoint.connect(addr, "localhost").unwrap().await.unwrap();
    let (mut driver, send_request) = h3::client::new(h3_quinn::Connection::new(conn)).await.unwrap();
    tokio::spawn(async move {
        let _ = driver.wait_idle().await;
    });
    (send_request, endpoint)
}

/// Send one request; returns the status, the response headers and the body.
async fn fetch(
    send_request: &mut SendRequest,
    method: &str,
    path: &str,
    body: Option<&'static [u8]>,
) -> (u16, http::HeaderMap, Vec<u8>) {
    let request = http::Request::builder()
        .method(method)
        .uri(format!("https://localhost{}", path))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(request).await.unwrap();
    if let Some(body) = body {
        stream.send_data(Bytes::from_static(body)).await.unwrap();
    }
    stream.finish().await.unwrap();
    let response = stream.recv_response().await.unwrap();
    let mut out = Vec::new();
    while let Some(mut chunk) = stream.recv_data().await.unwrap() {
        while chunk.has_remaining() {
            let part = chunk.chunk();
            out.extend_from_slice(part);
            let n = part.len();
            chunk.advance(n);
        }
    }
    (response.status().as_u16(), response.headers().clone(), out)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http3_round_trip_with_a_real_quic_client() {
    let cert_file = TempPath::new("h3_cert.pem");
    let key_file = TempPath::new("h3_key.pem");
    let big_file = TempPath::new("h3_big.bin");
    let content: Vec<u8> = (0..BIG_FILE_SIZE).map(|i| (i % 251) as u8).collect();
    std::fs::write(big_file.as_str(), &content).unwrap();

    let (addr, cert_der) = start_server(H3App { big_file: big_file.as_str().to_string() }, &cert_file, &key_file);
    let (mut send_request, endpoint) = connect(addr, cert_der).await;

    let (status, _, body) = fetch(&mut send_request, "GET", "/hello", None).await;
    assert_eq!(status, 200);
    assert_eq!(body, b"hello over h3");

    let (status, _, body) = fetch(&mut send_request, "POST", "/echo", Some(b"request body")).await;
    assert_eq!(status, 200);
    assert_eq!(String::from_utf8_lossy(&body), "POST request body");

    // stream_file: read from disk in chunks and sent over flow-controlled DATA frames.
    let (status, headers, body) = fetch(&mut send_request, "GET", "/big", None).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("content-length").unwrap(), &BIG_FILE_SIZE.to_string());
    assert_eq!(body.len(), BIG_FILE_SIZE);
    assert!(body == content, "streamed body differs from the file");

    // HEAD keeps the headers, sends no body.
    let (status, headers, body) = fetch(&mut send_request, "HEAD", "/big", None).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("content-length").unwrap(), &BIG_FILE_SIZE.to_string());
    assert!(body.is_empty());

    let (status, _, _) = fetch(&mut send_request, "GET", "/missing", None).await;
    assert_eq!(status, 404);

    endpoint.close(0u32.into(), b"done");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_sse_stream_over_http3() {
    // #12: GET /mcp's SSE stream is delivered over HTTP/3 as it's produced.
    let cert_file = TempPath::new("h3_mcp_cert.pem");
    let key_file = TempPath::new("h3_mcp_key.pem");
    let mcp = crate::mcp::McpServer::new("t", "1");
    let (addr, cert_der) = start_server(mcp.clone(), &cert_file, &key_file);
    let (mut send_request, _endpoint) = connect(addr, cert_der).await;
    let request = http::Request::builder().method("GET").uri("https://localhost/mcp")
        .header("accept", "text/event-stream").body(()).unwrap();
    let mut stream = send_request.send_request(request).await.unwrap();
    stream.finish().await.unwrap();
    let response = stream.recv_response().await.unwrap();
    assert_eq!(200, response.status().as_u16());
    assert_eq!("text/event-stream", response.headers()["content-type"]);
    mcp.notify("notifications/test", Some(r#"{"n":1}"#));
    let mut chunk = tokio::time::timeout(std::time::Duration::from_secs(5), stream.recv_data()).await
        .expect("no SSE event within 5 s").unwrap().expect("stream ended");
    let mut text = Vec::new();
    while chunk.has_remaining() {
        let part = chunk.chunk();
        text.extend_from_slice(part);
        let n = part.len();
        chunk.advance(n);
    }
    let text = String::from_utf8_lossy(&text);
    assert!(text.contains("data: ") && text.contains("notifications/test"), "{text}");
}
