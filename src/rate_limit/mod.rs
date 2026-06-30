#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
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
