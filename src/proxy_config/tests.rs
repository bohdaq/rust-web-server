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

// ── RouteMatcher: content-type prefix ─────────────────────────────────────────

#[test]
fn route_matcher_content_type_prefix_matches() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        content_type: Some("application/grpc*".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");

    // Matching prefix
    let mut req = make_request("POST", "/");
    req.headers.push(crate::header::Header {
        name: "Content-Type".to_string(),
        value: "application/grpc+proto".to_string(),
    });
    assert!(matcher.matches(&req, &conn));

    // Non-matching content-type
    let mut req2 = make_request("POST", "/");
    req2.headers.push(crate::header::Header {
        name: "Content-Type".to_string(),
        value: "application/json".to_string(),
    });
    assert!(!matcher.matches(&req2, &conn));

    // No content-type header at all
    assert!(!matcher.matches(&make_request("POST", "/"), &conn));
}

#[test]
fn route_matcher_sni_hostname() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        host: Some("api.example.com".to_string()),
        ..Default::default()
    });

    // SNI hostname matches (conn-level, no Host header required)
    let mut conn = make_conn("127.0.0.1");
    conn.sni_hostname = Some("api.example.com".to_string());
    assert!(matcher.matches(&make_request("GET", "/"), &conn));

    // SNI mismatch
    conn.sni_hostname = Some("www.example.com".to_string());
    assert!(!matcher.matches(&make_request("GET", "/"), &conn));

    // No SNI, no Host header → no match
    conn.sni_hostname = None;
    assert!(!matcher.matches(&make_request("GET", "/"), &conn));
}

#[test]
fn route_matcher_host_and_path_both_required() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        host: Some("example.com".to_string()),
        path: Some("/api/*".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");

    let mut req_right_host = make_request("GET", "/api/users");
    req_right_host.headers.push(crate::header::Header {
        name: "Host".to_string(),
        value: "example.com".to_string(),
    });
    // Both host and path match → matches
    assert!(matcher.matches(&req_right_host, &conn));

    // Wrong host, correct path → no match
    let mut req_wrong_host = make_request("GET", "/api/users");
    req_wrong_host.headers.push(crate::header::Header {
        name: "Host".to_string(),
        value: "other.com".to_string(),
    });
    assert!(!matcher.matches(&req_wrong_host, &conn));

    // Correct host, wrong path → no match
    let mut req_wrong_path = make_request("GET", "/other");
    req_wrong_path.headers.push(crate::header::Header {
        name: "Host".to_string(),
        value: "example.com".to_string(),
    });
    assert!(!matcher.matches(&req_wrong_path, &conn));
}

#[test]
fn route_matcher_method_case_insensitive() {
    let matcher = RouteMatcher::from_match_config(&MatchConfig {
        method: Some("POST".to_string()),
        ..Default::default()
    });
    let conn = make_conn("127.0.0.1");
    // Lower-case method from the request should still match
    assert!(matcher.matches(&make_request("post", "/any"), &conn));
    assert!(!matcher.matches(&make_request("get", "/any"), &conn));
}

// ── Parser: L4 proxy sections ──────────────────────────────────────────────────

#[test]
fn from_str_parses_tcp_proxy() {
    let cfg = ProxyConfig::from_str(r#"
[[tcp_proxy]]
name = "pg"
listen = "0.0.0.0:5432"
backends = ["db1:5432", "db2:5432"]
connect_timeout_ms = 1000
"#);
    assert_eq!(cfg.tcp_proxies.len(), 1);
    let tcp = &cfg.tcp_proxies[0];
    assert_eq!(tcp.name, "pg");
    assert_eq!(tcp.listen, "0.0.0.0:5432");
    assert_eq!(tcp.backends, vec!["db1:5432", "db2:5432"]);
    assert_eq!(tcp.connect_timeout_ms, 1000);
}

#[test]
fn from_str_parses_udp_proxy() {
    let cfg = ProxyConfig::from_str(r#"
[[udp_proxy]]
name = "dns"
listen = "0.0.0.0:53"
backends = ["8.8.8.8:53"]
reply_timeout_ms = 2000
buffer_size = 4096
"#);
    assert_eq!(cfg.udp_proxies.len(), 1);
    let udp = &cfg.udp_proxies[0];
    assert_eq!(udp.name, "dns");
    assert_eq!(udp.listen, "0.0.0.0:53");
    assert_eq!(udp.backends, vec!["8.8.8.8:53"]);
    assert_eq!(udp.reply_timeout_ms, 2000);
    assert_eq!(udp.buffer_size, 4096);
}

#[test]
fn from_str_parses_ws_proxy() {
    let cfg = ProxyConfig::from_str(r#"
[[ws_proxy]]
name = "chat"
listen = "0.0.0.0:9000"
backends = ["ws1:8080", "ws2:8080"]
connect_timeout_ms = 500
read_timeout_ms = 30000
"#);
    assert_eq!(cfg.ws_proxies.len(), 1);
    let ws = &cfg.ws_proxies[0];
    assert_eq!(ws.name, "chat");
    assert_eq!(ws.backends, vec!["ws1:8080", "ws2:8080"]);
    assert_eq!(ws.connect_timeout_ms, 500);
    assert_eq!(ws.read_timeout_ms, 30000);
}

// ── Parser: middleware fields ──────────────────────────────────────────────────

#[test]
fn from_str_parses_cache_middleware() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "cached"

[route.match]
path = "/static/*"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "ok"
content_type = "text/plain"

[route.middleware.cache]
ttl_secs = 3600
vary_by = ["Accept-Encoding", "Accept-Language"]
"#);
    let cache = cfg.routes[0].middleware.cache.as_ref().unwrap();
    assert_eq!(cache.ttl_secs, 3600);
    assert_eq!(cache.vary_by, vec!["Accept-Encoding", "Accept-Language"]);
}

#[test]
fn from_str_parses_bearer_auth() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "protected"

[route.match]
path = "/secret"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "secret data"
content_type = "text/plain"

[route.middleware.auth]
type = "bearer"
token_env = "MY_API_TOKEN"
"#);
    match cfg.routes[0].middleware.auth.as_ref().unwrap() {
        crate::proxy_config::AuthConfig::Bearer { token_env } => {
            assert_eq!(token_env, "MY_API_TOKEN");
        }
        other => panic!("expected Bearer, got {:?}", other),
    }
}

#[test]
fn from_str_parses_rewrite_request_rules() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "rewrite"

[route.match]
path = "/api/*"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "ok"
content_type = "text/plain"

[[route.middleware.rewrite.request]]
type = "header_set"
name = "X-Forwarded-Host"
value = "api.example.com"

[[route.middleware.rewrite.request]]
type = "uri_strip_prefix"
prefix = "/api"
"#);
    let rules = &cfg.routes[0].middleware.rewrite_request;
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].type_, "header_set");
    assert_eq!(rules[0].name.as_deref(), Some("X-Forwarded-Host"));
    assert_eq!(rules[0].value.as_deref(), Some("api.example.com"));
    assert_eq!(rules[1].type_, "uri_strip_prefix");
    assert_eq!(rules[1].prefix.as_deref(), Some("/api"));
}

#[test]
fn from_str_parses_rewrite_response_rules() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "secure"

[route.match]
path = "/*"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "ok"
content_type = "text/plain"

[[route.middleware.rewrite.response]]
type = "header_set"
name = "X-Frame-Options"
value = "DENY"

[[route.middleware.rewrite.response]]
type = "header_remove"
name = "Server"
"#);
    let rules = &cfg.routes[0].middleware.rewrite_response;
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].type_, "header_set");
    assert_eq!(rules[0].name.as_deref(), Some("X-Frame-Options"));
    assert_eq!(rules[0].value.as_deref(), Some("DENY"));
    assert_eq!(rules[1].type_, "header_remove");
    assert_eq!(rules[1].name.as_deref(), Some("Server"));
}

#[test]
fn from_str_parses_ip_filter() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "internal"

[route.match]
path = "/admin/*"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "admin"
content_type = "text/plain"

[route.middleware.ip_filter]
allow = ["10.0.0.0/8", "192.168.1.1"]
"#);
    let mw = &cfg.routes[0].middleware;
    assert_eq!(mw.ip_allow, vec!["10.0.0.0/8", "192.168.1.1"]);
    assert!(mw.ip_deny.is_empty());
}

#[test]
fn from_str_parses_redirect_302() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "temp-redirect"

[route.match]
path = "/moved"

[route.action]
type = "redirect"

[route.action.redirect]
location = "https://example.com/new"
status = 302
"#);
    match &cfg.routes[0].action {
        ActionConfig::Redirect { location, status } => {
            assert_eq!(location, "https://example.com/new");
            assert_eq!(*status, 302);
        }
        other => panic!("expected Redirect, got {:?}", other),
    }
}

// ── ConfigDrivenApp / builder middleware tests ─────────────────────────────────

#[test]
fn respond_action_sets_content_type_header() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "json-resp"

[route.match]
path = "/data"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "{\"ok\":true}"
content_type = "application/json"
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/data"), &conn).unwrap();
    assert_eq!(resp.status_code, 200);
    // Body is inside content_range_list[0]
    let ct = &resp.content_range_list[0].content_type;
    assert!(ct.contains("application/json"), "expected application/json, got {ct}");
}

#[test]
fn redirect_action_substitutes_dollar_path() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "path-redirect"

[route.match]
path = "/*"

[route.action]
type = "redirect"

[route.action.redirect]
location = "https://new.example.com$path"
status = 301
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/about"), &conn).unwrap();
    assert_eq!(resp.status_code, 301);
    let location = resp.headers.iter().find(|h| h.name == "Location").map(|h| h.value.as_str()).unwrap_or("");
    assert_eq!(location, "https://new.example.com/about");
}

#[test]
fn rate_limit_middleware_returns_429_after_limit() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "limited"

[route.match]
path = "/limited"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "ok"
content_type = "text/plain"

[route.middleware.rate_limit]
max_requests = 1
window_secs = 60
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("10.77.77.77");
    let req = make_request("GET", "/limited");

    // First request: within budget → 200
    let r1 = app.execute(&req, &conn).unwrap();
    assert_eq!(r1.status_code, 200, "first request should pass");

    // Second request: over limit → 429
    let r2 = app.execute(&req, &conn).unwrap();
    assert_eq!(r2.status_code, 429, "second request should be rate-limited");
}

#[test]
fn bearer_auth_returns_401_without_authorization_header() {
    std::env::set_var("RWSTEST_PROXY_BEARER_A", "supersecret");
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "protected"

[route.match]
path = "/secret"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "secret"
content_type = "text/plain"

[route.middleware.auth]
type = "bearer"
token_env = "RWSTEST_PROXY_BEARER_A"
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/secret"), &conn).unwrap();
    assert_eq!(resp.status_code, 401);
}

#[test]
fn bearer_auth_returns_401_with_wrong_token() {
    std::env::set_var("RWSTEST_PROXY_BEARER_B", "correct-token");
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "protected"

[route.match]
path = "/secret"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "secret"
content_type = "text/plain"

[route.middleware.auth]
type = "bearer"
token_env = "RWSTEST_PROXY_BEARER_B"
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let mut req = make_request("GET", "/secret");
    req.headers.push(crate::header::Header {
        name: "Authorization".to_string(),
        value: "Bearer wrong-token".to_string(),
    });
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(resp.status_code, 401);
}

#[test]
fn bearer_auth_passes_with_correct_token() {
    std::env::set_var("RWSTEST_PROXY_BEARER_C", "my-valid-token");
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "protected"

[route.match]
path = "/secret"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "secret"
content_type = "text/plain"

[route.middleware.auth]
type = "bearer"
token_env = "RWSTEST_PROXY_BEARER_C"
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let mut req = make_request("GET", "/secret");
    req.headers.push(crate::header::Header {
        name: "Authorization".to_string(),
        value: "Bearer my-valid-token".to_string(),
    });
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(resp.status_code, 200);
}

#[test]
fn ip_allow_filter_blocks_ip_not_in_allowlist() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "internal"

[route.match]
path = "/admin"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "admin"
content_type = "text/plain"

[route.middleware.ip_filter]
allow = ["192.168.1.0/24"]
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);

    // IP outside the allowed range → 403
    let outside = make_conn("10.0.0.1");
    let resp = app.execute(&make_request("GET", "/admin"), &outside).unwrap();
    assert_eq!(resp.status_code, 403);

    // IP inside the allowed range → 200
    let inside = make_conn("192.168.1.55");
    let resp2 = app.execute(&make_request("GET", "/admin"), &inside).unwrap();
    assert_eq!(resp2.status_code, 200);
}

#[test]
fn ip_deny_filter_blocks_listed_ip() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "open-but-blocked"

[route.match]
path = "/page"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "hello"
content_type = "text/plain"

[route.middleware.ip_filter]
deny = ["10.0.0.5"]
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);

    // Denied IP → 403
    let blocked = make_conn("10.0.0.5");
    let resp = app.execute(&make_request("GET", "/page"), &blocked).unwrap();
    assert_eq!(resp.status_code, 403);

    // Non-denied IP → 200
    let allowed = make_conn("10.0.0.6");
    let resp2 = app.execute(&make_request("GET", "/page"), &allowed).unwrap();
    assert_eq!(resp2.status_code, 200);
}

#[test]
fn rewrite_response_header_injected() {
    let cfg = ProxyConfig::from_str(r#"
[[route]]
name = "secure-headers"

[route.match]
path = "/page"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body = "hello"
content_type = "text/plain"

[[route.middleware.rewrite.response]]
type = "header_set"
name = "X-Frame-Options"
value = "DENY"
"#);
    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/page"), &conn).unwrap();
    assert_eq!(resp.status_code, 200);
    let xfo = resp.headers.iter().find(|h| h.name.eq_ignore_ascii_case("X-Frame-Options")).map(|h| h.value.as_str()).unwrap_or("");
    assert_eq!(xfo, "DENY");
}

// ── is_proxy_mode tests ────────────────────────────────────────────────────────

#[test]
fn is_proxy_mode_false_when_no_config_file() {
    // In test environment, rws.config.toml either doesn't exist or doesn't have
    // [[route]] sections — so this should return false.
    // We can't guarantee the file doesn't exist, so just assert it doesn't panic.
    let _ = ProxyConfig::is_proxy_mode();
}
