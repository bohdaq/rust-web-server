use super::*;

fn certificate(name: &str, not_after: (i32, u8, u8)) -> rcgen::CertifiedKey<rcgen::KeyPair> {
    let mut params = rcgen::CertificateParams::new(vec![name.to_string()]).unwrap();
    params.not_after = rcgen::date_time_ymd(not_after.0, not_after.1, not_after.2);
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    rcgen::CertifiedKey { cert, signing_key: key }
}

#[test]
fn handshakes_and_failures_are_counted() {
    record_handshake("1.3", "TEST_ONLY_CIPHER", "X25519");
    record_handshake("1.3", "TEST_ONLY_CIPHER", "X25519");
    record_handshake_failure();
    let text = prometheus_text();
    assert!(text.contains(r#"rws_tls_handshakes_total{tls_protocol_version="1.3",tls_cipher="TEST_ONLY_CIPHER",tls_group="X25519"} 2"#), "{text}");
    assert!(text.contains("rws_tls_handshake_failures_total "), "{text}");
}

#[test]
fn certificate_expiry_is_exported_per_name() {
    let a = certificate("a.test", (2031, 3, 4));
    let expected = crate::core::x509::days_since_epoch(2031, 3, 4).unwrap() * 86_400;
    set_on_demand_certificate("ondemand.test", a.cert.der());
    let text = prometheus_text();
    assert!(text.contains(&format!("rws_tls_certificate_expiry_timestamp_seconds{{name=\"ondemand.test\"}} {expected}")), "{text}");
    // Not a certificate: nothing recorded, nothing panics.
    set_on_demand_certificate("garbage.test", b"\x30\x03\x02\x01");
    assert!(!prometheus_text().contains("garbage.test"));
}

#[test]
#[cfg(feature = "http2")]
fn record_rustls_names_what_a_real_handshake_negotiated() {
    use std::sync::Arc;
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let certified = certificate("localhost", (2035, 1, 1));
    let key = rustls::pki_types::PrivateKeyDer::try_from(certified.signing_key.serialize_der()).unwrap();
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certified.cert.der().clone()).unwrap();
    let client_config = crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth();
    let mut server = rustls::ServerConnection::new(Arc::new(server_config)).unwrap();
    let mut client = rustls::ClientConnection::new(Arc::new(client_config), "localhost".try_into().unwrap()).unwrap();
    for _ in 0..10 {
        let mut buf = Vec::new();
        client.write_tls(&mut buf).unwrap();
        server.read_tls(&mut buf.as_slice()).unwrap();
        server.process_new_packets().unwrap();
        let mut buf = Vec::new();
        server.write_tls(&mut buf).unwrap();
        client.read_tls(&mut buf.as_slice()).unwrap();
        client.process_new_packets().unwrap();
        if !server.is_handshaking() && !client.is_handshaking() {
            break;
        }
    }
    assert!(!server.is_handshaking());
    record_rustls(&server);
    let text = prometheus_text();
    let line = text
        .lines()
        .find(|l| l.starts_with("rws_tls_handshakes_total{tls_protocol_version=\"1.3\",tls_cipher=\"TLS13_"))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(!line.contains("unknown"), "{line}");
    assert!(line.contains("tls_group=\"X25519MLKEM768\""), "post-quantum hybrid by default: {line}");
}
