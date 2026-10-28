use super::*;

/// A CA, and a server certificate it signed for `names` plus a URI SAN.
fn pki(names: &[&str], uri: &str) -> (String, rcgen::Certificate, rcgen::KeyPair) {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&ca_params, &ca_key);
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(names.iter().map(|n| n.to_string()).collect::<Vec<_>>()).unwrap();
    params.subject_alt_names.push(rcgen::SanType::URI(uri.try_into().unwrap()));
    let cert = params.signed_by(&key, &issuer).unwrap();
    (ca.pem(), cert, key)
}

fn config_map(name: &str, ca_pem: &str) -> k8s_openapi::api::core::v1::ConfigMap {
    serde_json::from_value(serde_json::json!({"apiVersion": "v1", "kind": "ConfigMap", "metadata": {"name": name, "namespace": "ns"}, "data": {"ca.crt": ca_pem}})).unwrap()
}

fn policy(name: &str, created: i64, service: &str, section: Option<&str>, ca_kind: &str) -> BackendTlsPolicy {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "BackendTLSPolicy",
        "metadata": {"name": name, "namespace": "ns", "creationTimestamp": format!("2026-01-01T00:00:{created:02}Z")},
        "spec": {
            "targetRefs": [{"group": "", "kind": "Service", "name": service, "sectionName": section}],
            "validation": {"caCertificateRefs": [{"group": "", "kind": ca_kind, "name": "ca"}], "hostname": "abc.example.com"}
        }
    }))
    .unwrap()
}

fn verdict(evaluated: &[((String, String), Evaluated)], name: &str) -> (Option<&'static str>, Option<&'static str>) {
    let (_, e) = evaluated.iter().find(|(k, _)| k.1 == name).unwrap();
    (e.accepted.as_ref().err().map(|(r, _)| *r), e.resolved.as_ref().err().map(|(r, _)| *r))
}

#[test]
fn policies_are_accepted_rejected_or_conflicted() {
    let (ca, _, _) = pki(&["abc.example.com"], "spiffe://x");
    let snapshot = Snapshot {
        config_maps: vec![config_map("ca", &ca)],
        backend_tls_policies: vec![
            policy("first", 1, "svc", None, "ConfigMap"),
            policy("second", 2, "svc", None, "ConfigMap"),
            policy("port", 3, "svc", Some("https"), "ConfigMap"),
            policy("bad-kind", 4, "other", None, "Secret"),
        ],
        ..Default::default()
    };
    let evaluated = evaluate(&snapshot);
    assert_eq!((None, None), verdict(&evaluated, "first"));
    assert_eq!((Some("Conflicted"), None), verdict(&evaluated, "second"), "the older wins");
    assert_eq!((None, None), verdict(&evaluated, "port"), "a section is another target");
    assert_eq!((Some("NoValidCACertificate"), Some("InvalidKind")), verdict(&evaluated, "bad-kind"));
    assert!(for_service(&evaluated, &snapshot, "ns", "svc", Some("https")).is_some_and(|t| t.is_ok()));
    assert!(for_service(&evaluated, &snapshot, "ns", "svc", Some("http")).is_some_and(|t| t.is_ok()), "the whole-Service policy");
    assert!(for_service(&evaluated, &snapshot, "ns", "other", None).is_some_and(|t| t.is_err()), "covered but unusable");
    assert!(for_service(&evaluated, &snapshot, "ns", "plain", None).is_none());

    let missing = Snapshot { backend_tls_policies: vec![policy("missing", 1, "svc", None, "ConfigMap")], ..Default::default() };
    assert_eq!((Some("NoValidCACertificate"), Some("InvalidCACertificateRef")), verdict(&evaluate(&missing), "missing"));
}

/// A TLS backend with `cert`, answering 200; its port.
fn tls_backend(cert: &rcgen::Certificate, key: &rcgen::KeyPair) -> u16 {
    use std::io::{Read, Write};
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.der().clone()], rustls::pki_types::PrivateKeyDer::try_from(key.serialize_der()).unwrap())
        .unwrap();
    let config = Arc::new(config);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let config = Arc::clone(&config);
            std::thread::spawn(move || {
                let mut tls = rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), tcp);
                let mut buffer = [0u8; 4096];
                if tls.read(&mut buffer).unwrap_or(0) > 0 {
                    let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
                    let _ = tls.flush();
                }
            });
        }
    });
    port
}

fn status(tls: &BackendTls, port: u16) -> Result<i16, String> {
    let request = crate::request::Request {
        method: "GET".to_string(),
        request_uri: "/".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: vec![crate::header::Header::new("Host", "client.example")],
        body: Default::default(),
    };
    crate::proxy::forward_tls_with_host(&tls.config, &tls.label, &request, "10.0.0.9", "127.0.0.1", port, &tls.server_name, "client.example", std::time::Duration::from_secs(5), std::time::Duration::from_secs(5))
        .map(|r| r.status_code)
}

fn validation(hostname: &str, sans: &[(&str, &str)]) -> BackendTlsValidation {
    BackendTlsValidation {
        hostname: hostname.to_string(),
        subject_alt_names: sans
            .iter()
            .map(|(kind, value)| PolicySubjectAltName {
                san_type: kind.to_string(),
                hostname: if *kind == "Hostname" { value.to_string() } else { String::new() },
                uri: if *kind == "URI" { value.to_string() } else { String::new() },
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn backends_are_verified_by_hostname_or_subject_alt_names() {
    let (ca, cert, key) = pki(&["abc.example.com"], "spiffe://cluster.local/ns/x/sa/y");
    let port = tls_backend(&cert, &key);
    let ca = Some(ca.as_bytes());
    let ok = |v: BackendTlsValidation| status(&build(ca, &v).unwrap(), port);
    assert_eq!(Ok(200), ok(validation("abc.example.com", &[])));
    assert!(ok(validation("other.example.com", &[])).is_err(), "the hostname must match");
    assert_eq!(Ok(200), ok(validation("other.example.com", &[("Hostname", "abc.example.com")])), "SANs replace the hostname check");
    assert!(ok(validation("abc.example.com", &[("Hostname", "dce.example.com")])).is_err());
    assert_eq!(Ok(200), ok(validation("abc.example.com", &[("URI", "spiffe://cluster.local/ns/x/sa/y")])));
    assert!(ok(validation("abc.example.com", &[("URI", "spiffe://cluster.local/ns/x/sa/z")])).is_err());
    assert_eq!(Ok(200), ok(validation("abc.example.com", &[("Hostname", "dce.example.com"), ("URI", "spiffe://cluster.local/ns/x/sa/y")])), "any SAN");
    let (other_ca, _, _) = pki(&["abc.example.com"], "spiffe://x");
    assert!(status(&build(Some(other_ca.as_bytes()), &validation("abc.example.com", &[])).unwrap(), port).is_err(), "another CA");
}

#[test]
fn certificate_wildcards_cover_one_label() {
    assert!(dns_matches("*.example.com", "abc.example.com"));
    assert!(!dns_matches("*.example.com", "a.b.example.com"));
    assert!(!dns_matches("*.example.com", "example.com"));
    assert!(dns_matches("ABC.example.com", "abc.example.com"));
}

#[test]
fn the_gateways_client_certificate_reaches_a_backend_that_requires_one() {
    use std::io::{Read, Write};
    let (ca, cert, key) = pki(&["abc.example.com"], "spiffe://x");
    // The client side: a CA the backend trusts, and a certificate it signed.
    let client_ca_key = rcgen::KeyPair::generate().unwrap();
    let mut client_ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    client_ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let client_ca = client_ca_params.self_signed(&client_ca_key).unwrap();
    let client_key = rcgen::KeyPair::generate().unwrap();
    let client = rcgen::CertificateParams::new(vec!["gateway".to_string()]).unwrap().signed_by(&client_key, &rcgen::Issuer::from_params(&client_ca_params, &client_ca_key)).unwrap();

    let mut roots = rustls::RootCertStore::empty();
    roots.add(client_ca.der().clone()).unwrap();
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots)).build().unwrap();
    let server_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert.der().clone()], rustls::pki_types::PrivateKeyDer::try_from(key.serialize_der()).unwrap())
        .unwrap();
    let server_config = Arc::new(server_config);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let config = Arc::clone(&server_config);
            std::thread::spawn(move || {
                let mut tls = rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), tcp);
                let mut buffer = [0u8; 4096];
                if tls.read(&mut buffer).unwrap_or(0) > 0 {
                    let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
                }
            });
        }
    });
    let tls = build(Some(ca.as_bytes()), &validation("abc.example.com", &[])).unwrap();
    assert!(status(&tls, port).is_err(), "no client certificate");
    let with = tls.with_client_cert(client.pem().as_bytes(), client_key.serialize_pem().as_bytes()).unwrap();
    assert_eq!(Ok(200), status(&with, port));
    assert_ne!(tls.label, with.label, "pooled apart");
}
