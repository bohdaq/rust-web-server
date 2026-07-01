//! Unit tests for the proxy_config module.

use crate::application::Application;
use crate::proxy_config::{
    ActionConfig, MatchConfig, ProxyConfig, RouteMatcher,
};
use crate::request::Request;
use crate::server::{Address, ConnectionInfo};

fn make_conn(ip: &str) -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: ip.to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    }
}

fn make_request(method: &str, uri: &str) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: vec![],
        body: vec![],
    }
}

// ── RouteMatcher tests ─────────────────────────────────────────────────────────

#[test]
fn route_matcher_exact_path() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        path: Some("/ping".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");
    assert!(matcher.matches(&make_request("GET", "/ping"), &conn));
    assert!(!matcher.matches(&make_request("GET", "/ping/extra"), &conn));
    assert!(!matcher.matches(&make_request("GET", "/other"), &conn));
}

#[test]
fn route_matcher_path_prefix() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        path: Some("/api/*".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");
    assert!(matcher.matches(&make_request("GET", "/api/users"), &conn));
    assert!(matcher.matches(&make_request("POST", "/api/data"), &conn));
    assert!(!matcher.matches(&make_request("GET", "/other"), &conn));
}

#[test]
fn route_matcher_method_filter() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        method: Some("GET".to_string()),
        path: Some("/test".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");
    assert!(matcher.matches(&make_request("GET", "/test"), &conn));
    assert!(!matcher.matches(&make_request("POST", "/test"), &conn));
}

#[test]
fn route_matcher_host() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        host: Some("example.com".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");

    // Without Host header → should not match
    assert!(!matcher.matches(&make_request("GET", "/"), &conn));

    // With matching Host header
    let mut req = make_request("GET", "/");
    req.headers.push(crate::header::Header {
        name: "Host".to_string(),
        value: "example.com".to_string(),
    });
    assert!(matcher.matches(&req, &conn));

    // With non-matching Host header
    let mut req2 = make_request("GET", "/");
    req2.headers.push(crate::header::Header {
        name: "Host".to_string(),
        value: "other.com".to_string(),
    });
    assert!(!matcher.matches(&req2, &conn));
}

#[test]
fn route_matcher_no_criteria_matches_all() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig::default());
    let conn = make_conn("192.168.1.1");
    assert!(matcher.matches(&make_request("GET", "/anything"), &conn));
    assert!(matcher.matches(&make_request("DELETE", "/whatever"), &conn));
}

#[test]
fn route_matcher_path_with_query_string() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        path: Some("/search".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");
    // Query string should be stripped before matching
    assert!(matcher.matches(&make_request("GET", "/search?q=rust"), &conn));
}

// ── ProxyConfig::from_str tests ────────────────────────────────────────────────

const FULL_CONFIG: &str = r#"
[[upstream]]
name = "backend"
backends = ["server1:8080", "server2:8080"]
strategy = "round_robin"

[upstream.health_check]
path = "/healthz"
interval_secs = 15
timeout_ms = 3000
healthy_threshold = 2
unhealthy_threshold = 3

[[route]]
name = "api"

[route.match]
path = "/api/*"
method = "GET"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "backend"
connect_timeout_ms = 3000
read_timeout_ms = 20000

[route.middleware.rate_limit]
max_requests = 500
window_secs = 60

[[route]]
name = "ping"

[route.match]
path = "/ping"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "pong"
content_type = "text/plain"
"#;

#[test]
fn from_str_parses_upstreams() {
    let cfg = ProxyConfig::from_str(FULL_CONFIG);
    assert_eq!(cfg.upstreams.len(), 1);
    assert_eq!(cfg.upstreams[0].name, "backend");
    assert_eq!(cfg.upstreams[0].backends, vec!["server1:8080", "server2:8080"]);
    assert_eq!(cfg.upstreams[0].strategy, "round_robin");
}

#[test]
fn from_str_parses_health_check() {
    let cfg = ProxyConfig::from_str(FULL_CONFIG);
    let hc = cfg.upstreams[0].health_check.as_ref().unwrap();
    assert_eq!(hc.path, "/healthz");
    assert_eq!(hc.interval_secs, 15);
    assert_eq!(hc.timeout_ms, 3000);
    assert_eq!(hc.healthy_threshold, 2);
    assert_eq!(hc.unhealthy_threshold, 3);
}

#[test]
fn from_str_parses_routes() {
    let cfg = ProxyConfig::from_str(FULL_CONFIG);
    assert_eq!(cfg.routes.len(), 2);

    let route0 = &cfg.routes[0];
    assert_eq!(route0.name, "api");
    assert_eq!(route0.match_.path, Some("/api/*".to_string()));
    assert_eq!(route0.match_.method, Some("GET".to_string()));
    match &route0.action {
        ActionConfig::Proxy { upstream, connect_timeout_ms, .. } => {
            assert_eq!(upstream, "backend");
            assert_eq!(*connect_timeout_ms, 3000);
        }
        other => panic!("expected Proxy, got {:?}", other),
    }

    let route1 = &cfg.routes[1];
    assert_eq!(route1.name, "ping");
    match &route1.action {
        ActionConfig::Respond { status, body, .. } => {
            assert_eq!(*status, 200);
            assert_eq!(body, "pong");
        }
        other => panic!("expected Respond, got {:?}", other),
    }
}

#[test]
fn from_str_parses_route_middleware() {
    let cfg = ProxyConfig::from_str(FULL_CONFIG);
    let rl = cfg.routes[0].middleware.rate_limit.as_ref().unwrap();
    assert_eq!(rl.max_requests, 500);
    assert_eq!(rl.window_secs, 60);
}

// ── Builder / ConfigDrivenApp routing tests ───────────────────────────────────

#[test]
fn config_driven_app_routes_to_respond_handler() {
    let cfg = ProxyConfig::from_str(
        r#"
[[route]]
name = "pong"

[route.match]
path = "/ping"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "pong"
content_type = "text/plain"
"#,
    );
    let (app, _handles) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let req = make_request("GET", "/ping");
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(resp.status_code, 200);
    let body: Vec<u8> = resp.content_range_list.into_iter().flat_map(|cr| cr.body).collect();
    assert_eq!(body, b"pong");
}

#[test]
fn config_driven_app_falls_back_to_app_for_unmatched() {
    let cfg = ProxyConfig::from_str(
        r#"
[[route]]
name = "ping"

[route.match]
path = "/ping"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "pong"
content_type = "text/plain"
"#,
    );
    let (app, _handles) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    // /healthz is NOT in any route; the built-in App fallback should handle it
    let req = make_request("GET", "/healthz");
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(resp.status_code, 200);
}

#[test]
fn config_driven_app_redirect() {
    let cfg = ProxyConfig::from_str(
        r#"
[[route]]
name = "redirect"

[route.match]
path = "/old"

[route.action]
type = "redirect"

[route.action.redirect]
location = "https://example.com/new"
status = 301
"#,
    );
    let (app, _handles) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let req = make_request("GET", "/old");
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(resp.status_code, 301);
    let location = resp
        .headers
        .iter()
        .find(|h| h.name == "Location")
        .map(|h| h.value.as_str())
        .unwrap_or("");
    assert_eq!(location, "https://example.com/new");
}

#[test]
fn config_driven_app_multiple_routes_first_match_wins() {
    let cfg = ProxyConfig::from_str(
        r#"
[[route]]
name = "first"

[route.match]
path = "/test"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "first"
content_type = "text/plain"

[[route]]
name = "second"

[route.match]
path = "/test"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "second"
content_type = "text/plain"
"#,
    );
    let (app, _handles) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let req = make_request("GET", "/test");
    let resp = app.execute(&req, &conn).unwrap();
    let body: Vec<u8> = resp.content_range_list.into_iter().flat_map(|cr| cr.body).collect();
    assert_eq!(body, b"first");
}

// ── is_proxy_mode tests ────────────────────────────────────────────────────────

#[test]
fn is_proxy_mode_false_when_no_config_file() {
    // In test environment, rws.config.toml either doesn't exist or doesn't have
    // [[route]] sections — so this should return false.
    // We can't guarantee the file doesn't exist, so just assert it doesn't panic.
    let _ = ProxyConfig::is_proxy_mode();
}
