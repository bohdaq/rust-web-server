//! Unit tests for the proxy_config module.

use crate::application::Application;
use crate::header::Header;
use crate::proxy_config::{
    ActionConfig, DynamicProxy, MatchConfig, ProxyConfig, RouteMatcher,
};
use crate::request::Request;
use crate::server::{Address, ConnectionInfo};
use crate::server_config::ServerConfig;

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
fn config_driven_app_with_config_pins_fallback_cors_denial() {
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
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(), // no allowed origins -> CORS denied
        ..ServerConfig::default()
    };
    let app = app.with_config(config);

    let conn = make_conn("127.0.0.1");
    // /healthz is not a configured route, so this falls through to the
    // pinned fallback App and should reflect its CORS settings, not env vars.
    let mut req = make_request("GET", "/healthz");
    req.headers.push(Header {
        name: Header::_ORIGIN.to_string(),
        value: "https://evil.example.com".to_string(),
    });

    let resp = app.execute(&req, &conn).unwrap();
    assert!(resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).is_none());
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

// ── StaticAdapter (config-driven `type = "static"` action) tests ───────────────

#[test]
fn static_action_serves_file_from_configured_root() {
    let dir = std::env::temp_dir().join(format!("rws_static_test_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hello.txt"), b"hello from static root").unwrap();

    let cfg = ProxyConfig::from_str(&format!(
        r#"
[[route]]
name = "static-site"

[route.match]
path = "/*"

[route.action]
type = "static"

[route.action.static]
root = "{}"
"#,
        dir.to_str().unwrap()
    ));

    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/hello.txt"), &conn).unwrap();
    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.content_range_list[0].body, b"hello from static root");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn static_action_serves_index_for_directory_request() {
    let dir = std::env::temp_dir().join(format!("rws_static_test_idx_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), b"<h1>home</h1>").unwrap();

    let cfg = ProxyConfig::from_str(&format!(
        r#"
[[route]]
name = "static-site"

[route.match]
path = "/*"

[route.action]
type = "static"

[route.action.static]
root = "{}"
"#,
        dir.to_str().unwrap()
    ));

    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/"), &conn).unwrap();
    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.content_range_list[0].body, b"<h1>home</h1>");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn static_action_rejects_path_traversal() {
    let dir = std::env::temp_dir().join(format!("rws_static_test_trav_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("secret.txt"), b"root-only").unwrap();
    // File outside the configured root that must never be reachable through it.
    let outside = std::env::temp_dir().join(format!("rws_static_outside_{}.txt", std::process::id()));
    std::fs::write(&outside, b"outside secret").unwrap();

    let cfg = ProxyConfig::from_str(&format!(
        r#"
[[route]]
name = "static-site"

[route.match]
path = "/*"

[route.action]
type = "static"

[route.action.static]
root = "{}"
"#,
        dir.to_str().unwrap()
    ));

    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let traversal_uri = format!("/../{}", outside.file_name().unwrap().to_str().unwrap());
    let resp = app.execute(&make_request("GET", &traversal_uri), &conn).unwrap();
    assert_eq!(resp.status_code, 403);

    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(&outside).ok();
}

#[test]
fn static_action_missing_file_is_404() {
    let dir = std::env::temp_dir().join(format!("rws_static_test_404_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let cfg = ProxyConfig::from_str(&format!(
        r#"
[[route]]
name = "static-site"

[route.match]
path = "/*"

[route.action]
type = "static"

[route.action.static]
root = "{}"
"#,
        dir.to_str().unwrap()
    ));

    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("127.0.0.1");
    let resp = app.execute(&make_request("GET", "/nope.txt"), &conn).unwrap();
    assert_eq!(resp.status_code, 404);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn is_proxy_mode_false_when_no_config_file() {
    // In test environment, rws.config.toml either doesn't exist or doesn't have
    // [[route]] sections — so this should return false.
    // We can't guarantee the file doesn't exist, so just assert it doesn't panic.
    let _ = ProxyConfig::is_proxy_mode();
}

// ── DynamicProxy load-balancing strategy tests ──────────────────────────────────

fn two_backends() -> std::sync::Arc<std::sync::RwLock<Vec<String>>> {
    std::sync::Arc::new(std::sync::RwLock::new(vec![
        "127.0.0.1:1".to_string(),
        "127.0.0.1:2".to_string(),
    ]))
}

#[test]
fn dynamic_proxy_round_robin_cycles_backends() {
    let dp = DynamicProxy::new(two_backends(), 100, 100, None, None, false, "round_robin".to_string());
    let first = dp.next_backend("10.0.0.1").unwrap();
    let second = dp.next_backend("10.0.0.1").unwrap();
    let third = dp.next_backend("10.0.0.1").unwrap();
    assert_ne!(first, second);
    assert_eq!(first, third);
}

#[test]
fn dynamic_proxy_default_strategy_is_round_robin() {
    // Empty/unset `strategy` must behave exactly like `strategy = "round_robin"`.
    let dp = DynamicProxy::new(two_backends(), 100, 100, None, None, false, String::new());
    let first = dp.next_backend("10.0.0.1").unwrap();
    let second = dp.next_backend("10.0.0.1").unwrap();
    assert_ne!(first, second);
}

#[test]
fn dynamic_proxy_unknown_strategy_falls_back_to_round_robin() {
    let dp = DynamicProxy::new(two_backends(), 100, 100, None, None, false, "not-a-real-strategy".to_string());
    let first = dp.next_backend("10.0.0.1").unwrap();
    let second = dp.next_backend("10.0.0.1").unwrap();
    assert_ne!(first, second);
}

#[test]
fn dynamic_proxy_ip_hash_is_sticky_per_client() {
    let dp = DynamicProxy::new(two_backends(), 100, 100, None, None, false, "ip_hash".to_string());
    let a1 = dp.next_backend("10.0.0.5").unwrap();
    let a2 = dp.next_backend("10.0.0.5").unwrap();
    let a3 = dp.next_backend("10.0.0.5").unwrap();
    assert_eq!(a1, a2);
    assert_eq!(a2, a3);
}

#[test]
fn dynamic_proxy_random_always_picks_a_live_backend() {
    let live = two_backends();
    let dp = DynamicProxy::new(live.clone(), 100, 100, None, None, false, "random".to_string());
    for _ in 0..20 {
        let backend = dp.next_backend("10.0.0.1").unwrap();
        assert!(live.read().unwrap().contains(&backend));
    }
}

#[test]
fn dynamic_proxy_least_connections_avoids_busy_backend() {
    let dp = DynamicProxy::new(two_backends(), 100, 100, None, None, false, "least_connections".to_string());

    // Mark "127.0.0.1:1" as already having 3 in-flight connections.
    let busy_counter = dp.connection_counter("127.0.0.1:1");
    busy_counter.fetch_add(3, std::sync::atomic::Ordering::Relaxed);

    let chosen = dp.next_backend("10.0.0.1").unwrap();
    assert_eq!(chosen, "127.0.0.1:2");
}

/// Spawns a backend that accepts up to `max_conns` sequential TCP connections
/// (each request gets a fresh connection, matching `proxy_http1`'s no-pooling
/// behavior for `DynamicProxy`) and replies with a body of `tag` each time.
fn spawn_tagged_backend(tag: &'static str, max_conns: usize) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..max_conns {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                    tag.len(),
                    tag
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    port
}

#[test]
fn config_driven_app_ip_hash_strategy_is_sticky_end_to_end() {
    let port_a = spawn_tagged_backend("A", 5);
    let port_b = spawn_tagged_backend("B", 5);

    let cfg = ProxyConfig::from_str(&format!(
        r#"
[[upstream]]
name = "backend"
backends = ["127.0.0.1:{port_a}", "127.0.0.1:{port_b}"]
strategy = "ip_hash"

[[route]]
name = "api"

[route.match]
path = "/*"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "backend"
"#
    ));

    let (app, _) = crate::proxy_config::builder::build(cfg);
    let conn = make_conn("10.0.0.42");

    let first = app.execute(&make_request("GET", "/x"), &conn).unwrap();
    let second = app.execute(&make_request("GET", "/y"), &conn).unwrap();

    let body_of = |resp: &crate::response::Response| -> Vec<u8> {
        resp.content_range_list.iter().flat_map(|c| c.body.iter().copied()).collect()
    };

    assert_eq!(first.status_code, 200);
    assert_eq!(second.status_code, 200);
    assert_eq!(body_of(&first), body_of(&second));
}
