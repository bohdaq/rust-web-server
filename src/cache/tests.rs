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
    req_en.headers.push(Header { name: "Accept".into(), value: "text/html".into() });
    req_en.headers.push(Header { name: "Accept-Language".into(), value: "en".into() });

    let mut req_fr = req_en.clone();
    req_fr.headers.iter_mut()
        .find(|h| h.name == "Accept-Language")
        .unwrap()
        .value = "fr".into();

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

#[test]
fn response_with_no_cache_is_not_cached_it_needs_revalidation() {
    let layer = CacheLayer::memory(100).ttl(60);
    assert_eq!(2, handler_calls(&layer, &get("/"), response_with_cache_control("no-cache")));
    assert_eq!(1, handler_calls(&layer, &get("/max-age"), response_with_cache_control("public, max-age=60")));
}

#[test]
fn range_requests_are_not_stored() {
    let layer = CacheLayer::memory(100).ttl(60);
    assert_eq!(2, handler_calls(&layer, &get_with_header("/r", "Range", "bytes=0-1"), ok_response()));
}

#[test]
fn vary_on_a_header_outside_the_key_is_not_cached_unless_it_changed_nothing() {
    let layer = CacheLayer::memory(100).ttl(60);
    // A gzipped body must not be replayed to a client without gzip...
    let gzipped = with_headers(&[("Content-Encoding", "gzip"), ("Vary", "Origin, Accept-Encoding")]);
    assert_eq!(2, handler_calls(&layer, &get("/gz"), gzipped.clone()));
    // ...unless Accept-Encoding is part of the key.
    let keyed = CacheLayer::memory(100).ttl(60).vary_by_header("Accept-Encoding");
    assert_eq!(1, handler_calls(&keyed, &get_with_header("/gz", "Accept-Encoding", "gzip"), gzipped));
    // An unencoded body is valid for every client.
    assert_eq!(1, handler_calls(&layer, &get("/plain"), with_headers(&[("Vary", "Origin, Accept-Encoding")])));
    // A CORS answer for one origin must not go to another.
    let cors = with_headers(&[("Vary", "Origin"), ("Access-Control-Allow-Origin", "https://a.example")]);
    assert_eq!(2, handler_calls(&layer, &get("/cors"), cors));
    assert_eq!(2, handler_calls(&layer, &get("/star"), with_headers(&[("Vary", "*")])));
    assert_eq!(2, handler_calls(&layer, &get("/ua"), with_headers(&[("Vary", "User-Agent")])));
}

// ── #49: never cache private responses ───────────────────────────────────────

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

#[test]
fn freshness_follows_s_maxage_then_max_age_capped_by_ttl() {
    use super::freshness;
    use std::time::Duration;
    let cap = Duration::from_secs(60);
    let f = |cc: &str| freshness(&response_with_cache_control(cc), cap).as_secs();
    assert_eq!(freshness(&ok_response(), cap).as_secs(), 60, "no Cache-Control: the layer's ttl");
    assert_eq!(f("max-age=10"), 10);
    assert_eq!(f("public, max-age=10, s-maxage=30"), 30, "s-maxage wins for a shared cache");
    assert_eq!(f("max-age=3600"), 60, "capped by the ttl");
    assert_eq!(f("max-age=0"), 0);
    assert_eq!(f("MAX-AGE = 5"), 5);
    assert_eq!(f("max-age=\"7\""), 7);
    assert_eq!(f("max-age=soon"), 60, "a bad value is ignored");
    assert_eq!(f("max-age=20, max-age=5"), 5, "repeated: the smaller");
    assert_eq!(f("stale-while-revalidate=99"), 60, "other directives don't count");
}

#[test]
fn max_age_zero_is_not_cached_and_max_age_expires_entries() {
    let zero = CountingApp::new(response_with_cache_control("max-age=0"));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/z"), &conn(), &zero).unwrap();
    layer.handle(&get("/z"), &conn(), &zero).unwrap();
    assert_eq!(2, zero.call_count());

    let short = CountingApp::new(response_with_cache_control("max-age=1"));
    let layer = CacheLayer::memory(100).ttl(60);
    layer.handle(&get("/s"), &conn(), &short).unwrap();
    layer.handle(&get("/s"), &conn(), &short).unwrap();
    assert_eq!(1, short.call_count(), "fresh for a second");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    layer.handle(&get("/s"), &conn(), &short).unwrap();
    assert_eq!(2, short.call_count(), "stale after max-age, though the ttl is 60");
}

// ── disk store ────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod disk {
    use super::*;
    use crate::test_env::TempPath;

    fn body(response: &Response) -> String {
        response.content_range_list.first().map(|p| String::from_utf8_lossy(&p.body).to_string()).unwrap_or_default()
    }

    fn dir(name: &str) -> TempPath {
        TempPath::new(name)
    }

    #[test]
    fn hits_come_from_disk_with_headers_body_and_age() {
        let tmp = dir("rws-cache-disk-roundtrip");
        let mut response = response_with_body("stored on disk");
        response.headers.push(Header::new("X-Origin", "app"));
        let app = CountingApp::new(response);
        let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap().ttl(60);
        layer.handle(&get("/a"), &conn(), &app).unwrap();
        let hit = layer.handle(&get("/a"), &conn(), &app).unwrap();
        assert_eq!(app.call_count(), 1);
        assert_eq!(body(&hit), "stored on disk");
        assert_eq!(hit.header("X-Origin").map(|h| h.value.to_string()).as_deref(), Some("app"));
        assert_eq!(hit.header("Age").map(|h| h.value.to_string()).as_deref(), Some("0"));
        assert!(hit.header("Content-Length").is_none(), "regenerated by the writer, not stored twice");
        assert_eq!((layer.size(), layer.hits(), layer.misses()), (1, 1, 1));
        assert!(layer.stored_bytes().unwrap() > 0);
        assert!(!crate::middleware::Middleware::runs_inline(&layer, &get("/a"), &conn()), "file I/O stays off the event loop");
    }

    #[test]
    fn entries_survive_a_restart() {
        let tmp = dir("rws-cache-disk-restart");
        let app = CountingApp::new(response_with_body("kept"));
        CacheLayer::disk(&*tmp, 1 << 20).unwrap().handle(&get("/k"), &conn(), &app).unwrap();
        let reopened = CacheLayer::disk(&*tmp, 1 << 20).unwrap();
        assert_eq!(reopened.size(), 1);
        let hit = reopened.handle(&get("/k"), &conn(), &app).unwrap();
        assert_eq!((app.call_count(), body(&hit).as_str()), (1, "kept"));
    }

    #[test]
    fn total_size_is_bounded_oldest_first() {
        let tmp = dir("rws-cache-disk-evict");
        let app = CountingApp::new(response_with_body(&"x".repeat(400)));
        let layer = CacheLayer::disk(&*tmp, 1500).unwrap();
        for path in ["/1", "/2", "/3", "/4"] {
            layer.handle(&get(path), &conn(), &app).unwrap();
        }
        assert!(layer.stored_bytes().unwrap() <= 1500, "{:?}", layer.stored_bytes());
        assert!(layer.size() < 4);
        layer.handle(&get("/4"), &conn(), &app).unwrap();
        assert_eq!(app.call_count(), 4, "the newest is still cached");
        layer.handle(&get("/1"), &conn(), &app).unwrap();
        assert_eq!(app.call_count(), 5, "the oldest was evicted");
        let files = std::fs::read_dir(&*tmp).unwrap().count();
        assert_eq!(files, layer.size(), "evicted files are deleted");
    }

    #[test]
    fn corrupt_and_foreign_files_are_removed_and_temp_files_cleaned() {
        let tmp = dir("rws-cache-disk-corrupt");
        std::fs::create_dir_all(&*tmp).unwrap();
        std::fs::write(&*tmp.join("deadbeef.entry"), b"RWSCACHE1 nonsense").unwrap();
        std::fs::write(&*tmp.join("x.entry.tmp-1-1"), b"partial").unwrap();
        std::fs::write(&*tmp.join("notes.txt"), b"left alone").unwrap();
        let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap();
        assert_eq!(layer.size(), 0);
        let mut names: Vec<String> = std::fs::read_dir(&*tmp).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, ["notes.txt"]);
    }

    #[test]
    fn an_entry_damaged_after_indexing_is_a_miss_and_is_removed() {
        let tmp = dir("rws-cache-disk-damaged");
        let app = CountingApp::new(response_with_body("v1"));
        let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap();
        layer.handle(&get("/d"), &conn(), &app).unwrap();
        for entry in std::fs::read_dir(&*tmp).unwrap() {
            std::fs::write(entry.unwrap().path(), b"garbage").unwrap();
        }
        let response = layer.handle(&get("/d"), &conn(), &app).unwrap();
        assert_eq!((app.call_count(), body(&response).as_str()), (2, "v1"), "served fresh, then re-stored");
    }

    #[test]
    fn expired_entries_are_misses_and_max_age_applies() {
        let tmp = dir("rws-cache-disk-ttl");
        let app = CountingApp::new(response_with_cache_control("max-age=1"));
        let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap().ttl(60);
        layer.handle(&get("/t"), &conn(), &app).unwrap();
        layer.handle(&get("/t"), &conn(), &app).unwrap();
        assert_eq!(app.call_count(), 1);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        layer.handle(&get("/t"), &conn(), &app).unwrap();
        assert_eq!(app.call_count(), 2);
        let zero = CountingApp::new(response_with_cache_control("max-age=0"));
        layer.handle(&get("/z"), &conn(), &zero).unwrap();
        assert!(layer.handle(&get("/z"), &conn(), &zero).is_ok());
        assert_eq!(zero.call_count(), 2);
    }

    #[test]
    fn clear_removes_files() {
        let tmp = dir("rws-cache-disk-clear");
        let app = CountingApp::new(ok_response());
        let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap();
        layer.handle(&get("/c"), &conn(), &app).unwrap();
        layer.clear();
        assert_eq!((layer.size(), std::fs::read_dir(&*tmp).unwrap().count()), (0, 0));
    }

    #[test]
    fn entry_headers_parse_strictly() {
        use crate::cache::disk::parse_entry_header;
        let ok = b"RWSCACHE1 1700000000000 60 3\n/ab\nHTTP/1.1 200 OK\r\n\r\n";
        let (stored, ttl, key, offset) = parse_entry_header(ok).unwrap();
        assert_eq!((stored, ttl.as_secs(), key.as_str(), &ok[offset..offset + 8]), (1_700_000_000_000, 60, "/ab", &b"HTTP/1.1"[..]));
        for bad in [&b""[..], b"RWSCACHE2 1 1 1\nx\n", b"RWSCACHE1 1 1 5\nab\n", b"RWSCACHE1 x 1 1\na\n", b"RWSCACHE1 1 1 99999999\n", b"RWSCACHE1 1 1 1 extra\na\n", b"RWSCACHE1 1 1 1\nab"] {
            assert!(parse_entry_header(bad).is_none(), "{:?}", String::from_utf8_lossy(bad));
        }
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
}

// ── purge ─────────────────────────────────────────────────────────────────────

fn request(method: &str, uri: &str, headers: &str) -> Request {
    Request::parse(format!("{method} {uri} HTTP/1.1\r\nHost: x\r\n{headers}\r\n").as_bytes()).unwrap()
}

#[test]
fn purge_exact_prefix_and_all_cover_vary_variants() {
    use super::Purge;
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).vary_by_header("Accept-Language");
    for (path, lang) in [("/p/1", "en"), ("/p/1", "uk"), ("/p/2", "en"), ("/q/1", "en")] {
        layer.handle(&get_with_header(path, "Accept-Language", lang), &conn(), &app).unwrap();
    }
    assert_eq!(layer.size(), 4);
    assert_eq!(layer.purge(&Purge::Exact("/p/1".into())), 2, "both language variants");
    assert_eq!(layer.purge(&Purge::Prefix("/p/".into())), 1);
    assert_eq!(layer.purge(&Purge::Exact("/nothing".into())), 0);
    assert_eq!(layer.purge(&Purge::All), 1);
    assert_eq!(layer.size(), 0);
    assert_eq!(Purge::from_target("/p/*"), Purge::Prefix("/p/".into()));
    assert_eq!(Purge::from_target("/p/1?x=2"), Purge::Exact("/p/1?x=2".into()));
}

#[test]
fn purge_method_needs_the_token_and_otherwise_passes_through() {
    let app = CountingApp::new(ok_response());
    let open = CacheLayer::memory(100);
    open.handle(&request("PURGE", "/a", ""), &conn(), &app).unwrap();
    assert_eq!(app.call_count(), 1, "without a token PURGE goes to the app");

    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::memory(100).purge_token("s3cret");
    layer.handle(&get("/a"), &conn(), &app).unwrap();
    layer.handle(&get("/a/b"), &conn(), &app).unwrap();
    let refused = layer.handle(&request("PURGE", "/a", "Authorization: Bearer nope\r\n"), &conn(), &app).unwrap();
    assert_eq!(refused.status_code, 401);
    assert!(refused.header("WWW-Authenticate").is_some());
    assert_eq!(layer.size(), 2);
    let done = layer.handle(&request("PURGE", "/a/*", "Authorization: Bearer s3cret\r\n"), &conn(), &app).unwrap();
    assert_eq!(done.status_code, 200);
    assert_eq!(String::from_utf8_lossy(&done.content_range_list[0].body), r#"{"purged":1}"#, "/a/* is /a/b, not /a");
    assert_eq!(layer.size(), 1);
    assert_eq!(app.call_count(), 2, "PURGE never reaches the app");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn disk_caches_purge_their_files() {
    let tmp = crate::test_env::TempPath::new("rws-cache-disk-purge");
    let app = CountingApp::new(ok_response());
    let layer = CacheLayer::disk(&*tmp, 1 << 20).unwrap();
    layer.handle(&get("/x/1"), &conn(), &app).unwrap();
    layer.handle(&get("/y/1"), &conn(), &app).unwrap();
    assert_eq!(layer.purge(&super::Purge::Prefix("/x/".into())), 1);
    assert_eq!((layer.size(), std::fs::read_dir(&*tmp).unwrap().count()), (1, 1));
    assert_eq!(layer.kind(), "disk");
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
    let cache = super::registry::get("route:catalog").expect("registered");
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
