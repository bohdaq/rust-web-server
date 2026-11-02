//! Tests of rws-server's `cache` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use std::sync::{Arc, Mutex};
use crate::cache::CacheLayer;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};
use crate::application::Application;

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1234 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 8192,
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

fn get_with_header(uri: &str, name: &str, value: &str) -> Request {
    let mut r = get(uri);
    r.headers.push(Header { name: name.to_string().into(), value: value.to_string().into() });
    r
}

fn post(uri: &str) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

fn ok_response() -> Response {
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: 200,
        reason_phrase: "OK".to_string(),
        headers: vec![Header { name: "Content-Type".into(), value: "text/plain".into() }],
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
        trailers: None,
    }
}

fn response_with_body(text: &str) -> Response {
    let bytes = text.as_bytes().to_vec();
    let len = bytes.len() as u64;
    let mut r = ok_response();
    r.content_range_list.push(ContentRange {
        unit: "bytes".to_string(),
        range: Range { start: 0, end: len.saturating_sub(1) },
        size: len.to_string(),
        body: bytes,
        content_type: "text/plain".to_string(),
    });
    r
}

fn response_with_status(status: i16) -> Response {
    use crate::response::STATUS_CODE_REASON_PHRASE;
    let phrase = match status {
        404 => STATUS_CODE_REASON_PHRASE.n404_not_found,
        500 => STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
        _ => STATUS_CODE_REASON_PHRASE.n200_ok,
    };
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: status,
        reason_phrase: phrase.reason_phrase.to_string(),
        headers: vec![],
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
        trailers: None,
    }
}

fn response_with_cache_control(value: &str) -> Response {
    let mut r = ok_response();
    r.headers.push(Header { name: "Cache-Control".into(), value: value.to_string().into() });
    r
}

/// Counts how many times the inner handler was invoked.
#[derive(Clone)]
struct CountingApp {
    calls: Arc<Mutex<u32>>,
    response: Response,
}

impl CountingApp {
    fn new(response: Response) -> Self {
        CountingApp { calls: Arc::new(Mutex::new(0)), response }
    }

    fn call_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

impl Application for CountingApp {
    fn execute(&self, _req: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        *self.calls.lock().unwrap() += 1;
        Ok(self.response.clone())
    }
}

fn with_headers(headers: &[(&str, &str)]) -> Response {
    let mut r = ok_response();
    for (name, value) in headers {
        r.headers.push(Header::new(name.to_string(), value.to_string()));
    }
    r
}

/// Calls the layer twice; how many times the handler ran (1 = cached).
fn handler_calls(layer: &CacheLayer, request: &Request, response: Response) -> u32 {
    let app = CountingApp::new(response);
    layer.handle(request, &conn(), &app).unwrap();
    layer.handle(request, &conn(), &app).unwrap();
    app.call_count()
}

/// Answers with whatever identifies the caller (Cookie or Authorization),
/// and sets a cookie or streams a file when asked to.
struct WhoAmI;

impl Application for WhoAmI {
    fn execute(&self, request: &Request, _c: &ConnectionInfo) -> Result<Response, String> {
        let who = request.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case("Cookie") || h.name.eq_ignore_ascii_case("Authorization"))
            .map(|h| h.value.to_string())
            .unwrap_or_else(|| "anonymous".to_string());
        let mut response = ok_response();
        response.content_range_list = vec![Range::get_content_range(who.into_bytes(), "text/plain".to_string())];
        if request.request_uri == "/login" {
            response.headers.push(Header { name: "Set-Cookie".into(), value: "session=alice".into() });
        }
        if request.request_uri == "/download" {
            response.content_range_list.clear();
            response.stream_file = Some("/tmp/whatever".to_string());
        }
        Ok(response)
    }
}

fn body(response: &Response) -> String {
    response.content_range_list.first().map(|c| String::from_utf8_lossy(&c.body).to_string()).unwrap_or_default()
}

fn request(method: &str, uri: &str, headers: &str) -> Request {
    Request::parse(format!("{method} {uri} HTTP/1.1\r\nHost: x\r\n{headers}\r\n").as_bytes()).unwrap()
}

fn cache_status_of(response: &Response) -> String {
    response.header("Cache-Status").map(|h| h.value.to_string()).unwrap_or_default()
}

#[test]
fn the_config_file_builds_a_disk_cache_and_check_reports_mistakes() {
    let tmp = dir("rws-cache-disk-config");
    let dir = tmp.as_str().to_string();
    let toml = format!("[[route]]\n[route.match]\npath = \"/\"\n[route.action]\ntype = \"respond\"\nbody = \"hi\"\n[route.middleware.cache]\nttl_secs = 30\ndir = \"{dir}\"\nmax_size = \"8M\"\n");
    assert!(crate::config_check::check_str(&toml).errors.is_empty(), "{:?}", crate::config_check::check_str(&toml).errors);
    let config = crate::proxy_config::ProxyConfig::from_str(&toml);
    let cache = config.routes[0].middleware.cache.clone().unwrap();
    assert_eq!((cache.dir.as_str(), cache.max_size.as_str()), (dir.as_str(), "8M"));
    let (app, _h) = crate::proxy_config::builder::build(config);
    app.execute(&get("/"), &conn()).unwrap();
    assert_eq!(std::fs::read_dir(&*tmp).unwrap().count(), 1, "one entry file");

    let bad = toml.replace("max_size = \"8M\"", "max_size = \"huge\"");
    assert!(crate::config_check::check_str_offline(&bad).errors.iter().any(|e| e.contains("not a size")));
    let no_dir = toml.replace(&format!("dir = \"{dir}\"\n"), "");
    assert!(crate::config_check::check_str_offline(&no_dir).warnings.iter().any(|w| w.contains("only applies with `dir`")));
    let unwritable = toml.replace(&dir, "/proc/rws-cache-test");
    assert!(crate::config_check::check_str(&unwritable).errors.iter().any(|e| e.contains("isn't writable")));
}

#[test]
fn config_routes_register_their_caches_and_admin_purges_them() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_TEST_PURGE_TOKEN", "tok");
    let toml = "[[route]]\nname = \"catalog\"\n[route.match]\npath = \"/c/*\"\n[route.action]\ntype = \"respond\"\nbody = \"hi\"\n[route.middleware.cache]\npurge_token_env = \"RWS_TEST_PURGE_TOKEN\"\n";
    assert!(crate::config_check::check_str(toml).errors.is_empty(), "{:?}", crate::config_check::check_str(toml).errors);
    let (app, _h) = crate::proxy_config::builder::build(crate::proxy_config::ProxyConfig::from_str(toml));
    app.execute(&get("/c/1"), &conn()).unwrap();
    app.execute(&get("/c/2"), &conn()).unwrap();
    let cache = crate::cache::registry::get("route:catalog").expect("registered");
    assert_eq!(cache.size(), 2);
    let api = crate::admin::AdminApi::new("t");
    let call = |method: &str, path: &str| {
        let raw = format!("{method} {path} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer t\r\n\r\n");
        let r = api.execute(&Request::parse(raw.as_bytes()).unwrap(), &conn()).unwrap();
        (r.status_code, String::from_utf8_lossy(&r.content_range_list[0].body).to_string())
    };
    let (status, list) = call("GET", "/admin/api/v1/caches");
    assert_eq!(status, 200);
    assert!(list.contains(r#"{"name":"route:catalog","kind":"memory","entries":2,"bytes":null,"hits":0,"misses":2}"#), "{list}");
    assert_eq!(call("DELETE", "/admin/api/v1/caches/route%3Acatalog?path=/c/1"), (200, r#"{"name":"route:catalog","purged":1}"#.to_string()));
    assert_eq!(call("DELETE", "/admin/api/v1/caches/route:catalog").1, r#"{"name":"route:catalog","purged":1}"#);
    assert_eq!(call("DELETE", "/admin/api/v1/caches/nope").0, 404);
    let purge = Request::parse(b"PURGE /c/1 HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer tok\r\n\r\n").unwrap();
    assert_eq!(app.execute(&purge, &conn()).unwrap().status_code, 200, "PURGE through the config route");
    std::env::remove_var("RWS_TEST_PURGE_TOKEN");
    let unset = crate::config_check::check_str(toml).errors.join("\n");
    assert!(unset.contains("RWS_TEST_PURGE_TOKEN is unset"), "{unset}");
}

use crate::test_env::TempPath;

fn dir(name: &str) -> TempPath {
    TempPath::new(name)
}
