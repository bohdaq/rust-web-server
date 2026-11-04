use crate::application::Application;
use crate::core::New;
use crate::http::VERSION;
use crate::ip_filter::IpFilter;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

// ── helpers ───────────────────────────────────────────────────────────────────

fn conn(ip: &str) -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: ip.to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    tls: false,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

struct OkApp;
impl Application for OkApp {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(r)
    }
}

fn status(filter: IpFilter, ip: &str) -> i16 {
    filter.handle(&get("/"), &conn(ip), &OkApp).unwrap().status_code
}

// ── allow mode ────────────────────────────────────────────────────────────────

#[test]
fn allow_matching_ip_passes() {
    assert_eq!(200, status(IpFilter::allow(["1.2.3.4"]), "1.2.3.4"));
}

#[test]
fn allow_non_matching_ip_is_blocked() {
    assert_eq!(403, status(IpFilter::allow(["1.2.3.4"]), "9.9.9.9"));
}

#[test]
fn allow_cidr_ip_in_range_passes() {
    assert_eq!(200, status(IpFilter::allow(["192.168.1.0/24"]), "192.168.1.42"));
}

#[test]
fn allow_cidr_ip_outside_range_is_blocked() {
    assert_eq!(403, status(IpFilter::allow(["192.168.1.0/24"]), "192.168.2.1"));
}

#[test]
fn allow_class_a_loopback_range() {
    assert_eq!(200, status(IpFilter::allow(["127.0.0.0/8"]), "127.0.0.1"));
    assert_eq!(200, status(IpFilter::allow(["127.0.0.0/8"]), "127.255.255.255"));
    assert_eq!(403, status(IpFilter::allow(["127.0.0.0/8"]), "128.0.0.1"));
}

#[test]
fn allow_multiple_entries_second_matches() {
    assert_eq!(200, status(IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16"]), "192.168.5.5"));
}

#[test]
fn allow_multiple_entries_none_match_is_blocked() {
    assert_eq!(403, status(IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16"]), "8.8.8.8"));
}

#[test]
fn allow_wildcard_cidr_matches_all_ipv4() {
    assert_eq!(200, status(IpFilter::allow(["0.0.0.0/0"]), "1.2.3.4"));
    assert_eq!(200, status(IpFilter::allow(["0.0.0.0/0"]), "255.255.255.255"));
}

#[test]
fn allow_single_host_cidr() {
    assert_eq!(200, status(IpFilter::allow(["10.0.0.5/32"]), "10.0.0.5"));
    assert_eq!(403, status(IpFilter::allow(["10.0.0.5/32"]), "10.0.0.6"));
}

// ── deny mode ─────────────────────────────────────────────────────────────────

#[test]
fn deny_matching_ip_is_blocked() {
    assert_eq!(403, status(IpFilter::deny(["1.2.3.4"]), "1.2.3.4"));
}

#[test]
fn deny_non_matching_ip_passes() {
    assert_eq!(200, status(IpFilter::deny(["1.2.3.4"]), "9.9.9.9"));
}

#[test]
fn deny_cidr_ip_in_range_is_blocked() {
    assert_eq!(403, status(IpFilter::deny(["10.0.0.0/8"]), "10.1.2.3"));
}

#[test]
fn deny_cidr_ip_outside_range_passes() {
    assert_eq!(200, status(IpFilter::deny(["10.0.0.0/8"]), "11.0.0.1"));
}

// ── edge cases ────────────────────────────────────────────────────────────────

#[test]
fn allow_mode_ipv6_client_is_blocked() {
    // IPv4 rules don't match native IPv6 clients.
    assert_eq!(403, status(IpFilter::allow(["192.168.0.0/16"]), "::1"));
    assert_eq!(403, status(IpFilter::allow(["0.0.0.0/0"]), "2001:db8::1"));
}

#[test]
fn deny_mode_ipv6_client_passes() {
    // IPv4 rules don't match native IPv6 clients.
    assert_eq!(200, status(IpFilter::deny(["1.2.3.4"]), "::1"));
}

#[test]
fn malformed_entry_is_silently_skipped() {
    // "not-an-ip" cannot be parsed, so the list is effectively empty.
    assert_eq!(403, status(IpFilter::allow(["not-an-ip"]), "1.2.3.4"));
}

#[test]
fn prefix_len_over_32_is_skipped() {
    assert_eq!(403, status(IpFilter::allow(["10.0.0.0/33"]), "10.0.0.1"));
}

#[test]
fn cidr_network_is_normalized() {
    // Host bits are masked; "10.0.1.5/8" should still match 10.x.x.x.
    assert_eq!(200, status(IpFilter::allow(["10.0.1.5/8"]), "10.2.3.4"));
}

#[test]
fn empty_allow_list_blocks_everyone() {
    let no_entries: &[&str] = &[];
    assert_eq!(403, status(IpFilter::allow(no_entries.iter().copied()), "1.2.3.4"));
}

#[test]
fn empty_deny_list_passes_everyone() {
    let no_entries: &[&str] = &[];
    assert_eq!(200, status(IpFilter::deny(no_entries.iter().copied()), "1.2.3.4"));
}

// ── integration: middleware stack ─────────────────────────────────────────────

#[test]
fn via_middleware_stack_allow_passes() {
    let app = WithMiddleware::new(OkApp).wrap(IpFilter::allow(["127.0.0.1"]));
    let resp = app.execute(&get("/"), &conn("127.0.0.1")).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn via_middleware_stack_deny_blocks() {
    let app = WithMiddleware::new(OkApp).wrap(IpFilter::deny(["1.2.3.4"]));
    let resp = app.execute(&get("/"), &conn("1.2.3.4")).unwrap();
    assert_eq!(403, resp.status_code);
}

#[test]
fn stacked_allow_and_deny() {
    use crate::app::App;
    // Allow internal network, then deny a specific internal address.
    let app = App::new()
        .wrap(IpFilter::allow(["10.0.0.0/8"]))
        .wrap(IpFilter::deny(["10.0.0.1"]));
    // 10.0.0.2 is allowed by the allowlist and not in the denylist.
    let resp = app.execute(&get("/"), &conn("10.0.0.2")).unwrap();
    assert_ne!(403, resp.status_code);
    // 10.0.0.1 is allowed by the allowlist but blocked by the denylist.
    let resp = app.execute(&get("/"), &conn("10.0.0.1")).unwrap();
    assert_eq!(403, resp.status_code);
    // 8.8.8.8 is blocked by the allowlist before even reaching the denylist.
    let resp = app.execute(&get("/"), &conn("8.8.8.8")).unwrap();
    assert_eq!(403, resp.status_code);
}

#[test]
fn ipv6_exact_and_cidr_rules_match() {
    assert_eq!(200, status(IpFilter::allow(["::1"]), "::1"));
    assert_eq!(200, status(IpFilter::allow(["2001:db8::/32"]), "2001:db8:1234::7"));
    assert_eq!(403, status(IpFilter::allow(["2001:db8::/32"]), "2001:db9::1"));
    assert_eq!(403, status(IpFilter::deny(["2001:db8::/32"]), "2001:db8::1"));
    assert_eq!(200, status(IpFilter::deny(["2001:db8::/32"]), "2001:db9::1"));
}

#[test]
fn ipv4_rule_matches_an_ipv4_mapped_client() {
    // A dual-stack socket reports IPv4 clients as ::ffff:a.b.c.d.
    assert_eq!(403, status(IpFilter::deny(["10.0.0.0/8"]), "::ffff:10.1.2.3"));
    assert_eq!(200, status(IpFilter::allow(["10.0.0.0/8"]), "::ffff:10.1.2.3"));
}

#[test]
fn is_valid_entry_accepts_both_families_and_rejects_junk() {
    for ok in ["1.2.3.4", "10.0.0.0/8", "0.0.0.0/0", "::1", "2001:db8::/32", " 192.168.0.1 "] {
        assert!(super::is_valid_entry(ok), "{ok}");
    }
    for bad in ["", "1.2.3", "10.0.0.0/33", "2001:db8::/129", "example.com", "1.2.3.4/x"] {
        assert!(!super::is_valid_entry(bad), "{bad}");
    }
}

// ── the client behind a trusted proxy (18.27.0) ──────────────────────────────

fn forwarded_for(xff: &str) -> Request {
    let mut r = get("/");
    r.headers.push(crate::header::Header::new("X-Forwarded-For".to_string(), xff.to_string()));
    r
}

#[test]
fn behind_a_trusted_proxy_the_filter_sees_the_client() {
    // The default trusted_proxies has 10/8: the gateway at 10.0.0.1 forwards
    // the client 203.0.113.9 (the client's own "1.1.1.1" doesn't count).
    let deny = IpFilter::deny(["203.0.113.9"]);
    let r = forwarded_for("1.1.1.1, 203.0.113.9");
    assert_eq!(deny.handle(&r, &conn("10.0.0.1"), &OkApp).unwrap().status_code, 403);
    let allow = IpFilter::allow(["203.0.113.0/24"]);
    assert_eq!(allow.handle(&r, &conn("10.0.0.1"), &OkApp).unwrap().status_code, 200);
}

#[test]
fn an_untrusted_peer_cant_claim_another_address() {
    // Straight from the internet: X-Forwarded-For is ignored.
    let allow = IpFilter::allow(["10.0.0.0/8"]);
    let r = forwarded_for("10.0.0.7");
    assert_eq!(allow.handle(&r, &conn("203.0.113.9"), &OkApp).unwrap().status_code, 403);
}

#[test]
fn the_scheme_comes_from_a_trusted_proxy_only() {
    use crate::ip_filter::{request_scheme_with, TrustedProxies};
    let trusted = TrustedProxies::parse("10.0.0.0/8").unwrap();
    let with = |name: &str, value: &str| {
        let mut r = get("/");
        r.headers.push(crate::header::Header::new(name.to_string(), value.to_string()));
        r
    };
    // A TLS connection is https whatever the headers say.
    assert_eq!("https", request_scheme_with(&get("/"), "203.0.113.9", true, &trusted, true));
    // Behind the gateway that terminated TLS.
    assert_eq!("https", request_scheme_with(&with("X-Forwarded-Proto", "https"), "10.0.0.3", false, &trusted, true));
    assert_eq!("https", request_scheme_with(&with("X-Forwarded-Proto", "HTTPS, http"), "10.0.0.3", false, &trusted, true));
    assert_eq!("https", request_scheme_with(&with("Forwarded", "for=203.0.113.9;proto=https, for=10.0.0.1"), "10.0.0.3", false, &trusted, true));
    assert_eq!("http", request_scheme_with(&with("X-Forwarded-Proto", "http"), "10.0.0.3", false, &trusted, true));
    assert_eq!("http", request_scheme_with(&get("/"), "10.0.0.3", false, &trusted, true));
    // A client on the internet can't claim it; nor anyone with client_ip = "peer".
    assert_eq!("http", request_scheme_with(&with("X-Forwarded-Proto", "https"), "203.0.113.9", false, &trusted, true));
    assert_eq!("http", request_scheme_with(&with("X-Forwarded-Proto", "https"), "10.0.0.3", false, &trusted, false));
}
