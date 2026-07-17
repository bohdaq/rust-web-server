#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A sliding-window per-key rate limiter.
///
/// Each call to [`RateLimiter::check`] records a timestamp for the given key
/// and returns `true` when the number of calls within the current window is
/// still below `max_requests`. Returns `false` once the limit is exceeded.
///
/// Thread-safe: the internal state is behind a `Mutex` so it can be shared
/// across threads via [`global`] or wrapped in an `Arc`.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::rate_limit::RateLimiter;
///
/// let limiter = RateLimiter::new(100, 60); // 100 req / 60 s
///
/// if limiter.check("192.168.1.1") {
///     // process request
/// } else {
///     // return 429 Too Many Requests
/// }
/// ```
pub struct RateLimiter {
    state: Mutex<HashMap<String, VecDeque<Instant>>>,
    max_requests: AtomicU32,
    window_secs: AtomicU64,
}

impl RateLimiter {
    /// Create a new limiter allowing `max_requests` per `window_secs`-second window.
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        RateLimiter {
            state: Mutex::new(HashMap::new()),
            max_requests: AtomicU32::new(max_requests),
            window_secs: AtomicU64::new(window_secs),
        }
    }

    /// Update the limits on a live limiter without restarting.
    ///
    /// Changes take effect on the next call to [`check`] or [`remaining`].
    /// Called automatically by [`crate::config_reload::reload`] on SIGHUP.
    pub fn set_limits(&self, max_requests: u32, window_secs: u64) {
        self.max_requests.store(max_requests, Ordering::Relaxed);
        self.window_secs.store(window_secs, Ordering::Relaxed);
    }

    fn window(&self) -> Duration {
        Duration::from_secs(self.window_secs.load(Ordering::Relaxed))
    }

    fn max(&self) -> u32 {
        self.max_requests.load(Ordering::Relaxed)
    }

    /// Returns `true` if `key` (typically a client IP) is within the rate limit,
    /// or `false` if the limit has been exceeded.
    ///
    /// A permitted call is always recorded so it counts toward future limits.
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = self.window();
        let max = self.max();
        let mut guard = self.state.lock().unwrap();
        let timestamps = guard.entry(key.to_string()).or_default();

        // Drop timestamps older than the window.
        while timestamps.front().map(|t| now.duration_since(*t) > window).unwrap_or(false) {
            timestamps.pop_front();
        }

        if (timestamps.len() as u32) < max {
            timestamps.push_back(now);
            true
        } else {
            false
        }
    }

    /// Number of remaining requests `key` may make within the current window.
    pub fn remaining(&self, key: &str) -> u32 {
        let now = Instant::now();
        let window = self.window();
        let max = self.max();
        let mut guard = self.state.lock().unwrap();
        let timestamps = guard.entry(key.to_string()).or_default();
        while timestamps.front().map(|t| now.duration_since(*t) > window).unwrap_or(false) {
            timestamps.pop_front();
        }
        max.saturating_sub(timestamps.len() as u32)
    }

    /// Remove all tracked state for `key`. Useful in tests.
    pub fn reset(&self, key: &str) {
        self.state.lock().unwrap().remove(key);
    }
}

// ── RedisRateLimiter ──────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use crate::redis_protocol::{RespConn, RespReply};

/// A distributed, fixed-window rate limiter backed by a Redis server.
///
/// [`RateLimiter`] only protects a single process: two `rws` instances behind
/// a load balancer each track their own in-memory counters, so the effective
/// limit for a client doubles (or worse) as replicas scale out. `RedisRateLimiter`
/// keys the counter on a shared Redis server instead, so every instance enforces
/// the same budget.
///
/// Unlike [`RateLimiter`]'s sliding window (a deque of timestamps), this uses a
/// fixed-window counter: `key`'s count lives under one Redis key with a TTL equal
/// to the window length, incremented atomically via `INCR`. This is the standard
/// distributed rate-limiting pattern — Redis guarantees `INCR` is atomic across
/// concurrent callers, so counters stay correct even under heavy concurrent load
/// from multiple `rws` processes. The tradeoff versus a sliding window: a client
/// can burst up to `2x max_requests` across a window boundary (once near the end
/// of one window, once at the start of the next).
///
/// Cloning is cheap — all clones share the same underlying TCP connection (one
/// persistent connection per `RedisRateLimiter` instance).
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::rate_limit::RedisRateLimiter;
///
/// let limiter = RedisRateLimiter::new("127.0.0.1:6379", None, 100, 60); // 100 req / 60s
///
/// match limiter.check("192.168.1.1") {
///     Ok(true) => { /* process request */ }
///     Ok(false) => { /* return 429 Too Many Requests */ }
///     Err(e) => { /* Redis unreachable — decide fail-open vs fail-closed */ }
/// }
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub struct RedisRateLimiter {
    conn: Arc<RespConn>,
    max_requests: AtomicU32,
    window_secs: AtomicU64,
}

#[cfg(not(target_arch = "wasm32"))]
impl Clone for RedisRateLimiter {
    fn clone(&self) -> Self {
        RedisRateLimiter {
            conn: Arc::clone(&self.conn),
            max_requests: AtomicU32::new(self.max()),
            window_secs: AtomicU64::new(self.window().as_secs()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RedisRateLimiter {
    /// Create a limiter that connects to `addr` (e.g. `"127.0.0.1:6379"`),
    /// allowing `max_requests` per `window_secs`-second window.
    /// `password` is passed to Redis `AUTH` if `Some`.
    pub fn new(addr: impl Into<String>, password: Option<String>, max_requests: u32, window_secs: u64) -> Self {
        RedisRateLimiter {
            conn: Arc::new(RespConn::new(addr, password)),
            max_requests: AtomicU32::new(max_requests),
            window_secs: AtomicU64::new(window_secs),
        }
    }

    /// Build a limiter from environment variables:
    /// - `RWS_REDIS_HOST` (default `127.0.0.1`)
    /// - `RWS_REDIS_PORT` (default `6379`)
    /// - `RWS_REDIS_PASSWORD` (optional)
    /// - `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` (default `1000`)
    /// - `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` (default `60`)
    pub fn from_env() -> Self {
        let host = std::env::var("RWS_REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("RWS_REDIS_PORT").unwrap_or_else(|_| "6379".into());
        let addr = format!("{}:{}", host, port);
        let password = std::env::var("RWS_REDIS_PASSWORD").ok();
        let max: u32 = std::env::var("RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);
        let window: u64 = std::env::var("RWS_CONFIG_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        Self::new(addr, password, max, window)
    }

    /// Update the limits on a live limiter without restarting.
    pub fn set_limits(&self, max_requests: u32, window_secs: u64) {
        self.max_requests.store(max_requests, Ordering::Relaxed);
        self.window_secs.store(window_secs, Ordering::Relaxed);
    }

    fn window(&self) -> Duration {
        Duration::from_secs(self.window_secs.load(Ordering::Relaxed))
    }

    fn max(&self) -> u32 {
        self.max_requests.load(Ordering::Relaxed)
    }

    fn redis_key(key: &str) -> Vec<u8> {
        format!("rws:ratelimit:{}", key).into_bytes()
    }

    /// Returns `Ok(true)` if `key` (typically a client IP) is within the rate
    /// limit, `Ok(false)` if the limit has been exceeded, or `Err` if the
    /// Redis server could not be reached.
    ///
    /// A call always increments the shared counter, whether permitted or not,
    /// so callers must decide for themselves whether to fail open (allow the
    /// request) or fail closed (deny it) on `Err` — this limiter does not
    /// silently pick one.
    pub fn check(&self, key: &str) -> std::io::Result<bool> {
        let redis_key = Self::redis_key(key);
        let window = self.window().as_secs().to_string();
        // Atomically create the key with a TTL the first time it's seen. If
        // it already exists, `SET ... NX` is a no-op — this only races once,
        // at creation, and `SET NX` is itself atomic on the Redis server.
        self.conn.cmd(&[b"SET", &redis_key, b"0", b"EX", window.as_bytes(), b"NX"])?;
        let count = match self.conn.cmd(&[b"INCR", &redis_key])? {
            RespReply::Int(n) => n,
            _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "unexpected INCR reply")),
        };
        Ok(count as u32 <= self.max())
    }

    /// Number of remaining requests `key` may make within the current window,
    /// or `Err` if the Redis server could not be reached.
    pub fn remaining(&self, key: &str) -> std::io::Result<u32> {
        match self.conn.cmd(&[b"GET", &Self::redis_key(key)])? {
            RespReply::Bulk(Some(bytes)) => {
                let count: u32 = String::from_utf8_lossy(&bytes).parse().unwrap_or(0);
                Ok(self.max().saturating_sub(count))
            }
            _ => Ok(self.max()),
        }
    }

    /// Remove all tracked state for `key`. Useful in tests.
    pub fn reset(&self, key: &str) -> std::io::Result<()> {
        self.conn.cmd(&[b"DEL", &Self::redis_key(key)])?;
        Ok(())
    }
}

static GLOBAL_LIMITER: OnceLock<RateLimiter> = OnceLock::new();

/// Return the process-wide rate limiter, initialized from environment variables.
///
/// | Variable | Default | Meaning |
/// |---|---|---|
/// | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `1000` | Requests allowed per window |
/// | `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `60` | Window length in seconds |
///
/// Returns `None` when rate limiting is disabled (`RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS=0`).
pub fn global() -> &'static RateLimiter {
    GLOBAL_LIMITER.get_or_init(|| {
        let max: u32 = std::env::var("RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);
        let window: u64 = std::env::var("RWS_CONFIG_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        RateLimiter::new(max, window)
    })
}
