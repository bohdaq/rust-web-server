use super::*;
use crate::response::STATUS_CODE_REASON_PHRASE;

fn header(response: &Response) -> Option<String> {
    response.headers.iter().find(|h| h.name.eq_ignore_ascii_case("proxy-status")).map(|h| h.value.to_string())
}

fn set(mode: Option<&str>, name: Option<&str>, next_hop: bool) {
    for (key, value) in [("RWS_CONFIG_PROXY_STATUS", mode), ("RWS_CONFIG_PROXY_NAME", name)] {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
    if next_hop {
        std::env::set_var("RWS_CONFIG_PROXY_STATUS_NEXT_HOP", "true");
    } else {
        std::env::remove_var("RWS_CONFIG_PROXY_STATUS_NEXT_HOP");
    }
    crate::server_config::invalidate();
}

#[test]
fn forwarder_failures_map_to_rfc_9209_error_types() {
    for (failure, expected) in [
        ("DNS lookup for nope.invalid:80 failed: failed to lookup address information", "dns_error"),
        ("connect to 10.0.0.5:8080 (10.0.0.5:8080) failed: Connection refused (os error 111)", "connection_refused"),
        ("connect to 10.0.0.5:8080 (10.0.0.5:8080) timed out", "connection_timeout"),
        ("connect to a:1 (10.9.9.9:1) failed: Network is unreachable (os error 101)", "destination_ip_unroutable"),
        ("TLS handshake with a:443 failed: invalid peer certificate: UnknownIssuer", "tls_certificate_error"),
        ("TLS handshake with a:443 failed: received fatal alert: HandshakeFailure", "tls_protocol_error"),
        ("TLS handshake with a:443 timed out", "connection_timeout"),
        ("backend response timed out", "http_response_timeout"),
        ("Resource temporarily unavailable (os error 11)", "http_response_timeout"),
        ("backend closed connection before headers were complete", "connection_terminated"),
        ("Connection reset by peer (os error 104)", "connection_terminated"),
        ("backend closed connection mid-body", "http_response_incomplete"),
        ("too many interim (1xx) responses from the backend", "http_protocol_error"),
        ("invalid upstream address 'x'", "proxy_configuration_error"),
        ("something odd", "proxy_internal_error"),
    ] {
        assert_eq!(expected, ProxyError::classify(failure).as_str(), "{failure}");
    }
}

#[test]
fn the_header_by_mode_name_and_next_hop() {
    let _g = crate::test_env::lock();
    let ok = || Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "hi");
    let bad = || crate::error::problem(STATUS_CODE_REASON_PHRASE.n502_bad_gateway, "");

    set(None, None, false);
    let mut r = ok();
    received(&mut r, Some("10.0.0.5:8080"));
    assert_eq!(Some("rws; received-status=200".to_string()), header(&r));
    let mut r = bad();
    failed_with(&mut r, "connect to b:1 (10.0.0.5:1) failed: Connection refused", Some("10.0.0.5:1"));
    assert_eq!(Some("rws; error=connection_refused".to_string()), header(&r), "no addresses by default");

    set(Some("errors"), Some("edge 1"), true);
    let mut r = ok();
    received(&mut r, Some("10.0.0.5:8080"));
    assert_eq!(None, header(&r), "errors only");
    let mut r = bad();
    r.headers.push(Header::new("Proxy-Status", "inner; error=http_response_timeout".to_string()));
    failed_with(&mut r, "backend said \"no\"\r\nX: y", Some("10.0.0.5:8080"));
    assert_eq!(
        Some(r#"inner; error=http_response_timeout, "edge 1"; error=proxy_internal_error; next-hop="10.0.0.5:8080"; details="backend said \"no\"X: y""#.to_string()),
        header(&r),
        "appended after the upstream's, quoted name, no CR/LF"
    );

    set(Some("off"), None, false);
    let mut r = bad();
    failed(&mut r, ProxyError::DestinationUnavailable, None, None);
    assert_eq!(None, header(&r));
    set(None, None, false);
}
