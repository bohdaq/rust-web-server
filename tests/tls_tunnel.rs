//! `tls_tunnels`: one name on the HTTPS listener is a TLS tunnel to a local
//! TCP service; other names are served as HTTP. A test binary of its own:
//! the setting is read once per process.
#![cfg(feature = "http2")]

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A local TCP service answering each read with "echo:" and the bytes.
fn echo() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = [0u8; 1024];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let mut reply = b"echo:".to_vec();
                    reply.extend_from_slice(&buf[..n]);
                    if stream.write_all(&reply).is_err() {
                        break;
                    }
                }
            });
        }
    });
    addr
}

/// The HTTPS listener with a certificate for both names; its root.
fn start(
    runtime: &tokio::runtime::Runtime,
) -> (SocketAddr, rustls::pki_types::CertificateDer<'static>) {
    let key =
        rcgen::generate_simple_self_signed(vec!["tunnel.test".into(), "web.test".into()]).unwrap();
    let dir = std::env::temp_dir().join(format!("rws-tls-tunnel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (cert, private) = (dir.join("cert.pem"), dir.join("key.pem"));
    std::fs::write(&cert, key.cert.pem()).unwrap();
    std::fs::write(&private, key.signing_key.serialize_pem()).unwrap();
    let acceptor = rust_web_server::tls::create_tls_acceptor(
        cert.to_str().unwrap(),
        private.to_str().unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let app = App::with_state(()).get("/hello", |_, _, _, _| {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "web")
    });
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    runtime.spawn(async move {
        loop {
            let (stream, peer) = listener.accept().await.unwrap();
            tokio::spawn(rust_web_server::server::serve_tls_connection(
                stream,
                peer,
                acceptor.clone(),
                app.clone(),
            ));
        }
    });
    (addr, key.cert.der().clone())
}

async fn connect(
    addr: SocketAddr,
    root: &rustls::pki_types::CertificateDer<'static>,
    name: &'static str,
    alpn: &[&[u8]],
) -> tokio_rustls::client::TlsStream<tokio::net::TcpStream> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(root.clone()).unwrap();
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(rustls::pki_types::ServerName::try_from(name).unwrap(), tcp)
        .await
        .unwrap()
}

#[test]
fn a_tunnel_name_reaches_its_service_and_others_are_http() {
    let service = echo();
    std::env::set_var("RWS_CONFIG_TLS_TUNNELS", format!("Tunnel.Test={service}"));
    assert_eq!(
        Some(service),
        rust_web_server::server::tls_tunnel::target(Some("tunnel.test"))
    );
    assert_eq!(
        None,
        rust_web_server::server::tls_tunnel::target(Some("web.test"))
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (addr, root) = start(&runtime);

    runtime.block_on(async {
        // The tunnel name: bytes both ways, after rws's own TLS handshake.
        let mut tunnel = connect(addr, &root, "tunnel.test", &[]).await;
        tunnel.write_all(b"ping").await.unwrap();
        let mut got = [0u8; 9];
        tunnel.read_exact(&mut got).await.unwrap();
        assert_eq!(b"echo:ping", &got);
        tunnel.write_all(b"again").await.unwrap();
        let mut got = [0u8; 10];
        tunnel.read_exact(&mut got).await.unwrap();
        assert_eq!(b"echo:again", &got);

        // Whatever ALPN the client offers, the name decides.
        let mut h2 = connect(addr, &root, "tunnel.test", &[b"h2", b"http/1.1"]).await;
        h2.write_all(b"x").await.unwrap();
        let mut got = [0u8; 6];
        h2.read_exact(&mut got).await.unwrap();
        assert_eq!(b"echo:x", &got);

        // Another name on the same listener is the web site.
        let mut web = connect(addr, &root, "web.test", &[b"http/1.1"]).await;
        web.write_all(b"GET /hello HTTP/1.1\r\nHost: web.test\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut reply = String::new();
        let _ = web.read_to_string(&mut reply).await;
        assert!(
            reply.starts_with("HTTP/1.1 200") && reply.ends_with("web"),
            "{reply}"
        );
    });
}
