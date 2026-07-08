//! Unit tests for `CircuitBreaker` and `RetryLayer`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::{BreakerState, CircuitBreaker, RedisCircuitBreaker, RetryLayer};
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

// ── HalfOpen concurrency cap ──────────────────────────────────────────────────

#[test]
fn half_open_default_cap_lets_only_one_probe_through() {
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    // First caller transitions Open -> HalfOpen and is let through.
    assert!(cb.is_available("x"));
    assert_eq!(BreakerState::HalfOpen, cb.state("x"));
    // A second concurrent caller (probe still unresolved) must be rejected —
    // this is exactly the bug: before this fix, every concurrent caller saw
    // HalfOpen => true unconditionally.
    assert!(!cb.is_available("x"), "a second concurrent probe should be rejected while one is in flight");
    assert!(!cb.is_available("x"), "a third concurrent probe should also be rejected");
}

#[test]
fn half_open_cap_releases_after_success() {
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    assert!(cb.is_available("x"));
    assert!(!cb.is_available("x"), "capped while the first probe is unresolved");
    cb.record_success("x");
    assert_eq!(BreakerState::Closed, cb.state("x"));
    assert!(cb.is_available("x"), "Closed state has no cap");
}

#[test]
fn half_open_cap_releases_after_failure() {
    let mut cb = CircuitBreaker::new(1, 0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    assert!(cb.is_available("x"));
    assert!(!cb.is_available("x"), "capped while the first probe is unresolved");
    cb.record_failure("x");
    assert_eq!(BreakerState::Open, cb.state("x"));
    std::thread::sleep(Duration::from_millis(1));
    // Recovery is 0s, so Open immediately re-qualifies for exactly one new
    // HalfOpen probe — not an unbounded number, proving the in-flight count
    // was reset to 0 (not left stuck elevated) when the first probe failed.
    assert!(cb.is_available("x"), "exactly one new probe should be allowed after re-opening");
    assert!(!cb.is_available("x"), "a second concurrent probe should again be capped");
}

#[test]
fn half_open_cap_can_be_raised() {
    let mut cb = CircuitBreaker::new(1, 0).max_half_open_probes(3);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    assert!(cb.is_available("x"), "probe 1");
    assert!(cb.is_available("x"), "probe 2");
    assert!(cb.is_available("x"), "probe 3");
    assert!(!cb.is_available("x"), "probe 4 should be rejected — cap is 3");
}

#[test]
fn half_open_cap_of_zero_is_clamped_to_one() {
    let mut cb = CircuitBreaker::new(1, 0).max_half_open_probes(0);
    cb.record_failure("x");
    std::thread::sleep(Duration::from_millis(1));
    assert!(cb.is_available("x"), "cap=0 must be clamped to at least 1, or recovery would never be tested");
    assert!(!cb.is_available("x"));
}

// ── all_states (metrics) ──────────────────────────────────────────────────────

#[test]
fn all_states_reflects_every_backend_seen() {
    let mut cb = CircuitBreaker::new(1, 30);
    cb.record_failure("a"); // opens "a"
    let _ = cb.is_available("b"); // "b" stays Closed, but now tracked

    let mut states: Vec<(String, BreakerState)> = cb.all_states();
    states.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        vec![("a".to_string(), BreakerState::Open), ("b".to_string(), BreakerState::Closed)],
        states
    );
}

#[test]
fn all_states_empty_for_a_fresh_breaker() {
    let cb = CircuitBreaker::new(3, 30);
    assert!(cb.all_states().is_empty());
}

#[test]
fn redis_half_open_default_cap_lets_only_one_probe_through() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 1, 0);
    cb.record_failure("x").unwrap();
    assert!(cb.is_available("x").unwrap());
    assert_eq!(BreakerState::HalfOpen, cb.state("x").unwrap());
    assert!(!cb.is_available("x").unwrap(), "a second concurrent probe should be rejected");
}

#[test]
fn redis_half_open_cap_can_be_raised() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 1, 0);
    cb.set_max_half_open_probes(2);
    cb.record_failure("x").unwrap();
    assert!(cb.is_available("x").unwrap(), "probe 1");
    assert!(cb.is_available("x").unwrap(), "probe 2");
    assert!(!cb.is_available("x").unwrap(), "probe 3 should be rejected — cap is 2");
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

// ── RedisCircuitBreaker ────────────────────────────────────────────────────
//
// Spins up a tiny in-process fake Redis server (RESP v2) rather than
// requiring a real Redis instance in CI — same harness shape as
// `RedisRateLimiter`'s in `src/rate_limit/tests.rs`. Supports just enough of
// SET/GET/DEL to exercise RedisCircuitBreaker's logic.

use std::collections::HashMap as Map;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;

fn start_fake_redis() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        let store: Arc<Mutex<Map<String, String>>> = Arc::new(Mutex::new(Map::new()));
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let store = Arc::clone(&store);
            std::thread::spawn(move || fake_redis_conn(&mut stream, &store));
        }
    });
    addr
}

fn fake_redis_conn(stream: &mut TcpStream, store: &Mutex<Map<String, String>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let line = line.trim_end();
        if !line.starts_with('*') {
            return;
        }
        let argc: usize = match line[1..].parse() {
            Ok(n) => n,
            Err(_) => return,
        };
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            let mut len_line = String::new();
            if reader.read_line(&mut len_line).unwrap_or(0) == 0 {
                return;
            }
            let len: usize = match len_line.trim_end()[1..].parse() {
                Ok(n) => n,
                Err(_) => return,
            };
            let mut buf = vec![0u8; len + 2];
            if reader.read_exact(&mut buf).is_err() {
                return;
            }
            buf.truncate(len);
            args.push(String::from_utf8_lossy(&buf).to_string());
        }
        let reply = fake_redis_execute(&args, store);
        if stream.write_all(reply.as_bytes()).is_err() {
            return;
        }
    }
}

fn fake_redis_execute(args: &[String], store: &Mutex<Map<String, String>>) -> String {
    let mut guard = store.lock().unwrap();
    match args[0].to_uppercase().as_str() {
        "SET" => {
            let key = &args[1];
            let value = &args[2];
            guard.insert(key.clone(), value.clone());
            "+OK\r\n".to_string()
        }
        "GET" => match guard.get(&args[1]) {
            Some(v) => format!("${}\r\n{}\r\n", v.len(), v),
            None => "$-1\r\n".to_string(),
        },
        "DEL" => {
            let existed = guard.remove(&args[1]).is_some();
            format!(":{}\r\n", if existed { 1 } else { 0 })
        }
        _ => "-ERR unknown command\r\n".to_string(),
    }
}

#[test]
fn redis_starts_closed() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 3, 30);
    assert!(cb.is_available("x").unwrap(), "new backend should be available");
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap());
}

#[test]
fn redis_opens_after_threshold() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 3, 30);
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap());
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap());
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb.state("x").unwrap(), "should open after threshold failures");
    assert!(!cb.is_available("x").unwrap(), "open circuit should not be available");
}

#[test]
fn redis_half_opens_after_recovery() {
    let addr = start_fake_redis();
    // Zero-second recovery so the transition happens on the very next check.
    let cb = RedisCircuitBreaker::new(addr, None, 1, 0);
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb.state("x").unwrap());
    // No sleep needed: recovery=0 means "elapsed >= 0", which a saturating
    // unsigned-seconds difference always satisfies, even in the same second.
    assert!(cb.is_available("x").unwrap(), "should be available (half-open) after recovery window");
    assert_eq!(BreakerState::HalfOpen, cb.state("x").unwrap());
}

#[test]
fn redis_closes_after_success_in_half_open() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 1, 0);
    cb.record_failure("x").unwrap();
    let _ = cb.is_available("x").unwrap(); // transition to HalfOpen (recovery=0)
    assert_eq!(BreakerState::HalfOpen, cb.state("x").unwrap());
    cb.record_success("x").unwrap();
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap());
    assert!(cb.is_available("x").unwrap());
}

#[test]
fn redis_reopens_after_failure_in_half_open() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 1, 0);
    cb.record_failure("x").unwrap();
    let _ = cb.is_available("x").unwrap(); // transition to HalfOpen (recovery=0)
    assert_eq!(BreakerState::HalfOpen, cb.state("x").unwrap());
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb.state("x").unwrap(), "failure in HalfOpen should re-open");
}

#[test]
fn redis_reset_clears_state() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 2, 30);
    cb.record_failure("x").unwrap();
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb.state("x").unwrap());
    cb.reset("x").unwrap();
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap());
    assert!(cb.is_available("x").unwrap());
}

#[test]
fn redis_independent_backends() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 2, 30);
    cb.record_failure("a").unwrap();
    cb.record_failure("a").unwrap();
    assert_eq!(BreakerState::Open, cb.state("a").unwrap());
    assert_eq!(BreakerState::Closed, cb.state("b").unwrap());
    assert!(cb.is_available("b").unwrap());
}

#[test]
fn redis_state_survives_a_new_instance_pointed_at_the_same_backend() {
    // A fresh RedisCircuitBreaker connecting to the same Redis server is a
    // stand-in for "the rws process restarted" — the whole point of this
    // feature. Since state lives in Redis (the fake server here), not in
    // the RedisCircuitBreaker struct itself, a brand new instance must see
    // exactly the state the previous one left behind.
    let addr = start_fake_redis();
    let cb1 = RedisCircuitBreaker::new(addr.clone(), None, 2, 30);
    cb1.record_failure("x").unwrap();
    cb1.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb1.state("x").unwrap());
    drop(cb1);

    let cb2 = RedisCircuitBreaker::new(addr, None, 2, 30);
    assert_eq!(BreakerState::Open, cb2.state("x").unwrap(), "state must persist across a fresh instance");
    assert!(!cb2.is_available("x").unwrap());
}

#[test]
fn redis_clone_shares_the_same_persisted_state() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 2, 30);
    let cloned = cb.clone();
    cb.record_failure("x").unwrap();
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cloned.state("x").unwrap());
}

#[test]
fn redis_set_limits_takes_effect_immediately() {
    let addr = start_fake_redis();
    let cb = RedisCircuitBreaker::new(addr, None, 1, 30);
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Open, cb.state("x").unwrap());
    cb.reset("x").unwrap();

    cb.set_limits(5, 30);
    cb.record_failure("x").unwrap();
    assert_eq!(BreakerState::Closed, cb.state("x").unwrap(), "new higher threshold should apply");
}

#[test]
fn redis_operations_error_when_server_unreachable() {
    // Port 1 is privileged and effectively never listening — connection
    // should fail immediately (refused).
    let cb = RedisCircuitBreaker::new("127.0.0.1:1", None, 5, 30);
    assert!(cb.is_available("x").is_err());
    assert!(cb.record_failure("x").is_err());
    assert!(cb.record_success("x").is_err());
    assert!(cb.reset("x").is_err());
    assert!(cb.state("x").is_err());
}
