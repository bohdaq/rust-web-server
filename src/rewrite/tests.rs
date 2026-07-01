use std::sync::Mutex;

use crate::application::Application;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};
use super::RewriteLayer;

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
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

fn ok(body: &[u8]) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

// Captures the incoming request and returns a fixed response.
struct Spy {
    captured: Mutex<Option<Request>>,
    reply: Response,
}

impl Spy {
    fn new(reply: Response) -> Self {
        Spy { captured: Mutex::new(None), reply }
    }
    fn captured(&self) -> Request {
        self.captured.lock().unwrap().clone().expect("spy not called")
    }
}

impl Application for Spy {
    fn execute(&self, request: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        *self.captured.lock().unwrap() = Some(request.clone());
        Ok(self.reply.clone())
    }
}

// ── Request rules ─────────────────────────────────────────────────────────────

#[test]
fn request_header_set_adds_new_header() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_header_set("X-Env", "production");
    layer.handle(&get("/"), &conn(), &spy).unwrap();
    let req = spy.captured();
    let h = req.headers.iter().find(|h| h.name.eq_ignore_ascii_case("X-Env"));
    assert!(h.is_some(), "X-Env not found");
    assert_eq!("production", h.unwrap().value);
}

#[test]
fn request_header_set_replaces_existing_header() {
    let mut req = get("/");
    req.headers.push(Header { name: "X-Env".to_string(), value: "staging".to_string() });
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_header_set("x-env", "production");
    layer.handle(&req, &conn(), &spy).unwrap();
    let received = spy.captured();
    let values: Vec<&str> = received.headers.iter()
        .filter(|h| h.name.eq_ignore_ascii_case("X-Env"))
        .map(|h| h.value.as_str())
        .collect();
    assert_eq!(vec!["production"], values, "should have exactly one X-Env with the new value");
}

#[test]
fn request_header_remove_removes_header() {
    let mut req = get("/");
    req.headers.push(Header { name: "X-Debug".to_string(), value: "1".to_string() });
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_header_remove("X-Debug");
    layer.handle(&req, &conn(), &spy).unwrap();
    let found = spy.captured().headers.iter().any(|h| h.name.eq_ignore_ascii_case("X-Debug"));
    assert!(!found, "X-Debug should have been removed");
}

#[test]
fn request_header_remove_is_case_insensitive() {
    let mut req = get("/");
    req.headers.push(Header { name: "x-debug".to_string(), value: "1".to_string() });
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_header_remove("X-DEBUG");
    layer.handle(&req, &conn(), &spy).unwrap();
    let found = spy.captured().headers.iter().any(|h| h.name.eq_ignore_ascii_case("x-debug"));
    assert!(!found, "x-debug should have been removed case-insensitively");
}

#[test]
fn request_uri_set_replaces_uri() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_uri_set("/new/path");
    layer.handle(&get("/old"), &conn(), &spy).unwrap();
    assert_eq!("/new/path", spy.captured().request_uri);
}

#[test]
fn request_uri_strip_prefix_strips_present_prefix() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_uri_strip_prefix("/api/v1");
    layer.handle(&get("/api/v1/users"), &conn(), &spy).unwrap();
    assert_eq!("/users", spy.captured().request_uri);
}

#[test]
fn request_uri_strip_prefix_noop_when_absent() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_uri_strip_prefix("/api/v2");
    layer.handle(&get("/api/v1/users"), &conn(), &spy).unwrap();
    assert_eq!("/api/v1/users", spy.captured().request_uri);
}

#[test]
fn request_uri_strip_prefix_normalizes_to_slash() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_uri_strip_prefix("/api");
    layer.handle(&get("/api"), &conn(), &spy).unwrap();
    assert_eq!("/", spy.captured().request_uri);
}

#[test]
fn request_uri_add_prefix_prepends() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().request_uri_add_prefix("/v2");
    layer.handle(&get("/users"), &conn(), &spy).unwrap();
    assert_eq!("/v2/users", spy.captured().request_uri);
}

// ── Response rules ────────────────────────────────────────────────────────────

#[test]
fn response_header_set_adds_new_header() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().response_header_set("Cache-Control", "no-store");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    let found = resp.headers.iter().any(|h| {
        h.name.eq_ignore_ascii_case("Cache-Control") && h.value == "no-store"
    });
    assert!(found, "Cache-Control: no-store not found in response");
}

#[test]
fn response_header_set_replaces_existing() {
    let mut reply = ok(b"");
    reply.headers.push(Header { name: "Cache-Control".to_string(), value: "max-age=3600".to_string() });
    let spy = Spy::new(reply);
    let layer = RewriteLayer::new().response_header_set("cache-control", "no-store");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    let values: Vec<&str> = resp.headers.iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Cache-Control"))
        .map(|h| h.value.as_str())
        .collect();
    assert_eq!(vec!["no-store"], values, "should have exactly one Cache-Control header");
}

#[test]
fn response_header_remove_removes_header() {
    let mut reply = ok(b"");
    reply.headers.push(Header { name: "Server".to_string(), value: "rws".to_string() });
    let spy = Spy::new(reply);
    let layer = RewriteLayer::new().response_header_remove("Server");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    let found = resp.headers.iter().any(|h| h.name.eq_ignore_ascii_case("Server"));
    assert!(!found, "Server header should have been removed");
}

#[test]
fn response_status_overrides_code_and_reason() {
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new().response_status(202, "Accepted");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    assert_eq!(202, resp.status_code);
    assert_eq!("Accepted", resp.reason_phrase);
}

#[test]
fn response_body_replace_substitutes_bytes() {
    let spy = Spy::new(ok(b"hello world"));
    let layer = RewriteLayer::new().response_body_replace("world", "there");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    assert_eq!(b"hello there", resp.content_range_list[0].body.as_slice());
}

#[test]
fn response_body_replace_handles_multiple_occurrences() {
    let spy = Spy::new(ok(b"aaa"));
    let layer = RewriteLayer::new().response_body_replace("a", "bb");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    assert_eq!(b"bbbbbb", resp.content_range_list[0].body.as_slice());
}

#[test]
fn response_body_replace_noop_when_no_match() {
    let spy = Spy::new(ok(b"hello"));
    let layer = RewriteLayer::new().response_body_replace("xyz", "abc");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    assert_eq!(b"hello", resp.content_range_list[0].body.as_slice());
}

#[test]
fn response_body_replace_noop_on_empty_needle() {
    let spy = Spy::new(ok(b"hello"));
    let layer = RewriteLayer::new().response_body_replace("", "abc");
    let resp = layer.handle(&get("/"), &conn(), &spy).unwrap();
    assert_eq!(b"hello", resp.content_range_list[0].body.as_slice());
}

// ── Invariant: original request not mutated ───────────────────────────────────

#[test]
fn original_request_is_not_mutated() {
    let req = get("/original");
    let spy = Spy::new(ok(b""));
    let layer = RewriteLayer::new()
        .request_uri_set("/modified")
        .request_header_set("X-Added", "yes");
    layer.handle(&req, &conn(), &spy).unwrap();
    assert_eq!("/original", req.request_uri, "original request_uri should be unchanged");
    assert!(req.headers.is_empty(), "original headers should be unchanged");
}
