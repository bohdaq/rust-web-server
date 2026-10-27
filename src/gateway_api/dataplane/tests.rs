use std::io::{Read, Write};

use super::*;
use crate::gateway_api::routes::{Backend, CompiledHttpRoute, CompiledRule};
use crate::gateway_api::types::{HttpPathMatch, HttpRouteMatch};

/// A backend answering with the Host and path it got.
fn echo_backend() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = text.lines().next().unwrap_or("").to_string();
                let host = text.lines().find_map(|l| l.strip_prefix("Host: ")).unwrap_or("").to_string();
                let body = format!("{line}|{host}");
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    port
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn config(container_port: u16, backend: u16, protocol: &str, tls: Option<(Vec<u8>, Vec<u8>)>) -> GatewayConfig {
    let rule = CompiledRule {
        name: None,
        matches: vec![HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some("PathPrefix".to_string()), value: Some("/".to_string()) }), ..Default::default() }],
        filters: vec![],
        backends: vec![Backend { weight: 1, endpoints: Ok(vec![format!("127.0.0.1:{backend}")]), app_protocol: None, filters: vec![] }],
        timeouts: None,
        retry: None,
        mirrors: vec![],
    };
    GatewayConfig {
        listeners: vec![ListenerConfig {
            gateway: ("ns".to_string(), "gw".to_string()),
            name: "l".to_string(),
            port: 80,
            protocol: protocol.to_string(),
            hostname: None,
            container_port,
            tls,
            http_routes: vec![CompiledHttpRoute { key: ("ns".to_string(), "r".to_string()), hostnames: vec![], created: String::new(), rules: vec![rule] }],
        }],
    }
}

fn get(port: u16, host: &str, path: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_listener_port_forwards_keeping_the_host_and_closes_when_removed() {
    let backend = echo_backend();
    let port = free_port();
    let plane = DataPlane::new("127.0.0.1");
    plane.apply(&config(port, backend, "HTTP", None));
    assert_eq!(vec![port], plane.ports());
    let response = tokio::task::spawn_blocking(move || get(port, "shop.example.com:80", "/cart?id=1")).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("GET /cart?id=1 HTTP/1.1|shop.example.com:80"), "path and Host as the client sent them: {response}");

    plane.apply(&GatewayConfig::default());
    assert!(plane.ports().is_empty());
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "the port is closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_https_listener_terminates_tls_with_the_secret_certificate() {
    let backend = echo_backend();
    let port = free_port();
    let cert = rcgen::generate_simple_self_signed(vec!["secure.example.com".to_string()]).unwrap();
    let (cert_pem, key_pem) = (cert.cert.pem(), cert.signing_key.serialize_pem());
    let plane = DataPlane::new("127.0.0.1");
    plane.apply(&config(port, backend, "HTTPS", Some((cert_pem.clone().into_bytes(), key_pem.into_bytes()))));
    let body = tokio::task::spawn_blocking(move || {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec())).unwrap();
        let config = crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth();
        let connection = rustls::ClientConnection::new(Arc::new(config), "secure.example.com".try_into().unwrap()).unwrap();
        let tcp = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        tcp.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut tls = rustls::StreamOwned::new(connection, tcp);
        write!(tls, "GET /pay HTTP/1.1\r\nHost: secure.example.com\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        let _ = tls.read_to_string(&mut response);
        response
    })
    .await
    .unwrap();
    assert!(body.starts_with("HTTP/1.1 200"), "{body}");
    assert!(body.contains("GET /pay HTTP/1.1|secure.example.com"), "{body}");
}
