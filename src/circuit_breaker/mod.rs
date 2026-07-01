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
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::middleware::Middleware;
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
}

impl BackendEntry {
    fn new() -> Self {
        Self { state: BreakerState::Closed, failures: 0, opened_at: None }
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
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// * `failure_threshold` — how many consecutive failures open the circuit.
    /// * `recovery_secs` — how long the circuit stays Open before testing again.
    pub fn new(failure_threshold: u32, recovery_secs: u64) -> Self {
        Self {
            backends: HashMap::new(),
            failure_threshold,
            recovery: Duration::from_secs(recovery_secs),
        }
    }

    /// Returns `true` if a request should be forwarded to `backend`.
    ///
    /// Transitions `Open → HalfOpen` when the recovery window has elapsed.
    pub fn is_available(&mut self, backend: &str) -> bool {
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        match entry.state {
            BreakerState::Closed => true,
            BreakerState::HalfOpen => true,
            BreakerState::Open => {
                if let Some(opened_at) = entry.opened_at {
                    if opened_at.elapsed() >= self.recovery {
                        entry.state = BreakerState::HalfOpen;
                        entry.opened_at = None;
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Record a successful response for `backend`.
    ///
    /// Transitions `HalfOpen → Closed` and resets the failure counter.
    pub fn record_success(&mut self, backend: &str) {
        let entry = self.backends.entry(backend.to_string()).or_insert_with(BackendEntry::new);
        entry.state = BreakerState::Closed;
        entry.failures = 0;
        entry.opened_at = None;
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
    }

    /// Return the current state for `backend` (defaults to `Closed` if unseen).
    pub fn state(&self, backend: &str) -> BreakerState {
        self.backends
            .get(backend)
            .map(|e| e.state.clone())
            .unwrap_or(BreakerState::Closed)
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
