use super::*;
use crate::gateway_api::routes::{Backend, CompiledRule};

fn route(name: &str, created: &str, hostnames: &[&str], backends: Vec<Backend>) -> CompiledHttpRoute {
    CompiledHttpRoute {
        key: ("ns".to_string(), name.to_string()),
        hostnames: hostnames.iter().map(|h| h.to_string()).collect(),
        created: created.to_string(),
        rules: vec![CompiledRule { name: None, matches: vec![], filters: vec![], backends, timeouts: None, retry: None, mirrors: vec![] }],
    }
}

fn backend(weight: u32, endpoints: &[&str]) -> Backend {
    Backend { weight, endpoints: Ok(endpoints.iter().map(|e| e.to_string()).collect()), app_protocol: None, filters: vec![] }
}

#[test]
fn routes_are_picked_by_sni_then_age() {
    let routes = vec![
        route("newer-any", "2026-01-02", &[], vec![]),
        route("older-any", "2026-01-01", &[], vec![]),
        route("wildcard", "2026-01-03", &["*.example.com"], vec![]),
        route("exact", "2026-01-04", &["a.example.com"], vec![]),
    ];
    let name = |sni: Option<&str>| pick_route(&routes, sni).map(|r| r.key.1.clone());
    assert_eq!(Some("exact".to_string()), name(Some("a.example.com")));
    assert_eq!(Some("wildcard".to_string()), name(Some("b.example.com")));
    assert_eq!(Some("older-any".to_string()), name(Some("other.org")), "the oldest route without hostnames");
    assert_eq!(Some("older-any".to_string()), name(None));
    assert_eq!(None, pick_route(&routes[2..], Some("other.org")), "nothing matches");
}

#[test]
fn endpoints_follow_weights_and_need_something_ready() {
    let counter = AtomicUsize::new(0);
    let weighted = route("r", "", &[], vec![backend(3, &["10.0.0.1:1"]), backend(1, &["10.0.0.2:1"])]);
    let picks: Vec<String> = (0..4000).filter_map(|_| pick_endpoint(&weighted, &counter)).collect();
    let first = picks.iter().filter(|p| p.starts_with("10.0.0.1")).count();
    assert!((2700..3300).contains(&first), "{first}");
    assert_eq!(None, pick_endpoint(&route("r", "", &[], vec![backend(0, &["10.0.0.1:1"])]), &counter), "weight 0");
    assert_eq!(None, pick_endpoint(&route("r", "", &[], vec![backend(1, &[])]), &counter), "no ready endpoint");
    assert_eq!(None, pick_endpoint(&route("r", "", &[], vec![]), &counter), "no backends");
}

/// The bytes a rustls client sends first, for `name`.
fn hello_bytes(name: &str) -> Vec<u8> {
    let config = crate::tls_client_builder().with_root_certificates(rustls::RootCertStore::empty()).with_no_client_auth();
    let mut client = rustls::ClientConnection::new(Arc::new(config), name.to_string().try_into().unwrap()).unwrap();
    let mut bytes = Vec::new();
    client.write_tls(&mut bytes).unwrap();
    bytes
}

#[test]
fn the_client_hello_gives_the_sni() {
    let hello = hello_bytes("pass.example.com");
    assert_eq!(Hello::Sni(Some("pass.example.com".to_string())), client_hello(&hello));
    assert_eq!(Hello::NeedMore, client_hello(&hello[..hello.len() / 2]));
    assert_eq!(Hello::NeedMore, client_hello(&[]));
    assert_eq!(Hello::NotTls, client_hello(b"GET / HTTP/1.1\r\n\r\n"));
}
