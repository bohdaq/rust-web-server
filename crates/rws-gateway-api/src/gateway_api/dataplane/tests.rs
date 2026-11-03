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

/// Serve what `build` makes for a free port; another port if a parallel
/// test took that one between finding it free and the bind.
fn serve_on_free_port(plane: &DataPlane, build: impl Fn(u16) -> GatewayConfig) -> u16 {
    for _ in 0..20 {
        let port = free_port();
        plane.apply(&build(port));
        if plane.ports().contains(&port) {
            return port;
        }
    }
    panic!("no free port");
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn config(container_port: u16, backend: u16, protocol: &str, tls: Option<(Vec<u8>, Vec<u8>)>) -> GatewayConfig {
    let rule = CompiledRule {
        name: None,
        matches: vec![HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some("PathPrefix".to_string()), value: Some("/".to_string()) }), ..Default::default() }],
        filters: vec![],
        backends: vec![Backend { weight: 1, endpoints: Ok(vec![format!("127.0.0.1:{backend}")]), app_protocol: None, tls: None, filters: vec![] }],
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
            client_ca: None,
            backend_client_cert: None,
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
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| config(port, backend, "HTTP", None));
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
    let cert = rcgen::generate_simple_self_signed(vec!["secure.example.com".to_string()]).unwrap();
    let (cert_pem, key_pem) = (cert.cert.pem(), cert.signing_key.serialize_pem());
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| config(port, backend, "HTTPS", Some((cert_pem.clone().into_bytes(), key_pem.clone().into_bytes()))));
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

/// A TCP backend echoing every byte back.
fn tcp_echo_backend() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut reader = stream.try_clone().unwrap();
                let mut writer = stream;
                let _ = std::io::copy(&mut reader, &mut writer);
            });
        }
    });
    port
}

fn l4_config(container_port: u16, backend: u16, protocol: &str, tls: Option<(Vec<u8>, Vec<u8>)>) -> GatewayConfig {
    let mut config = config(container_port, backend, protocol, tls);
    config.listeners[0].http_routes[0].rules[0].matches.clear();
    config
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tcp_listener_relays_bytes() {
    let backend = tcp_echo_backend();
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| l4_config(port, backend, "TCP", None));
    let echoed = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        stream.write_all(b"raw bytes").unwrap();
        let mut buffer = [0u8; 9];
        stream.read_exact(&mut buffer).unwrap();
        buffer.to_vec()
    })
    .await
    .unwrap();
    assert_eq!(b"raw bytes".to_vec(), echoed);
}

/// A TLS client to the gateway's `port` for `name`, trusting `cert`: sends
/// "ping", returns what comes back.
fn tls_ping(port: u16, name: &str, cert: &rcgen::CertifiedKey<rcgen::KeyPair>) -> Vec<u8> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec())).unwrap();
    let config = crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth();
    let connection = rustls::ClientConnection::new(Arc::new(config), name.to_string().try_into().unwrap()).unwrap();
    let tcp = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    let mut tls = rustls::StreamOwned::new(connection, tcp);
    tls.write_all(b"ping").unwrap();
    let mut buffer = [0u8; 4];
    tls.read_exact(&mut buffer).unwrap();
    buffer.to_vec()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tls_terminate_listener_decrypts_and_relays() {
    let backend = tcp_echo_backend();
    let cert = rcgen::generate_simple_self_signed(vec!["term.example.com".to_string()]).unwrap();
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| l4_config(port, backend, "TLS", Some((cert.cert.pem().into_bytes(), cert.signing_key.serialize_pem().into_bytes()))));
    let echoed = tokio::task::spawn_blocking(move || tls_ping(port, "term.example.com", &cert)).await.unwrap();
    assert_eq!(b"ping".to_vec(), echoed, "plaintext reached the echo backend and came back encrypted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tls_passthrough_listener_forwards_the_encrypted_stream() {
    // The backend terminates TLS itself, with its own certificate.
    let cert = rcgen::generate_simple_self_signed(vec!["pass.example.com".to_string()]).unwrap();
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec())], rustls::pki_types::PrivateKeyDer::try_from(cert.signing_key.serialize_der()).unwrap())
        .unwrap();
    let server_config = Arc::new(server_config);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let backend = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let connection = rustls::ServerConnection::new(server_config).unwrap();
        let mut tls = rustls::StreamOwned::new(connection, tcp);
        let mut buffer = [0u8; 4];
        if tls.read_exact(&mut buffer).is_ok() {
            let _ = tls.write_all(&buffer);
            let _ = tls.flush();
        }
    });
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| {
        let mut config = l4_config(port, backend, "TLS", None);
        config.listeners[0].http_routes[0].hostnames = vec!["pass.example.com".to_string()];
        config
    });
    let echoed = tokio::task::spawn_blocking(move || tls_ping(port, "pass.example.com", &cert)).await.unwrap();
    assert_eq!(b"ping".to_vec(), echoed, "the backend's certificate, end to end");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_udp_listener_relays_datagrams_both_ways() {
    let backend = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let backend_port = backend.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1500];
        while let Ok((n, from)) = backend.recv_from(&mut buffer) {
            let _ = backend.send_to(&buffer[..n], from);
        }
    });
    let port = std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let plane = DataPlane::new("127.0.0.1");
    plane.apply(&l4_config(port, backend_port, "UDP", None));
    let replies = tokio::task::spawn_blocking(move || {
        let client = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut replies = Vec::new();
        for message in [&b"one"[..], b"two"] {
            client.send_to(message, ("127.0.0.1", port)).unwrap();
            let mut buffer = [0u8; 1500];
            let (n, _) = client.recv_from(&mut buffer).unwrap();
            replies.push(buffer[..n].to_vec());
        }
        replies
    })
    .await
    .unwrap();
    assert_eq!(vec![b"one".to_vec(), b"two".to_vec()], replies);
    assert_eq!(vec![port], plane.ports());
}

/// A CA and a client certificate it signed.
fn client_pki() -> (String, rustls::pki_types::CertificateDer<'static>, rustls::pki_types::PrivateKeyDer<'static>) {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&ca_params, &ca_key);
    let client_key = rcgen::KeyPair::generate().unwrap();
    let client = rcgen::CertificateParams::new(vec!["client".to_string()]).unwrap().signed_by(&client_key, &issuer).unwrap();
    (ca.pem(), client.der().clone(), rustls::pki_types::PrivateKeyDer::try_from(client_key.serialize_der()).unwrap())
}

/// GET over TLS, with a client certificate or none; the response, or
/// `None` when the handshake or exchange fails.
fn get_with_client_cert(port: u16, server: &rcgen::CertifiedKey<rcgen::KeyPair>, client: Option<(rustls::pki_types::CertificateDer<'static>, rustls::pki_types::PrivateKeyDer<'static>)>) -> Option<String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(server.cert.der().to_vec())).unwrap();
    let builder = crate::tls_client_builder().with_root_certificates(roots);
    let config = match client {
        Some((cert, key)) => builder.with_client_auth_cert(vec![cert], key).unwrap(),
        None => builder.with_no_client_auth(),
    };
    let connection = rustls::ClientConnection::new(Arc::new(config), "secure.example.com".try_into().unwrap()).unwrap();
    let tcp = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    let mut tls = rustls::StreamOwned::new(connection, tcp);
    write!(tls, "GET / HTTP/1.1\r\nHost: secure.example.com\r\nConnection: close\r\n\r\n").ok()?;
    let mut response = String::new();
    tls.read_to_string(&mut response).ok()?;
    response.starts_with("HTTP/1.1").then_some(response)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_certificates_are_required_or_let_through_by_the_mode() {
    let backend = echo_backend();
    let server = rcgen::generate_simple_self_signed(vec!["secure.example.com".to_string()]).unwrap();
    let (ca_pem, client_cert, client_key) = client_pki();
    let (_, other_cert, other_key) = client_pki();
    let plane = DataPlane::new("127.0.0.1");
    for insecure_fallback in [false, true] {
        let port = serve_on_free_port(&plane, |port| {
            let mut config = config(port, backend, "HTTPS", Some((server.cert.pem().into_bytes(), server.signing_key.serialize_pem().into_bytes())));
            config.listeners[0].client_ca = Some(crate::gateway_api::translate::ClientCa { pem: ca_pem.clone().into_bytes(), insecure_fallback });
            config
        });
        let server = rcgen::CertifiedKey { cert: server.cert.clone(), signing_key: rcgen::KeyPair::from_pem(&server.signing_key.serialize_pem()).unwrap() };
        let (valid, other, none) = (Some((client_cert.clone(), client_key.clone_key())), Some((other_cert.clone(), other_key.clone_key())), None);
        let results = tokio::task::spawn_blocking(move || {
            [valid, other, none].into_iter().map(|client| get_with_client_cert(port, &server, client).is_some_and(|r| r.starts_with("HTTP/1.1 200"))).collect::<Vec<_>>()
        })
        .await
        .unwrap();
        let expected = if insecure_fallback { vec![true, true, true] } else { vec![true, false, false] };
        assert_eq!(expected, results, "insecure_fallback = {insecure_fallback}: [signed by the CA, another CA, none]");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_port_that_changes_kind_keeps_its_socket_and_serves_the_new_way() {
    let http_backend = echo_backend();
    let tcp_backend = tcp_echo_backend();
    let plane = DataPlane::new("127.0.0.1");
    let port = serve_on_free_port(&plane, |port| config(port, http_backend, "HTTP", None));
    plane.apply(&l4_config(port, tcp_backend, "TCP", None));
    assert_eq!(vec![port], plane.ports(), "no rebind needed");
    let echoed = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        stream.write_all(b"raw").unwrap();
        let mut buffer = [0u8; 3];
        stream.read_exact(&mut buffer).unwrap();
        buffer.to_vec()
    })
    .await
    .unwrap();
    assert_eq!(b"raw".to_vec(), echoed, "relayed as TCP now, not parsed as HTTP");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_udp_port_given_up_with_live_sessions_can_be_served_again_at_once() {
    let backend = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let backend_port = backend.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1500];
        while let Ok((n, from)) = backend.recv_from(&mut buffer) {
            let _ = backend.send_to(&buffer[..n], from);
        }
    });
    let ping = |port: u16| {
        let client = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
        client.send_to(b"hi", ("127.0.0.1", port)).unwrap();
        let mut buffer = [0u8; 16];
        client.recv_from(&mut buffer).map(|(n, _)| buffer[..n].to_vec()).ok()
    };
    let port = std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let plane = DataPlane::new("127.0.0.1");
    plane.apply(&l4_config(port, backend_port, "UDP", None));
    assert_eq!(Some(b"hi".to_vec()), tokio::task::spawn_blocking(move || ping(port)).await.unwrap(), "a session is open now");
    plane.apply(&GatewayConfig::default());
    plane.apply(&l4_config(port, backend_port, "UDP", None));
    let started = std::time::Instant::now();
    while !plane.ports().contains(&port) {
        assert!(started.elapsed() < std::time::Duration::from_secs(3), "the port is served again within the retries");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(Some(b"hi".to_vec()), tokio::task::spawn_blocking(move || ping(port)).await.unwrap());
}
