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
    let config = AdminConfig { listen: "127.0.0.1:0".into(), token_env: "RWS_TEST_ADMIN_TCP_TOKEN".into(), allowed_origins: vec![] };
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
