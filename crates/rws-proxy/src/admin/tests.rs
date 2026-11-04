use super::*;
use crate::config_check::{check_str, check_str_offline};
use std::io::{Read, Write};

fn get(api: &AdminApi, path: &str, headers: &str) -> Response {
    let raw = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:9901\r\n{headers}\r\n");
    api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap()
}

fn body(response: &Response) -> String {
    response.content_range_list.first().map(|p| String::from_utf8_lossy(&p.body).to_string()).unwrap_or_default()
}

#[test]
fn every_request_needs_the_token() {
    let api = AdminApi::new("s3cret");
    let refused = get(&api, "/admin/api/v1/status", "");
    assert_eq!(refused.status_code, 401);
    assert!(refused.headers.iter().any(|h| h.name.eq_ignore_ascii_case("www-authenticate")));
    assert_eq!(get(&api, "/admin/api/v1/status", "Authorization: Bearer wrong\r\n").status_code, 401);
    assert_eq!(get(&api, "/admin/api/v1/status", "Authorization: Basic czNjcmV0\r\n").status_code, 401);
    assert_eq!(get(&api, "/nothing", "").status_code, 401, "even unknown paths say nothing without the token");
    assert_eq!(get(&api, "/admin/api/v1/status", "Authorization: Bearer s3cret\r\n").status_code, 200);
}

#[test]
fn status_reports_the_build_and_the_listeners() {
    let api = AdminApi::new("t");
    let response = get(&api, "/admin/api/v1/status", "Authorization: Bearer t\r\n");
    let text = body(&response);
    assert!(text.contains(&format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION"))), "{text}");
    for key in ["\"pid\":", "\"uptime_secs\":", "\"ready\":", "\"features\":[", "\"listeners\":{\"main\":"] {
        assert!(text.contains(key), "{key} missing in {text}");
    }
    assert!(crate::mcp::json_rpc::object_members(&text).is_some(), "valid JSON object: {text}");
    assert!(response.headers.iter().any(|h| h.name == "Cache-Control" && h.value == "no-store"));
}

#[test]
fn unknown_routes_and_methods() {
    let api = AdminApi::new("t");
    let auth = "Authorization: Bearer t\r\n";
    assert_eq!(get(&api, "/admin/api/v1/nope", auth).status_code, 404);
    assert_eq!(get(&api, "/other", auth).status_code, 404);
    let raw = "POST /admin/api/v1/status HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer t\r\nContent-Length: 0\r\n\r\n";
    assert_eq!(api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap().status_code, 405);
}

#[test]
fn browsers_on_other_origins_are_refused() {
    let api = AdminApi::new("t").allowed_origins(vec!["https://ops.example.com".into()]);
    let auth = "Authorization: Bearer t\r\n";
    assert_eq!(get(&api, "/admin/api/v1/status", &format!("{auth}Origin: https://evil.example\r\n")).status_code, 403);
    assert_eq!(get(&api, "/admin/api/v1/status", &format!("{auth}Origin: http://127.0.0.1:9901\r\n")).status_code, 200, "its own origin");
    assert_eq!(get(&api, "/admin/api/v1/status", &format!("{auth}Origin: https://ops.example.com/\r\n")).status_code, 200, "an allowed one");
}

#[test]
fn config_from_env_and_the_token() {
    let _g = crate::test_env::lock();
    std::env::set_var(Config::RWS_CONFIG_ADMIN_LISTEN, " 127.0.0.1:0 ");
    std::env::remove_var(Config::RWS_CONFIG_ADMIN_TOKEN_ENV);
    std::env::set_var(Config::RWS_CONFIG_ADMIN_ALLOWED_ORIGINS, "https://a.example/, https://b.example");
    let config = AdminConfig::from_env();
    assert_eq!(config.listen, "127.0.0.1:0");
    assert_eq!(config.token_env, DEFAULT_TOKEN_ENV);
    assert_eq!(config.allowed_origins, ["https://a.example", "https://b.example"]);
    std::env::set_var(Config::RWS_CONFIG_ADMIN_TOKEN_ENV, "RWS_TEST_ADMIN_TOKEN");
    std::env::remove_var("RWS_TEST_ADMIN_TOKEN");
    let config = AdminConfig::from_env();
    assert!(config.token().unwrap_err().contains("RWS_TEST_ADMIN_TOKEN"));
    assert!(start(&config).unwrap_err().contains("unset or empty"), "no token, no listener");
    for name in [Config::RWS_CONFIG_ADMIN_LISTEN, Config::RWS_CONFIG_ADMIN_TOKEN_ENV, Config::RWS_CONFIG_ADMIN_ALLOWED_ORIGINS] {
        std::env::remove_var(name);
    }
    assert!(!AdminConfig::from_env().enabled());
    assert_eq!(start(&AdminConfig::from_env()), Ok(None));
}

#[test]
fn the_listener_serves_the_api_over_tcp() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_TEST_ADMIN_TCP_TOKEN", "tcp-token");
    let config = AdminConfig { listen: "127.0.0.1:0".into(), token_env: "RWS_TEST_ADMIN_TCP_TOKEN".into(), ..AdminConfig::default() };
    let addr = start(&config).unwrap().unwrap();
    std::env::remove_var("RWS_TEST_ADMIN_TCP_TOKEN");
    let call = |token: &str| {
        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        write!(stream, "GET /admin/api/v1/status HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n").unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    };
    let ok = call("tcp-token");
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains(&format!(r#""admin":"127.0.0.1:{}""#, addr.port())), "{ok}");
    assert!(call("nope").starts_with("HTTP/1.1 401"));
}

#[test]
fn rws_check_reports_admin_mistakes() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_TEST_ADMIN_CHECK");
    let good = "[admin]\nlisten = \"127.0.0.1:9901\"\ntoken_env = \"RWS_TEST_ADMIN_CHECK\"\n";
    assert!(check_str_offline(good).errors.is_empty(), "{:?}", check_str_offline(good).errors);
    assert!(check_str(good).errors.iter().any(|e| e.contains("RWS_TEST_ADMIN_CHECK") && e.contains("won't start")), "{:?}", check_str(good).errors);
    std::env::set_var("RWS_TEST_ADMIN_CHECK", "x");
    assert!(check_str(good).errors.is_empty(), "{:?}", check_str(good).errors);
    std::env::remove_var("RWS_TEST_ADMIN_CHECK");

    let public = "[admin]\nlisten = \"0.0.0.0:9901\"\n";
    assert!(check_str_offline(public).warnings.iter().any(|w| w.contains("reachable from other hosts")));
    let bad = "[admin]\nlisten = \"localhost:9901\"\nallowed_origins = [\"ops.example.com\", \"https://ok.example\"]\n";
    let errors = check_str_offline(bad).errors.join("\n");
    assert!(errors.contains("expected host:port") && errors.contains("\"ops.example.com\" is not an origin") && !errors.contains("ok.example"), "{errors}");
    let off = "[admin]\ntoken_env = \"X\"\n";
    assert!(check_str_offline(off).warnings.iter().any(|w| w.contains("no listen")));
    assert!(check_str_offline("[admin]\nlisten = \"127.0.0.1:1\"\ntypo = 1\n").errors.iter().any(|e| e.contains("typo")));
}

fn call(api: &AdminApi, method: &str, path: &str, body: &str) -> (u16, String) {
    let raw = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:9901\r\nAuthorization: Bearer t\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let response = api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
    (response.status_code as u16, body_of(&response))
}

fn body_of(response: &Response) -> String {
    body(response)
}

#[test]
fn config_hides_secrets() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_TEST_ADMIN_DB_PASSWORD", "hunter2");
    std::env::set_var("RWS_CONFIG_TEST_ADMIN_PLAIN", "visible \"quoted\"");
    let (status, text) = call(&AdminApi::new("t"), "GET", "/admin/api/v1/config", "");
    std::env::remove_var("RWS_CONFIG_TEST_ADMIN_DB_PASSWORD");
    std::env::remove_var("RWS_CONFIG_TEST_ADMIN_PLAIN");
    assert_eq!(status, 200);
    assert!(text.contains(r#""RWS_CONFIG_TEST_ADMIN_DB_PASSWORD":"(hidden)""#) && !text.contains("hunter2"), "{text}");
    assert!(text.contains(r#""RWS_CONFIG_TEST_ADMIN_PLAIN":"visible \"quoted\"""#), "{text}");
    assert!(crate::mcp::json_rpc::object_members(&text).is_some());
}

#[test]
fn maintenance_blocklist_and_flags_change_the_running_server() {
    let _g = crate::test_env::lock();
    let api = AdminApi::new("t");
    assert_eq!(call(&api, "PUT", "/admin/api/v1/maintenance", r#"{"active":true}"#), (200, r#"{"active":true}"#.to_string()));
    assert!(crate::maintenance::MAINTENANCE_MODE.load(Ordering::SeqCst));
    assert_eq!(call(&api, "PUT", "/admin/api/v1/maintenance", r#"{"active":false}"#).1, r#"{"active":false}"#);
    assert_eq!(call(&api, "PUT", "/admin/api/v1/maintenance", r#"{"active":"yes"}"#).0, 400);

    assert_eq!(call(&api, "POST", "/admin/api/v1/blocklist", r#"{"ip":"203.0.113.77"}"#).0, 200);
    assert!(crate::blocklist::global().is_blocked("203.0.113.77"));
    assert!(call(&api, "GET", "/admin/api/v1/blocklist", "").1.contains("\"203.0.113.77\""));
    assert_eq!(call(&api, "POST", "/admin/api/v1/blocklist", r#"{"ip":"not-an-ip"}"#).0, 400);
    assert_eq!(call(&api, "DELETE", "/admin/api/v1/blocklist/2001:db8::7", "").0, 200);
    assert_eq!(call(&api, "DELETE", "/admin/api/v1/blocklist/203.0.113.77", "").0, 200);
    assert!(!crate::blocklist::global().is_blocked("203.0.113.77"));

    assert_eq!(call(&api, "PUT", "/admin/api/v1/flags/admin-test-flag", r#"{"enabled":true}"#).0, 200);
    assert!(crate::feature::global().is_enabled("admin-test-flag"));
    assert!(call(&api, "GET", "/admin/api/v1/flags", "").1.contains(r#"{"name":"admin-test-flag","enabled":true}"#));
    assert_eq!(call(&api, "PUT", "/admin/api/v1/flags/admin-test-flag", "{}").0, 400);
    crate::feature::global().set("admin-test-flag", false);
}

#[test]
fn reads_rate_limits_cors_metrics_requests_and_routes() {
    let api = AdminApi::new("t");
    for path in ["rate-limit", "cors", "metrics", "build", "requests?n=3", "errors", "routes", "maintenance"] {
        let (status, text) = call(&api, "GET", &format!("/admin/api/v1/{path}"), "");
        assert_eq!(status, 200, "{path}: {text}");
        assert!(text.starts_with('{') || text.starts_with('['), "{path}: {text}");
    }
    assert!(call(&api, "GET", "/admin/api/v1/rate-limit/check?ip=198.51.100.9", "").1.contains(r#""ip":"198.51.100.9""#));
    assert_eq!(call(&api, "GET", "/admin/api/v1/rate-limit/check", "").0, 400);
    assert_eq!(call(&api, "POST", "/admin/api/v1/metrics", "").0, 405);
}

#[test]
fn the_cache_and_routes_come_from_whoever_registered_them() {
    let _g = crate::test_env::lock();
    let api = AdminApi::new("t");
    let cache = crate::cache::CacheLayer::memory(10).ttl(60);
    ops::register_cache(cache.clone());
    ops::register_routes(vec![("GET".into(), "/api/version".into())]);
    assert_eq!(call(&api, "GET", "/admin/api/v1/cache", "").1, r#"{"hits":0,"misses":0,"size":0}"#);
    assert_eq!(call(&api, "DELETE", "/admin/api/v1/cache", "").1, r#"{"cleared":true}"#);
    assert_eq!(call(&api, "GET", "/admin/api/v1/routes", "").1, r#"[{"method":"GET","pattern":"/api/version"}]"#);
}

#[test]
fn reload_waits_for_the_reload_and_reports_its_errors() {
    let _g = crate::test_env::lock();
    let ready = crate::metrics::SERVER_READY.load(Ordering::SeqCst);
    let api = AdminApi::new("t").reloader(
        || {
            std::thread::spawn(|| {
                crate::config_reload::on_reload_test_error();
            });
        },
        Duration::from_secs(5),
    );
    crate::metrics::SERVER_READY.store(false, Ordering::SeqCst);
    assert_eq!(call(&api, "POST", "/admin/api/v1/reload", "").0, 503, "not before the server runs");
    crate::metrics::SERVER_READY.store(true, Ordering::SeqCst);
    let (status, text) = call(&api, "POST", "/admin/api/v1/reload", "");
    assert_eq!(status, 200, "{text}");
    assert!(text.contains(r#""applied":false"#) && text.contains("test reload error"), "{text}");

    let never = AdminApi::new("t").reloader(|| {}, Duration::from_millis(100));
    assert_eq!(call(&never, "POST", "/admin/api/v1/reload", "").0, 504);
    crate::metrics::SERVER_READY.store(ready, Ordering::SeqCst);
}

/// A backend answering every request with its own name.
fn named_backend(name: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{name}", name.len());
        }
    });
    format!("http://{addr}")
}

#[test]
fn a_drained_backend_gets_no_new_requests_and_draining_all_gives_503() {
    let (a, b) = (named_backend("a"), named_backend("b"));
    let upstream = format!("drain-test-{}", std::process::id());
    let toml = format!(
        "[[upstream]]\nname = \"{upstream}\"\nbackends = [\"{a}\", \"{b}\"]\n\n[[route]]\n[route.match]\npath = \"/\"\n[route.action]\ntype = \"proxy\"\nupstream = \"{upstream}\"\n"
    );
    let (app, _handles) = crate::proxy_config::builder::build(crate::proxy_config::ProxyConfig::from_str(&toml));
    let fetch = || {
        let raw = "GET / HTTP/1.1\r\nHost: x\r\n\r\n";
        let response = app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
        (response.status_code, body(&response))
    };
    let seen: std::collections::HashSet<String> = (0..6).map(|_| fetch().1).collect();
    assert_eq!(seen.len(), 2, "both backends in rotation: {seen:?}");

    crate::proxy_config::upstream::set_drained(&upstream, &a, true);
    for _ in 0..6 {
        assert_eq!(fetch(), (200, "b".to_string()));
    }
    crate::proxy_config::upstream::set_drained(&upstream, &b, true);
    assert_eq!(fetch().0, 503, "everything drained");
    crate::proxy_config::upstream::set_drained(&upstream, &a, false);
    crate::proxy_config::upstream::set_drained(&upstream, &b, false);
    assert!(!crate::proxy_config::upstream::is_drained(&upstream, &a));
}

#[test]
fn upstreams_and_certificates_endpoints() {
    let api = AdminApi::new("t");
    let (status, text) = call(&api, "GET", "/admin/api/v1/upstreams", "");
    assert_eq!(status, 200);
    assert!(text.starts_with('['), "{text}");
    let (status, text) = call(&api, "PUT", "/admin/api/v1/upstreams/no-such-upstream-x/drain", r#"{"backend":"http://h:1","drained":true}"#);
    assert_eq!(status, 404, "{text}");
    assert!(text.contains("no upstream"), "{text}");
    assert_eq!(call(&api, "PUT", "/admin/api/v1/upstreams/u/drain", r#"{"drained":true}"#).0, 400);
    assert_eq!(call(&api, "GET", "/admin/api/v1/upstreams/u/drain", "").0, 405);

    let (status, text) = call(&api, "GET", "/admin/api/v1/certificates", "");
    assert_eq!(status, 200);
    assert!(text.starts_with(r#"{"certificates":["#) && text.contains(r#""acme":{"running":"#), "{text}");
    let (status, _) = call(&api, "POST", "/admin/api/v1/certificates/renew", "");
    assert!(status == 404 || status == 202, "{status}");
}

#[test]
fn recent_log_lines_and_a_live_follower() {
    let _g = crate::test_env::lock();
    let api = AdminApi::new("t");
    let marker = format!("tail-test-{}", std::process::id());
    crate::log::tail::record(format!("{{\"path\":\"/{marker}-1\"}}\n{{\"path\":\"/{marker}-2\"}}\n").as_bytes());
    let (status, text) = call(&api, "GET", "/admin/api/v1/logs?n=500", "");
    assert_eq!(status, 200);
    assert!(text.contains(&format!(r#""{{\"path\":\"/{marker}-1\"}}""#)), "lines as JSON strings: {text}");

    let raw = "GET /admin/api/v1/logs/stream HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer t\r\n\r\n";
    let mut response = api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
    assert_eq!(response.status_code, 200);
    let mut pipe = response.stream_pipe.take().expect("an event stream");
    crate::log::tail::record(format!("live-{marker}\n").as_bytes());
    let mut seen = String::new();
    let mut buf = [0u8; 4096];
    while !seen.contains(&format!("data: live-{marker}")) {
        let n = pipe.read(&mut buf).unwrap();
        assert!(n > 0, "stream ended: {seen}");
        seen.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
}

#[test]
fn long_lines_are_cut_and_the_buffer_is_bounded() {
    let _g = crate::test_env::lock();
    let _ = AdminApi::new("t");
    let long = format!("cut-{}{}", std::process::id(), "é".repeat(crate::log::tail::MAX_LINE));
    crate::log::tail::record(format!("{long}\n").as_bytes());
    let kept = crate::log::tail::recent(crate::log::tail::CAPACITY);
    let line = kept.iter().find(|l| l.starts_with(&format!("cut-{}", std::process::id()))).expect("kept");
    assert!(line.len() <= crate::log::tail::MAX_LINE);
    for i in 0..crate::log::tail::CAPACITY + 10 {
        crate::log::tail::record(format!("fill-{i}\n").as_bytes());
    }
    assert_eq!(crate::log::tail::recent(usize::MAX).len(), crate::log::tail::CAPACITY);
}

#[test]
fn the_ui_is_served_without_the_token_and_locked_down() {
    let api = AdminApi::new("t");
    let page = get(&api, "/admin/", "");
    assert_eq!(page.status_code, 200);
    assert!(body(&page).contains("<rws-admin>"), "{}", body(&page));
    let csp = page.headers.iter().find(|h| h.name == "Content-Security-Policy").map(|h| h.value.to_string()).unwrap_or_default();
    assert!(csp.contains("script-src 'self'") && csp.contains("frame-ancestors 'none'") && !csp.contains("script-src 'self' 'unsafe"), "{csp}");
    assert!(page.headers.iter().any(|h| h.name == "X-Content-Type-Options" && h.value == "nosniff"));
    for (path, kind) in [("/admin/admin.js", "text/javascript"), ("/admin/admin.css", "text/css")] {
        let file = get(&api, path, "");
        assert_eq!(file.status_code, 200, "{path}");
        assert!(file.content_range_list[0].content_type.starts_with(kind), "{path}");
        assert!(!file.content_range_list[0].body.is_empty(), "{path} is built");
    }
    let redirect = get(&api, "/admin", "");
    assert_eq!(redirect.status_code, 308);
    assert!(redirect.headers.iter().any(|h| h.name == "Location" && h.value == "/admin/"));
    assert_eq!(get(&api, "/admin/other.js", "").status_code, 401, "only the UI's own files are public");
    let raw = "POST /admin/ HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n";
    assert_eq!(api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap().status_code, 401);
}

#[test]
fn traffic_is_404_while_off_and_a_summary_while_on() {
    let _g = crate::test_env::lock();
    let api = AdminApi::new("t");
    crate::traffic::record::disable_for_tests();
    let (status, text) = call(&api, "GET", "/admin/api/v1/traffic", "");
    assert_eq!(status, 404, "{text}");
    crate::traffic::enable();
    let (status, text) = call(&api, "GET", "/admin/api/v1/traffic?hours=168&host=example.com", "");
    assert_eq!(status, 200, "{text}");
    assert!(text.contains(r#""hours":168"#) && text.contains(r#""bucket":"day""#) && text.contains(r#""host":"example.com""#), "{text}");
    assert!(text.contains(r#""totals":{"people":"#), "{text}");
    assert_eq!(call(&api, "POST", "/admin/api/v1/traffic", "").0, 405);
}

// ── 18.22.0: users, roles, audit log ──────────────────────────────────────

fn call_as(api: &AdminApi, token: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let raw = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:9901\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let response = api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
    (response.status_code as u16, body_of(&response))
}

fn team() -> AdminApi {
    let env = |name: &str| match name {
        "RWS_TEST_VIEW" => Some("viewer-token-0123456789".to_string()),
        "RWS_TEST_OPS" => Some("operator-token-0123456789".to_string()),
        _ => None,
    };
    let users = access::parse_users("grafana:RWS_TEST_VIEW:viewer, deploy-bot:RWS_TEST_OPS:operator", &env).unwrap();
    AdminApi::new("admin-token").users(users)
}

#[test]
fn users_parse_and_bad_entries_are_named() {
    let env = |name: &str| (name == "SET").then(|| "a-long-enough-token-1".to_string());
    let users = access::parse_users("ci:SET:Operator", &env).unwrap();
    assert_eq!(users, [access::TokenUser { name: "ci".into(), token: "a-long-enough-token-1".into(), role: access::Role::Operator }]);
    assert!(access::parse_users("", &env).unwrap().is_empty());
    assert!(access::parse_users("ci:SET", &env).unwrap_err().contains("name:ENV_VAR:role"));
    assert!(access::parse_users("ci:SET:root", &env).unwrap_err().contains("viewer, operator or admin"));
    assert!(access::parse_users("ci:UNSET:viewer", &env).unwrap_err().contains("UNSET is unset"));
    let short = |_: &str| Some("short".to_string());
    assert!(access::parse_users("ci:X:viewer", &short).unwrap_err().contains("shorter than 16"));
}

#[test]
fn role_maps_match_groups_emails_and_anyone() {
    let map = access::parse_role_map("sre=operator, rws-admins=admin, ann@example.com=admin, *=viewer").unwrap();
    assert_eq!(access::role_for(&map, None, &["sre".to_string()]), Some(access::Role::Operator));
    assert_eq!(access::role_for(&map, Some("ANN@example.com"), &[]), Some(access::Role::Admin));
    assert_eq!(access::role_for(&map, None, &["sre".into(), "rws-admins".into()]), Some(access::Role::Admin), "the highest match");
    assert_eq!(access::role_for(&map, None, &[]), Some(access::Role::Viewer));
    let strict = access::parse_role_map("sre=operator").unwrap();
    assert_eq!(access::role_for(&strict, Some("x@y.z"), &["dev".into()]), None);
    assert!(access::parse_role_map("sre").is_err());
    assert!(access::parse_role_map("sre=god").is_err());
}

#[test]
fn roles_decide_what_each_user_may_do() {
    let _g = crate::test_env::lock();
    let api = team();
    // Everyone reads.
    for token in ["viewer-token-0123456789", "operator-token-0123456789", "admin-token"] {
        assert_eq!(call_as(&api, token, "GET", "/admin/api/v1/maintenance", "").0, 200);
    }
    // The viewer can't change anything.
    let (status, text) = call_as(&api, "viewer-token-0123456789", "PUT", "/admin/api/v1/maintenance", r#"{"active":true}"#);
    assert_eq!(status, 403);
    assert!(text.contains("grafana has role viewer; this needs operator"), "{text}");
    // The operator changes the server, but not authorization tuples.
    assert_eq!(call_as(&api, "operator-token-0123456789", "PUT", "/admin/api/v1/maintenance", r#"{"active":false}"#).0, 200);
    assert_eq!(call_as(&api, "operator-token-0123456789", "POST", "/admin/api/v1/authz/tuples", r#"{"tuple":"doc:1#viewer@user:ann"}"#).0, 403);
    // whoami
    let (status, text) = call_as(&api, "operator-token-0123456789", "GET", "/admin/api/v1/whoami", "");
    assert_eq!((status, text.as_str()), (200, r#"{"user":"deploy-bot","role":"operator","via":"token"}"#));
    assert_eq!(call_as(&api, "admin-token", "GET", "/admin/api/v1/whoami", "").1, r#"{"user":"admin","role":"admin","via":"token"}"#);
    assert_eq!(call_as(&api, "nobody-token", "GET", "/admin/api/v1/whoami", "").0, 401);
}

#[test]
fn changes_and_refusals_are_audited_with_secrets_redacted() {
    let _g = crate::test_env::lock();
    let api = team();
    let marker = format!("203.0.113.{}", std::process::id() % 200 + 1);
    call_as(&api, "operator-token-0123456789", "POST", "/admin/api/v1/blocklist", &format!(r#"{{"ip":"{marker}","api_token":"hunter2"}}"#));
    call_as(&api, "viewer-token-0123456789", "DELETE", &format!("/admin/api/v1/blocklist/{marker}"), "");
    call_as(&api, "wrong-token-zzzzzzzzzz", "DELETE", &format!("/admin/api/v1/blocklist/{marker}"), "");
    call_as(&api, "operator-token-0123456789", "GET", "/admin/api/v1/status", "");
    let (status, audit) = call_as(&api, "viewer-token-0123456789", "GET", "/admin/api/v1/audit?n=1000", "");
    assert_eq!(status, 200);
    let entries: Vec<&str> = audit.split("},{").filter(|e| e.contains(&marker)).collect();
    assert_eq!(entries.len(), 3, "{audit}");
    let block = entries.iter().find(|e| e.contains(r#""method":"POST""#)).unwrap();
    assert!(block.contains(r#""user":"deploy-bot""#) && block.contains(r#""status":200"#) && block.contains(r#""via":"token""#), "{block}");
    assert!(block.contains("(redacted)") && !block.contains("hunter2"), "{block}");
    assert!(entries.iter().any(|e| e.contains(r#""user":"grafana""#) && e.contains(r#""status":403"#)));
    assert!(entries.iter().any(|e| e.contains(r#""user":"(bad token)""#) && e.contains(r#""status":401"#)));
    assert!(!audit.contains(r#""path":"/admin/api/v1/status""#), "reads aren't audited");
    call_as(&api, "admin-token", "DELETE", &format!("/admin/api/v1/blocklist/{marker}"), "");
}

#[test]
fn the_audit_log_is_appended_to_a_file() {
    let _g = crate::test_env::lock();
    let file = crate::test_env::TempPath::new("admin-audit.log");
    access::set_audit_file(file.as_str());
    call_as(&team(), "operator-token-0123456789", "PUT", "/admin/api/v1/maintenance", r#"{"active":false}"#);
    access::set_audit_file("");
    let text = std::fs::read_to_string(&*file).unwrap();
    // Other tests' audited calls may land in the file while it's set: find ours.
    assert!(text.lines().any(|line| line.starts_with('{') && line.contains(r#""path":"/admin/api/v1/maintenance""#) && line.contains(r#""user":"deploy-bot""#)), "{text}");
}

// ── 18.33.0: the audit log's hash chain ──────────────────────────────────────

fn entry_json(n: u64) -> String {
    access::AuditEntry {
        at: 1_700_000_000 + n,
        user: "ann".into(),
        role: "operator".into(),
        via: "token".into(),
        address: "127.0.0.1".into(),
        method: "PUT".into(),
        path: "/admin/api/v1/maintenance".into(),
        detail: format!(r#"{{"active":{}}}"#, n % 2 == 0),
        status: 200,
        node: "web-1".into(),
    }
    .to_json()
}

/// `n` chained lines after `prev`, newline-terminated.
fn chained(n: u64, mut prev: String) -> Vec<String> {
    (0..n)
        .map(|i| {
            let line = access::chain_line(&entry_json(i), &prev);
            prev = access::line_hash(&line);
            line
        })
        .collect()
}

fn verify(lines: &[String]) -> access::ChainReport {
    access::verify_chain(&(lines.join("\n") + "\n"))
}

#[test]
fn a_chained_line_is_the_entry_plus_prev_and_hash() {
    let line = access::chain_line(&entry_json(0), access::CHAIN_START);
    let json = entry_json(0);
    assert!(line.starts_with(&json[..json.len() - 1]), "{line}");
    let with_prev = format!("{},\"prev\":\"{}\"}}", &json[..json.len() - 1], access::CHAIN_START);
    assert_eq!(format!("{},\"prev\":\"{}\",\"hash\":\"{}\"}}", &json[..json.len() - 1], access::CHAIN_START, access::line_hash(&with_prev)), line);
    // Still a JSON object a log shipper reads.
    assert!(crate::mcp::json_rpc::object_members(&line).is_some_and(|m| m.len() == 13), "{line}");
}

#[test]
fn an_intact_chain_verifies() {
    let report = verify(&chained(5, access::CHAIN_START.to_string()));
    assert!(report.ok(), "{report:?}");
    assert_eq!((5, 5, 0, None), (report.entries, report.chained, report.unchained, report.continues_from.clone()));
    assert_eq!(access::verify_chain(""), access::ChainReport::default());
    assert!(report.to_json().starts_with(r#"{"ok":true,"entries":5,"chained":5,"unchained":0,"continues_from":null,"broken":null}"#));
}

#[test]
fn an_edited_entry_breaks_its_own_hash() {
    let mut lines = chained(4, access::CHAIN_START.to_string());
    lines[2] = lines[2].replace(r#""user":"ann""#, r#""user":"bob""#);
    let report = verify(&lines);
    assert_eq!(Some(3), report.broken.as_ref().map(|b| b.0), "{report:?}");
    assert!(report.broken.unwrap().1.contains("changed"));
    assert_eq!(2, report.chained, "the lines before it verified");
    assert!(verify(&lines).to_json().contains(r#""broken":{"line":3,"#));
}

#[test]
fn a_removed_or_reordered_entry_breaks_the_next_prev() {
    let lines = chained(4, access::CHAIN_START.to_string());
    let mut removed = lines.clone();
    removed.remove(1);
    let report = verify(&removed);
    assert_eq!(Some(2), report.broken.as_ref().map(|b| b.0), "{report:?}");
    assert!(report.broken.unwrap().1.contains("removed"));
    let mut swapped = lines.clone();
    swapped.swap(1, 2);
    assert_eq!(Some(2), verify(&swapped).broken.map(|b| b.0));
    // A rewritten line whose hash was recomputed still breaks the next prev.
    let mut forged = lines.clone();
    let json = entry_json(9);
    let prev = access::line_hash(&lines[0]);
    forged[1] = access::chain_line(&json, &prev);
    assert_eq!(Some(3), verify(&forged).broken.map(|b| b.0));
}

#[test]
fn entries_from_before_the_chain_are_accepted_until_it_starts() {
    // A pre-18.33 file, then entries appended by 18.33: the first chained
    // line's prev is the hash of the last old line.
    let old = vec![entry_json(0), entry_json(1)];
    let mut lines = old.clone();
    lines.extend(chained(2, access::line_hash(&old[1])));
    let report = verify(&lines);
    assert!(report.ok(), "{report:?}");
    assert_eq!((4, 2, 2), (report.entries, report.chained, report.unchained));
    // An old line edited: caught by the next chained line only if it is the
    // last old one.
    let mut edited = lines.clone();
    edited[1] = edited[1].replace("ann", "eve");
    assert_eq!(Some(3), verify(&edited).broken.map(|b| b.0));
    // A line without hash after the chain started is refused (a stripped hash).
    let mut stripped = lines.clone();
    stripped.push(entry_json(7));
    let report = verify(&stripped);
    assert_eq!(Some(5), report.broken.as_ref().map(|b| b.0));
    assert!(report.broken.unwrap().1.contains("no prev/hash"));
}

#[test]
fn a_file_continuing_a_rotated_one_says_so() {
    let previous_file = chained(3, access::CHAIN_START.to_string());
    let next_file = chained(2, access::line_hash(previous_file.last().unwrap()));
    let report = verify(&next_file);
    assert!(report.ok());
    assert_eq!(Some(access::line_hash(previous_file.last().unwrap())), report.continues_from);
}

#[test]
fn the_audit_file_is_chained_across_restarts_and_verified_by_the_api() {
    let _g = crate::test_env::lock();
    let file = crate::test_env::TempPath::new("admin-audit-chain.log");
    // A line written before 18.33.
    std::fs::write(&*file, format!("{}\n", entry_json(0))).unwrap();
    access::set_audit_file(file.as_str());
    let api = team();
    call_as(&api, "operator-token-0123456789", "PUT", "/admin/api/v1/maintenance", r#"{"active":false}"#);
    // A restart: the chain continues from the file's last line.
    access::set_audit_file(file.as_str());
    call_as(&api, "operator-token-0123456789", "PUT", "/admin/api/v1/maintenance", r#"{"active":false}"#);
    let (status, text) = call_as(&api, "viewer-token-0123456789", "GET", "/admin/api/v1/audit/verify", "");
    assert_eq!(status, 200, "{text}");
    assert!(text.starts_with(&format!(r#"{{"file":"{}","ok":true,"#, file.as_str())), "{text}");
    assert!(text.contains(r#""unchained":1"#), "{text}");
    let report = access::verify_chain(&std::fs::read_to_string(&*file).unwrap());
    assert!(report.chained >= 2 && report.ok(), "{report:?}");
    // Someone edits the file.
    let tampered = std::fs::read_to_string(&*file).unwrap().replacen("deploy-bot", "someone-else", 1);
    std::fs::write(&*file, tampered).unwrap();
    let (status, text) = call_as(&api, "viewer-token-0123456789", "GET", "/admin/api/v1/audit/verify", "");
    assert_eq!(status, 200);
    assert!(text.contains(r#""ok":false"#) && text.contains(r#""broken":{"line":"#), "{text}");
    assert_eq!(call_as(&api, "admin-token", "POST", "/admin/api/v1/audit/verify", "").0, 405);
    access::set_audit_file("");
    let (status, text) = call_as(&api, "viewer-token-0123456789", "GET", "/admin/api/v1/audit/verify", "");
    assert_eq!(status, 404);
    assert!(text.contains("no [admin] audit_log"), "{text}");
}

#[test]
fn redaction_keeps_ordinary_values() {
    assert_eq!(access::redact(br#"{"ip":"1.2.3.4","password":"x","drained":true}"#), r#"{"ip":"1.2.3.4","password":"(redacted)","drained":true}"#);
    assert_eq!(access::redact(b"not json"), "not json");
    assert!(access::redact(&vec![b'a'; 5000]).len() < 2100);
}

#[test]
fn the_login_page_learns_the_sign_in_methods_without_a_token() {
    let response = get(&AdminApi::new("t"), "/admin/api/v1/auth", "");
    assert_eq!((response.status_code, body(&response).as_str()), (200, r#"{"token":true,"oidc":false}"#));
    assert_eq!(get(&AdminApi::new("t"), "/admin/auth/login", "").status_code, 404);
}

#[test]
fn admin_users_and_oidc_settings_are_checked() {
    let report = check_str_offline("[admin]\nlisten = \"127.0.0.1:9901\"\nusers = [\"ci:CI_TOKEN:superuser\"]\noidc_issuer = \"https://id.example.com\"\n");
    let text = format!("{report:?}");
    assert!(text.contains("viewer, operator or admin"), "{text}");
    assert!(text.contains("no oidc_client_id"), "{text}");
    assert!(text.contains("needs oidc_roles"), "{text}");
    let report = check_str_offline("[admin]\nlisten = \"127.0.0.1:9901\"\nusers = [\"ci:CI_TOKEN:operator\"]\naudit_log = \"/var/log/rws/audit.log\"\n");
    assert!(report.is_ok(), "{report:?}");
}

#[test]
fn mcp_tool_changes_are_audited() {
    use crate::middleware::Middleware;
    struct Ok200;
    impl Application for Ok200 {
        fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            Ok(error(STATUS_CODE_REASON_PHRASE.n200_ok, "ok"))
        }
    }
    let layer = access::McpAuditLayer::new(["block_ip"]);
    let marker = format!("198.51.100.{}", std::process::id() % 200 + 1);
    let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"block_ip","arguments":{{"ip":"{marker}","token":"x"}}}}}}"#);
    let raw = format!("POST /mcp HTTP/1.1\r\nHost: h\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    layer.handle(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default(), &Ok200).unwrap();
    let read = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"server_metrics","arguments":{}}}"#;
    let raw = format!("POST /mcp HTTP/1.1\r\nHost: h\r\nContent-Length: {}\r\n\r\n{read}", read.len());
    layer.handle(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default(), &Ok200).unwrap();
    let audit = access::recent_json(1000);
    let entry = audit.split("},{").find(|e| e.contains(&marker)).expect("the tool call is audited");
    assert!(entry.contains(r#""user":"mcp""#) && entry.contains(r#""path":"block_ip""#) && entry.contains("(redacted)"), "{entry}");
    assert!(!audit.contains(r#""path":"server_metrics""#));
}
