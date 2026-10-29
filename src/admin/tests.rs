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
