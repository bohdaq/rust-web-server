use std::sync::{Arc, Mutex};

use crate::application::Application;
use crate::cache::CacheLayer;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

// ── helpers ───────────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1234 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 8192,
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

fn get_with_header(uri: &str, name: &str, value: &str) -> Request {
    let mut r = get(uri);
    r.headers.push(Header { name: name.to_string(), value: value.to_string() });
    r
}

fn post(uri: &str) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn ok_response() -> Response {
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: 200,
        reason_phrase: "OK".to_string(),
        headers: vec![Header { name: "Content-Type".to_string(), value: "text/plain".to_string() }],
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
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
    }
}

fn response_with_cache_control(value: &str) -> Response {
    let mut r = ok_response();
    r.headers.push(Header { name: "Cache-Control".to_string(), value: value.to_string() });
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn cache_miss_calls_handler() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(1, app.call_count());
}

#[test]
fn cache_hit_does_not_call_handler_again() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(1, app.call_count());
}

#[test]
fn cache_hit_returns_correct_body() {
    let app = CountingApp::new(response_with_body("hello"));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    let res = layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(200, res.status_code);
    assert!(!res.content_range_list.is_empty());
    assert_eq!(b"hello", res.content_range_list[0].body.as_slice());
}

#[test]
fn different_uris_are_cached_separately() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/a"), &conn(), &app).unwrap();
    layer.handle(&get("/b"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn post_requests_bypass_cache() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&post("/"), &conn(), &app).unwrap();
    layer.handle(&post("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn non_2xx_responses_are_not_cached() {
    let app = CountingApp::new(response_with_status(404));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn response_with_no_store_is_not_cached() {
    let app = CountingApp::new(response_with_cache_control("no-store"));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn response_with_private_is_not_cached() {
    let app = CountingApp::new(response_with_cache_control("private"));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn request_no_cache_bypasses_cache_but_stores_result() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    // First: normal — stored.
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(1, app.call_count());
    // Second: no-cache — bypasses, calls handler again, stores fresh copy.
    layer.handle(&get_with_header("/", "Cache-Control", "no-cache"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
    // Third: normal — hits cache (result from second request).
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn ttl_zero_expires_entries_immediately() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(0);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    layer.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn vary_by_header_separates_entries() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60).vary_by_header("Accept");
    layer.handle(&get_with_header("/", "Accept", "text/html"), &conn(), &app).unwrap();
    layer.handle(&get_with_header("/", "Accept", "application/json"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn vary_by_header_hits_same_entry_for_same_value() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60).vary_by_header("Accept");
    layer.handle(&get_with_header("/", "Accept", "text/html"), &conn(), &app).unwrap();
    layer.handle(&get_with_header("/", "Accept", "text/html"), &conn(), &app).unwrap();
    assert_eq!(1, app.call_count());
}

#[test]
fn capacity_evicts_oldest_entry() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(1).ttl(60);
    // Fill cache with /a.
    layer.handle(&get("/a"), &conn(), &app).unwrap();
    // /b evicts /a.
    layer.handle(&get("/b"), &conn(), &app).unwrap();
    // /a must be called again.
    layer.handle(&get("/a"), &conn(), &app).unwrap();
    assert_eq!(3, app.call_count());
}

#[test]
fn age_header_present_on_cache_hit() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/"), &conn(), &app).unwrap();
    let res = layer.handle(&get("/"), &conn(), &app).unwrap();
    assert!(
        res.headers.iter().any(|h| h.name.eq_ignore_ascii_case("Age")),
        "Age header must be present on a cache hit"
    );
}

#[test]
fn multiple_vary_headers_combine_into_key() {
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).ttl(60)
        .vary_by_header("Accept")
        .vary_by_header("Accept-Language");

    let mut req_en = get("/");
    req_en.headers.push(Header { name: "Accept".to_string(), value: "text/html".to_string() });
    req_en.headers.push(Header { name: "Accept-Language".to_string(), value: "en".to_string() });

    let mut req_fr = req_en.clone();
    req_fr.headers.iter_mut()
        .find(|h| h.name == "Accept-Language")
        .unwrap()
        .value = "fr".to_string();

    layer.handle(&req_en, &conn(), &app).unwrap();
    layer.handle(&req_fr, &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());

    layer.handle(&req_en, &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

#[test]
fn cache_is_per_layer_instance() {
    let app = CountingApp::new(ok_response());
    let layer_a = CacheLayer::memory(100).ttl(60);
    let layer_b = CacheLayer::memory(100).ttl(60);
    layer_a.handle(&get("/"), &conn(), &app).unwrap();
    layer_b.handle(&get("/"), &conn(), &app).unwrap();
    assert_eq!(2, app.call_count());
}

// ── #49: never cache private responses ───────────────────────────────────────

/// Answers with whatever identifies the caller (Cookie or Authorization),
/// and sets a cookie or streams a file when asked to.
struct WhoAmI;

impl Application for WhoAmI {
    fn execute(&self, request: &Request, _c: &ConnectionInfo) -> Result<Response, String> {
        let who = request.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case("Cookie") || h.name.eq_ignore_ascii_case("Authorization"))
            .map(|h| h.value.clone())
            .unwrap_or_else(|| "anonymous".to_string());
        let mut response = ok_response();
        response.content_range_list = vec![Range::get_content_range(who.into_bytes(), "text/plain".to_string())];
        if request.request_uri == "/login" {
            response.headers.push(Header { name: "Set-Cookie".to_string(), value: "session=alice".to_string() });
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

#[test]
fn a_response_for_one_user_is_not_served_to_another() {
    let cache = CacheLayer::memory(100).ttl(60);
    for header in ["Cookie", "Authorization"] {
        let alice = get_with_header("/me", header, "alice");
        assert_eq!("alice", body(&cache.handle(&alice, &conn(), &WhoAmI).unwrap()));
        // Bob, without credentials, must not get Alice's page.
        assert_eq!("anonymous", body(&cache.handle(&get("/me"), &conn(), &WhoAmI).unwrap()));
        let bob = get_with_header("/me", header, "bob");
        assert_eq!("bob", body(&cache.handle(&bob, &conn(), &WhoAmI).unwrap()));
    }
}

#[test]
fn cookie_setting_partial_and_streamed_responses_are_not_stored() {
    let cache = CacheLayer::memory(100).ttl(60);
    let _ = cache.handle(&get("/login"), &conn(), &WhoAmI).unwrap();
    let again = cache.handle(&get("/login"), &conn(), &WhoAmI).unwrap();
    assert!(again.headers.iter().all(|h| h.name != "Age"), "Set-Cookie response came from the cache");

    let _ = cache.handle(&get("/download"), &conn(), &WhoAmI).unwrap();
    let again = cache.handle(&get("/download"), &conn(), &WhoAmI).unwrap();
    assert!(again.headers.iter().all(|h| h.name != "Age"), "streamed response came from the cache");
    assert!(again.stream_file.is_some());

    // A Range request neither reads nor fills the cache.
    let _ = cache.handle(&get_with_header("/part", "Range", "bytes=0-1"), &conn(), &WhoAmI).unwrap();
    let plain = cache.handle(&get("/part"), &conn(), &WhoAmI).unwrap();
    assert!(plain.headers.iter().all(|h| h.name != "Age"), "a Range response filled the cache");
}
