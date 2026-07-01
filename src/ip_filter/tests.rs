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
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
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
    // IPv6 addresses are not matched; allow mode blocks them.
    assert_eq!(403, status(IpFilter::allow(["192.168.0.0/16"]), "::1"));
    assert_eq!(403, status(IpFilter::allow(["0.0.0.0/0"]), "2001:db8::1"));
}

#[test]
fn deny_mode_ipv6_client_passes() {
    // IPv6 addresses are not matched; deny mode lets them through.
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
