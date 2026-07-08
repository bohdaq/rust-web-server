//! Circuit breaker state machine and retry middleware.
//!
//! # Circuit breaker
//!
//! [`CircuitBreaker`] tracks per-backend failure counts and transitions through
//! three states:
//!
//! * **Closed** — the backend is healthy; failures are counted.  When the count
//!   reaches `failure_threshold` the breaker moves to **Open**.
//! * **Open** — the backend is considered unhealthy; all requests are rejected
//!   immediately (no TCP connection is attempted).  After `recovery` seconds the
//!   breaker moves to **HalfOpen**.
//! * **HalfOpen** — one probe request is let through.  On success the breaker
//!   closes; on failure it re-opens and the recovery timer resets.
//!
//! # Retry middleware
//!
//! [`RetryLayer`] wraps any [`Application`] and re-dispatches the request when
//! the inner app returns one of the configured status codes (default: 502, 503,
//! 504) up to `max_retries` additional times.
//!
//! # Persistence
//!
//! [`CircuitBreaker`] keeps state in a plain in-process `HashMap` — a restart
//! (or a deploy) resets every backend back to `Closed`, so a backend that
//! tripped the breaker moments before a restart looks healthy again
//! immediately, and may cascade failures again before anything notices.
//! [`RedisCircuitBreaker`] has the same state machine and the same
//! `is_available`/`record_success`/`record_failure`/`reset`/`state` shape, but
//! persists each backend's state in Redis (via the same hand-rolled RESP
//! client [`crate::rate_limit::RedisRateLimiter`] and
//! [`crate::session::RedisSessionStore`] use) — surviving a restart, and
//! shared across every `rws` instance pointed at that Redis server.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::circuit_breaker::RetryLayer;
//! use rust_web_server::middleware::WithMiddleware;
//!
//! let app = WithMiddleware::new(App::new())
//!     .wrap(RetryLayer::new().max_retries(2));
//! ```

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::redis_protocol::{RespConn, RespReply};
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

// ── BreakerState ─────────────────────────────────────────────────────────────

/// Current state of a single backend's circuit breaker.
#[derive(Debug, Clone, PartialEq)]
pub enum BreakerState {
    /// Healthy — requests are forwarded and failures are counted.
    Closed,
    /// Unhealthy — requests are rejected until the recovery window expires.
    Open,
    /// Probing — one request is let through to test backend health.
    HalfOpen,
}

// ── BackendEntry ──────────────────────────────────────────────────────────────

struct BackendEntry {
    state: BreakerState,
    failures: u32,
    opened_at: Option<Instant>,
    /// Probes currently let through while `state == HalfOpen`. Capped by
    /// [`CircuitBreaker::max_half_open_probes`] so a burst of concurrent
    /// requests arriving the instant a backend transitions to `HalfOpen`
    /// doesn't all land as "the" trial request at once.
    half_open_in_flight: u32,
}

impl BackendEntry {
    fn new() -> Self {
        Self { state: BreakerState::Closed, failures: 0, opened_at: None, half_open_in_flight: 0 }
    }
}

// ── CircuitBreaker ────────────────────────────────────────────────────────────

/// Per-backend circuit breaker.
///
/// # Concurrency
///
/// `CircuitBreaker` is not `Sync` on its own — wrap it in a [`Mutex`] for
/// shared use across threads (see [`global()`]).
pub struct CircuitBreaker {
    backends: HashMap<String, BackendEntry>,
    failure_threshold: u32,
    recovery: Duration,
    max_half_open_probes: u32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// * `failure_threshold` — how many consecutive failures open the circuit.
    /// * `recovery_secs` — how long the circuit stays Open before testing again.
    ///
    /// Defaults to letting exactly one probe through while `HalfOpen` — see
    /// [`CircuitBreaker::max_half_open_probes`] to change that.
    pub fn new(failure_threshold: u32, recovery_secs: u64) -> Self {
        Self {
            backends: HashMap::new(),
            failure_threshold,
            recovery: Duration::from_secs(recovery_secs),
            max_half_open_probes: 1,
        }
    }

    /// Override how many concurrent probe requests are let through while a
    /// backend is `HalfOpen` (default: 1). Chainable — call before the
    /// breaker is put behind a shared `Mutex`/`Arc`.
    pub fn max_half_open_probes(mut self, n: u32) -> Self {
        self.max_half_open_probes = n.max(1);
        self
    }

    /// Returns `true` if a request should be forwarded to `backend`.
    ///
    /// Transitions `Open → HalfOpen` when the recovery window has elapsed.
    /// While `HalfOpen`, at most `max_half_open_probes` concurrent calls
    /// return `true` — further calls are rejected like `Open` until the
    /// in-flight probe(s) resolve via `record_success`/`record_failure`.
    pub fn is_available(&mut self, backend: &str) -> bool {
        let max_probes = self.max_half_open_probes;
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        match entry.state {
            BreakerState::Closed => true,
            BreakerState::HalfOpen => {
                if entry.half_open_in_flight < max_probes {
                    entry.half_open_in_flight += 1;
                    true
                } else {
                    false
                }
            }
            BreakerState::Open => {
                if let Some(opened_at) = entry.opened_at {
                    if opened_at.elapsed() >= self.recovery {
                        entry.state = BreakerState::HalfOpen;
                        entry.opened_at = None;
                        entry.half_open_in_flight = 1;
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Record a successful response for `backend`.
    ///
    /// Transitions `HalfOpen → Closed` and resets the failure counter and
    /// the in-flight half-open probe count.
    pub fn record_success(&mut self, backend: &str) {
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        entry.state = BreakerState::Closed;
        entry.failures = 0;
        entry.opened_at = None;
        entry.half_open_in_flight = 0;
    }

    /// Record a failed response for `backend`.
    ///
    /// In `Closed` state, increments the counter and opens the circuit when
    /// `failure_threshold` is reached.  In `HalfOpen` state, immediately
    /// re-opens the circuit and resets the recovery timer.
    pub fn record_failure(&mut self, backend: &str) {
        let threshold = self.failure_threshold;
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        match entry.state {
            BreakerState::Closed => {
                entry.failures += 1;
                if entry.failures >= threshold {
                    entry.state = BreakerState::Open;
                    entry.opened_at = Some(Instant::now());
                }
            }
            BreakerState::HalfOpen => {
                entry.state = BreakerState::Open;
                entry.opened_at = Some(Instant::now());
                entry.half_open_in_flight = 0;
            }
            BreakerState::Open => {
                // Already open; refresh the timer.
                entry.opened_at = Some(Instant::now());
            }
        }
    }

    /// Reset `backend` to `Closed` with zero failures.
    pub fn reset(&mut self, backend: &str) {
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        entry.state = BreakerState::Closed;
        entry.failures = 0;
        entry.opened_at = None;
        entry.half_open_in_flight = 0;
    }

    /// Return the current state for `backend` (defaults to `Closed` if unseen).
    pub fn state(&self, backend: &str) -> BreakerState {
        self.backends
            .get(backend)
            .map(|e| e.state.clone())
            .unwrap_or(BreakerState::Closed)
    }

    /// Snapshot of every backend this breaker has ever seen, with its
    /// current state. Powers the `rws_circuit_breaker_state{backend}`
    /// metric (`crate::metrics::prometheus_text`) — there is no
    /// `RedisCircuitBreaker` equivalent, since enumerating keys isn't
    /// something the minimal hand-rolled RESP client supports (no
    /// `SCAN`/`KEYS` array-reply decoding).
    pub fn all_states(&self) -> Vec<(String, BreakerState)> {
        self.backends.iter().map(|(k, v)| (k.clone(), v.state.clone())).collect()
    }
}

// ── global() ─────────────────────────────────────────────────────────────────

static GLOBAL_BREAKER: OnceLock<Mutex<CircuitBreaker>> = OnceLock::new();

/// Return the process-wide default circuit breaker (threshold=5, recovery=30 s).
///
/// Acquire the mutex before calling any `CircuitBreaker` method:
///
/// ```rust
/// use rust_web_server::circuit_breaker;
///
/// let available = circuit_breaker::global().lock().unwrap().is_available("backend-a:8080");
/// ```
pub fn global() -> &'static Mutex<CircuitBreaker> {
    GLOBAL_BREAKER.get_or_init(|| Mutex::new(CircuitBreaker::new(5, 30)))
}

// ── RedisCircuitBreaker ─────────────────────────────────────────────────────────

/// Per-backend circuit breaker, persisted in Redis.
///
/// Same Closed → Open → HalfOpen state machine as [`CircuitBreaker`], and the
/// same method names, but every operation is a synchronous Redis round trip
/// (via the shared [`crate::redis_protocol`] RESP client) instead of an
/// in-memory `HashMap` update — so state survives a process restart, and is
/// shared across every `rws` instance pointed at the same Redis server.
///
/// # Why Redis, not the model layer
///
/// The model layer (`DbPool`, `sqlx`) is `async fn`-only, while
/// `CircuitBreaker`'s methods and `Middleware::handle` (what [`RetryLayer`]
/// implements) are both synchronous — the identical async/sync mismatch that
/// left `SqliteRateLimiter` unbuilt after [`crate::rate_limit::RedisRateLimiter`]
/// shipped. Redis, reached over a plain blocking `TcpStream`, stays fully
/// synchronous and drops into the same call sites `CircuitBreaker` already
/// has, with no new Cargo dependency.
///
/// # Consistency
///
/// Each operation is a read-then-write (`GET` then `SET`) against one Redis
/// key per backend — not a single atomic command. Two `rws` instances racing
/// to record a failure for the same backend at the same instant can lose one
/// of the two increments. This is a deliberate simplification: unlike a rate
/// limit (a hard resource/security boundary, where `RedisRateLimiter` uses
/// genuinely atomic `INCR` for exactly this reason), a circuit breaker is a
/// self-healing heuristic where opening one failure late — or one request
/// later than a perfectly-synchronized count would — has no real consequence.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::circuit_breaker::RedisCircuitBreaker;
///
/// let breaker = RedisCircuitBreaker::new("127.0.0.1:6379", None, 5, 30);
///
/// match breaker.is_available("backend-a:8080") {
///     Ok(true) => { /* forward the request */ }
///     Ok(false) => { /* short-circuit — return 503 without contacting the backend */ }
///     Err(e) => { /* Redis unreachable — decide fail-open vs fail-closed yourself */ }
/// }
/// ```
pub struct RedisCircuitBreaker {
    conn: Arc<RespConn>,
    failure_threshold: AtomicU32,
    recovery_secs: AtomicU64,
    max_half_open_probes: AtomicU32,
}

impl Clone for RedisCircuitBreaker {
    fn clone(&self) -> Self {
        RedisCircuitBreaker {
            conn: Arc::clone(&self.conn),
            failure_threshold: AtomicU32::new(self.failure_threshold.load(Ordering::Relaxed)),
            recovery_secs: AtomicU64::new(self.recovery_secs.load(Ordering::Relaxed)),
            max_half_open_probes: AtomicU32::new(self.max_half_open_probes.load(Ordering::Relaxed)),
        }
    }
}

impl RedisCircuitBreaker {
    /// Create a breaker that connects to `addr` (e.g. `"127.0.0.1:6379"`).
    /// `password` is passed to Redis `AUTH` if `Some`. Defaults to letting
    /// exactly one probe through while `HalfOpen` — see
    /// [`RedisCircuitBreaker::set_max_half_open_probes`] to change that.
    pub fn new(addr: impl Into<String>, password: Option<String>, failure_threshold: u32, recovery_secs: u64) -> Self {
        RedisCircuitBreaker {
            conn: Arc::new(RespConn::new(addr, password)),
            failure_threshold: AtomicU32::new(failure_threshold),
            recovery_secs: AtomicU64::new(recovery_secs),
            max_half_open_probes: AtomicU32::new(1),
        }
    }

    /// Build a breaker from environment variables:
    /// - `RWS_REDIS_HOST` (default `127.0.0.1`)
    /// - `RWS_REDIS_PORT` (default `6379`)
    /// - `RWS_REDIS_PASSWORD` (optional)
    /// - `RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD` (default `5`)
    /// - `RWS_CONFIG_CIRCUIT_BREAKER_RECOVERY_SECS` (default `30`)
    /// - `RWS_CONFIG_CIRCUIT_BREAKER_MAX_HALF_OPEN_PROBES` (default `1`)
    pub fn from_env() -> Self {
        let host = std::env::var("RWS_REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("RWS_REDIS_PORT").unwrap_or_else(|_| "6379".into());
        let addr = format!("{}:{}", host, port);
        let password = std::env::var("RWS_REDIS_PASSWORD").ok();
        let failure_threshold = std::env::var("RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let recovery_secs = std::env::var("RWS_CONFIG_CIRCUIT_BREAKER_RECOVERY_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let breaker = Self::new(addr, password, failure_threshold, recovery_secs);
        if let Some(n) = std::env::var("RWS_CONFIG_CIRCUIT_BREAKER_MAX_HALF_OPEN_PROBES").ok().and_then(|v| v.parse().ok()) {
            breaker.set_max_half_open_probes(n);
        }
        breaker
    }

    /// Update the thresholds on a live breaker without restarting.
    pub fn set_limits(&self, failure_threshold: u32, recovery_secs: u64) {
        self.failure_threshold.store(failure_threshold, Ordering::Relaxed);
        self.recovery_secs.store(recovery_secs, Ordering::Relaxed);
    }

    /// Override how many concurrent probe requests are let through while a
    /// backend is `HalfOpen` (default: 1) — on a live breaker, no restart
    /// required.
    pub fn set_max_half_open_probes(&self, n: u32) {
        self.max_half_open_probes.store(n.max(1), Ordering::Relaxed);
    }

    fn redis_key(backend: &str) -> Vec<u8> {
        format!("rws:cb:{}", backend).into_bytes()
    }

    fn load(&self, backend: &str) -> std::io::Result<(BreakerState, u32, u64, u32)> {
        match self.conn.cmd(&[b"GET", &Self::redis_key(backend)])? {
            RespReply::Bulk(Some(bytes)) => Ok(decode_entry(&bytes)),
            _ => Ok((BreakerState::Closed, 0, 0, 0)),
        }
    }

    fn store(&self, backend: &str, state: &BreakerState, failures: u32, opened_at: u64, half_open_in_flight: u32) -> std::io::Result<()> {
        let encoded = encode_entry(state, failures, opened_at, half_open_in_flight);
        self.conn.cmd(&[b"SET", &Self::redis_key(backend), encoded.as_bytes()])?;
        Ok(())
    }

    /// Returns `Ok(true)` if a request should be forwarded to `backend`.
    ///
    /// Transitions `Open → HalfOpen` when the recovery window has elapsed.
    /// While `HalfOpen`, at most `max_half_open_probes` concurrent calls
    /// return `Ok(true)` — this is a read-then-write against one Redis key
    /// (not atomic), the same deliberate simplification already documented
    /// above for this type, so a race between two instances can very rarely
    /// let one extra probe through; it cannot let an unbounded burst through.
    /// Returns `Err` if Redis is unreachable — callers decide whether that
    /// means fail open (treat as available) or fail closed (treat as not).
    pub fn is_available(&self, backend: &str) -> std::io::Result<bool> {
        let (state, failures, opened_at, half_open_in_flight) = self.load(backend)?;
        match state {
            BreakerState::Closed => Ok(true),
            BreakerState::HalfOpen => {
                let max_probes = self.max_half_open_probes.load(Ordering::Relaxed);
                if half_open_in_flight < max_probes {
                    self.store(backend, &BreakerState::HalfOpen, failures, opened_at, half_open_in_flight + 1)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            BreakerState::Open => {
                let recovery = self.recovery_secs.load(Ordering::Relaxed);
                if now_unix().saturating_sub(opened_at) >= recovery {
                    self.store(backend, &BreakerState::HalfOpen, failures, 0, 1)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Record a successful response for `backend`.
    ///
    /// Transitions `HalfOpen → Closed` and resets the failure counter and
    /// the in-flight half-open probe count.
    pub fn record_success(&self, backend: &str) -> std::io::Result<()> {
        self.store(backend, &BreakerState::Closed, 0, 0, 0)
    }

    /// Record a failed response for `backend`.
    ///
    /// In `Closed` state, increments the counter and opens the circuit when
    /// `failure_threshold` is reached. In `HalfOpen` state, immediately
    /// re-opens the circuit and resets the recovery timer.
    pub fn record_failure(&self, backend: &str) -> std::io::Result<()> {
        let (state, failures, _, _) = self.load(backend)?;
        match state {
            BreakerState::Closed => {
                let failures = failures + 1;
                if failures >= self.failure_threshold.load(Ordering::Relaxed) {
                    self.store(backend, &BreakerState::Open, failures, now_unix(), 0)
                } else {
                    self.store(backend, &BreakerState::Closed, failures, 0, 0)
                }
            }
            BreakerState::HalfOpen | BreakerState::Open => {
                self.store(backend, &BreakerState::Open, failures, now_unix(), 0)
            }
        }
    }

    /// Reset `backend` to `Closed` with zero failures.
    pub fn reset(&self, backend: &str) -> std::io::Result<()> {
        self.conn.cmd(&[b"DEL", &Self::redis_key(backend)])?;
        Ok(())
    }

    /// Return the current state for `backend` (defaults to `Closed` if unseen).
    pub fn state(&self, backend: &str) -> std::io::Result<BreakerState> {
        Ok(self.load(backend)?.0)
    }
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `"state|failures|opened_at|half_open_in_flight"` — a plain-string encoding
/// chosen so a backend's whole entry fits in one Redis key read via
/// `GET`/written via `SET`, rather than a hash needing `HGETALL` (which
/// `redis_protocol`'s minimal RESP client doesn't decode — it only handles
/// simple/bulk/integer replies, not arrays).
fn encode_entry(state: &BreakerState, failures: u32, opened_at: u64, half_open_in_flight: u32) -> String {
    let state_str = match state {
        BreakerState::Closed => "closed",
        BreakerState::Open => "open",
        BreakerState::HalfOpen => "half_open",
    };
    format!("{}|{}|{}|{}", state_str, failures, opened_at, half_open_in_flight)
}

fn decode_entry(raw: &[u8]) -> (BreakerState, u32, u64, u32) {
    let text = String::from_utf8_lossy(raw);
    let mut parts = text.splitn(4, '|');
    let state = match parts.next() {
        Some("open") => BreakerState::Open,
        Some("half_open") => BreakerState::HalfOpen,
        _ => BreakerState::Closed,
    };
    let failures = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let opened_at = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let half_open_in_flight = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (state, failures, opened_at, half_open_in_flight)
}

// ── Breaker trait (ReverseProxy auto-wiring) ─────────────────────────────────

/// Object-safe interface implemented by both `Mutex<CircuitBreaker>` and
/// [`RedisCircuitBreaker`], so a consumer like
/// [`crate::proxy::ReverseProxy::with_circuit_breaker`] can accept either
/// breaker kind through one `Arc<dyn Breaker>` without a generic parameter
/// leaking into its own type.
///
/// `RedisCircuitBreaker`'s `Err` (Redis unreachable) is treated as *available*
/// — failing open, not closed. A breaker that can't be reached is a worse
/// single point of failure than occasionally skipping the check it exists to
/// perform; callers who need fail-closed semantics should call
/// `RedisCircuitBreaker::is_available` directly and decide for themselves, as
/// its own docs already describe.
pub trait Breaker: Send + Sync {
    /// Returns `true` if a request should be forwarded to `backend`.
    fn is_available(&self, backend: &str) -> bool;
    /// Record a successful response for `backend`.
    fn record_success(&self, backend: &str);
    /// Record a failed response for `backend`.
    fn record_failure(&self, backend: &str);
}

impl Breaker for Mutex<CircuitBreaker> {
    fn is_available(&self, backend: &str) -> bool {
        self.lock().unwrap().is_available(backend)
    }
    fn record_success(&self, backend: &str) {
        self.lock().unwrap().record_success(backend);
    }
    fn record_failure(&self, backend: &str) {
        self.lock().unwrap().record_failure(backend);
    }
}

impl Breaker for RedisCircuitBreaker {
    fn is_available(&self, backend: &str) -> bool {
        RedisCircuitBreaker::is_available(self, backend).unwrap_or(true)
    }
    fn record_success(&self, backend: &str) {
        let _ = RedisCircuitBreaker::record_success(self, backend);
    }
    fn record_failure(&self, backend: &str) {
        let _ = RedisCircuitBreaker::record_failure(self, backend);
    }
}

/// Lets `circuit_breaker::global()` (which returns `&'static Mutex<CircuitBreaker>`,
/// not an owned `Mutex<CircuitBreaker>`) be used directly as a `Breaker`:
/// `Arc::new(circuit_breaker::global())` then coerces to `Arc<dyn Breaker>`.
impl<T: Breaker + ?Sized> Breaker for &T {
    fn is_available(&self, backend: &str) -> bool {
        (**self).is_available(backend)
    }
    fn record_success(&self, backend: &str) {
        (**self).record_success(backend);
    }
    fn record_failure(&self, backend: &str) {
        (**self).record_failure(backend);
    }
}

// ── RetryLayer ────────────────────────────────────────────────────────────────

/// Retry middleware.
///
/// When the inner application returns a response whose status code is in the
/// configured list, the request is re-dispatched up to `max_retries` additional
/// times.  If all attempts return a retryable status the last response is
/// returned as-is.
pub struct RetryLayer {
    max_retries: u32,
    retry_on: Vec<i16>,
}

impl RetryLayer {
    /// Create a `RetryLayer` with defaults: retry on 502, 503, 504 up to 3 times.
    pub fn new() -> Self {
        Self { max_retries: 3, retry_on: vec![502, 503, 504] }
    }

    /// Override the maximum number of retry attempts.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Override the set of status codes that trigger a retry.
    pub fn retry_on(mut self, codes: Vec<i16>) -> Self {
        self.retry_on = codes;
        self
    }
}

impl Default for RetryLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for RetryLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let mut response = next.execute(request, connection)?;
        let mut attempts = 0u32;
        while attempts < self.max_retries && self.retry_on.contains(&response.status_code) {
            response = next.execute(request, connection)?;
            attempts += 1;
        }
        Ok(response)
    }
}
