//! Unit tests for `CircuitBreaker` and `RetryLayer`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::{BreakerState, CircuitBreaker, RetryLayer};
use crate::application::Application;
use crate::middleware::WithMiddleware;
use crate::range::Range;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};
use crate::http::VERSION;

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_connection() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    }
}

fn make_request() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn make_response(status: i16, phrase: &str) -> Response {
    let cr = Range::get_content_range(vec![], MimeType::TEXT_PLAIN.to_string());
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: status,
        reason_phrase: phrase.to_string(),
        headers: vec![],
        content_range_list: vec![cr],
        stream_file: None,
        stream_pipe: None,
    }
}

// ── Spy application ───────────────────────────────────────────────────────────

/// A test Application that counts calls and returns 502 for the first
/// `fail_count` calls, then 200.
struct Spy {
    call_count: Arc<AtomicU32>,
    fail_count: u32,
}

impl Spy {
    fn new(fail_count: u32) -> (Self, Arc<AtomicU32>) {
        let counter = Arc::new(AtomicU32::new(0));
        (Spy { call_count: Arc::clone(&counter), fail_count }, counter)
    }
}

impl Application for Spy {
    fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        let n = self.call_count.fetch_add(1, Ordering::Relaxed);
        if n < self.fail_count {
            Ok(make_response(502, "Bad Gateway"))
        } else {
            Ok(make_response(200, "OK"))
        }
    }
}

// ── CircuitBreaker tests ──────────────────────────────────────────────────────

#[test]
fn starts_closed() {
    let mut cb = CircuitBreaker::new(3, 30);
    assert!(cb.is_available("x"), "new backend should be available");
    assert_eq!(BreakerState::Closed, cb.state("x"));
}

#[test]
fn opens_after_threshold() {
    let mut cb = CircuitBreaker::new(3, 30);
    cb.record_failure("x");
    assert_eq!(BreakerState::Closed, cb.state("x"));
    cb.record_failure("x");
    assert_eq!(BreakerState::Closed, cb.state("x"));
    cb.record_failure("x");
    assert_eq!(BreakerState::Open, cb.state("x"), "should open after threshold failures");
    assert!(!cb.is_available("x"), "open circuit should not be available");
}

#[test]
fn half_opens_after_recovery() {
    // Use a zero-second recovery so the transition happens immediately.
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    assert_eq!(BreakerState::Open, cb.state("x"));
    // Give the elapsed time a moment to exceed the zero-duration recovery.
    std::thread::sleep(Duration::from_millis(1));
    assert!(cb.is_available("x"), "should be available (half-open) after recovery window");
    assert_eq!(BreakerState::HalfOpen, cb.state("x"));
}

#[test]
fn closes_after_success_in_half_open() {
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    // Transition to HalfOpen
    let _ = cb.is_available("x");
    assert_eq!(BreakerState::HalfOpen, cb.state("x"));
    cb.record_success("x");
    assert_eq!(BreakerState::Closed, cb.state("x"));
    assert!(cb.is_available("x"));
}

#[test]
fn reopens_after_failure_in_half_open() {
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    // Transition to HalfOpen
    let _ = cb.is_available("x");
    assert_eq!(BreakerState::HalfOpen, cb.state("x"));
    cb.record_failure("x");
    assert_eq!(BreakerState::Open, cb.state("x"), "failure in HalfOpen should re-open");
}

#[test]
fn reset_clears_state() {
    let mut cb = CircuitBreaker::new(2, 30);
    cb.record_failure("x");
    cb.record_failure("x");
    assert_eq!(BreakerState::Open, cb.state("x"));
    cb.reset("x");
    assert_eq!(BreakerState::Closed, cb.state("x"));
    assert!(cb.is_available("x"));
}

#[test]
fn independent_backends() {
    let mut cb = CircuitBreaker::new(2, 30);
    cb.record_failure("a");
    cb.record_failure("a");
    assert_eq!(BreakerState::Open, cb.state("a"));
    // "b" is still untouched
    assert_eq!(BreakerState::Closed, cb.state("b"));
    assert!(cb.is_available("b"));
}

// ── RetryLayer tests ──────────────────────────────────────────────────────────

#[test]
fn retry_layer_retries_on_bad_gateway() {
    let (spy, counter) = Spy::new(2); // first 2 calls return 502, then 200
    let app = WithMiddleware::new(spy).wrap(RetryLayer::new().max_retries(3));
    let req = make_request();
    let conn = make_connection();
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(200, resp.status_code, "final response should be 200");
    assert_eq!(3, counter.load(Ordering::Relaxed), "spy should have been called 3 times");
}

#[test]
fn retry_layer_does_not_retry_on_success() {
    let (spy, counter) = Spy::new(0); // always 200
    let app = WithMiddleware::new(spy).wrap(RetryLayer::new());
    let req = make_request();
    let conn = make_connection();
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(200, resp.status_code);
    assert_eq!(1, counter.load(Ordering::Relaxed), "should only call once on success");
}

#[test]
fn retry_layer_gives_up_after_max_retries() {
    let (spy, counter) = Spy::new(100); // always 502
    let app = WithMiddleware::new(spy).wrap(RetryLayer::new().max_retries(2));
    let req = make_request();
    let conn = make_connection();
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(502, resp.status_code, "should return 502 after exhausting retries");
    // 1 initial + 2 retries = 3 total
    assert_eq!(3, counter.load(Ordering::Relaxed));
}

#[test]
fn retry_layer_custom_codes() {
    let (spy, counter) = Spy::new(1); // first call 502, then 200
    let app = WithMiddleware::new(spy)
        .wrap(RetryLayer::new().retry_on(vec![404, 502]).max_retries(5));
    let req = make_request();
    let conn = make_connection();
    let resp = app.execute(&req, &conn).unwrap();
    assert_eq!(200, resp.status_code);
    assert_eq!(2, counter.load(Ordering::Relaxed));
}
