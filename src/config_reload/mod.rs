//! Hot configuration reload.
//!
//! Re-reads `rws.config.toml` without restarting the server. Non-binding
//! settings — those that would require a new TCP socket (IP, port,
//! thread count) or a new TLS acceptor (cert/key paths) — are logged as
//! ignored. All other settings take effect on the next incoming request.
//!
//! # Triggering a reload
//!
//! * **Unix signal**: send `SIGHUP` to the server process.
//!   ```bash
//!   kill -HUP $(pidof rws)
//!   ```
//! * **HTTP endpoint**: `POST /admin/config/reload` (no body required).
//!
//! # What is hot-reloadable
//!
//! | Setting | Env var |
//! |---------|---------|
//! | CORS — all fields | `RWS_CONFIG_CORS_*` |
//! | Rate-limit thresholds | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS`, `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` |
//! | Log format | `RWS_CONFIG_LOG_FORMAT` |
//! | Request allocation size | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |
//! | Max body size | `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` |
//!
//! # What is NOT hot-reloadable (requires restart)
//!
//! | Setting | Why |
//! |---------|-----|
//! | IP / Port | Bound socket cannot be moved |
//! | Thread count | Thread pool is fixed at startup |
//! | TLS cert / key | TLS acceptor is built once at startup |

#[cfg(test)]
mod tests;

use std::sync::{OnceLock, RwLock};
use std::sync::atomic::AtomicBool;
#[cfg(all(unix, feature = "http1"))]
use std::sync::atomic::Ordering;

use crate::entry_point::Config;
use crate::entry_point::config_file::override_environment_variables_from_config;
use crate::rate_limit;

/// Global flag set by the SIGHUP signal handler.
///
/// The accept loop checks this between connections and calls [`reload`] when
/// it is `true`, then resets it to `false`.
pub static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Snapshot of all hot-reloadable configuration values at a point in time.
///
/// Obtain the current snapshot with [`current()`]. The snapshot is updated
/// atomically every time [`reload()`] completes — no partial reads are possible.
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    /// `RWS_CONFIG_CORS_ALLOW_ALL`
    pub cors_allow_all: bool,
    /// `RWS_CONFIG_CORS_ALLOW_ORIGINS`
    pub cors_allow_origins: String,
    /// `RWS_CONFIG_CORS_ALLOW_CREDENTIALS`
    pub cors_allow_credentials: String,
    /// `RWS_CONFIG_CORS_ALLOW_METHODS`
    pub cors_allow_methods: String,
    /// `RWS_CONFIG_CORS_ALLOW_HEADERS`
    pub cors_allow_headers: String,
    /// `RWS_CONFIG_CORS_EXPOSE_HEADERS`
    pub cors_expose_headers: String,
    /// `RWS_CONFIG_CORS_MAX_AGE`
    pub cors_max_age: String,
    /// `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS`
    pub rate_limit_max_requests: u32,
    /// `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS`
    pub rate_limit_window_secs: u64,
    /// `RWS_CONFIG_LOG_FORMAT`
    pub log_format: String,
    /// `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES`
    pub request_allocation_size: i64,
    /// `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`
    pub max_body_size: u64,
}

impl ConfigSnapshot {
    fn from_env() -> Self {
        let read = |key: &str| std::env::var(key).unwrap_or_default();
        Self {
            cors_allow_all: read(Config::RWS_CONFIG_CORS_ALLOW_ALL)
                .eq_ignore_ascii_case("true"),
            cors_allow_origins:     read(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS),
            cors_allow_credentials: read(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS),
            cors_allow_methods:     read(Config::RWS_CONFIG_CORS_ALLOW_METHODS),
            cors_allow_headers:     read(Config::RWS_CONFIG_CORS_ALLOW_HEADERS),
            cors_expose_headers:    read(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS),
            cors_max_age:           read(Config::RWS_CONFIG_CORS_MAX_AGE),
            rate_limit_max_requests: std::env::var("RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            rate_limit_window_secs: std::env::var("RWS_CONFIG_RATE_LIMIT_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            log_format: read(Config::RWS_CONFIG_LOG_FORMAT),
            request_allocation_size: std::env::var(
                Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES,
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(*Config::RWS_DEFAULT_REQUEST_ALLOCATION_SIZE_IN_BYTES),
            max_body_size: crate::entry_point::get_max_body_size(),
        }
    }
}

static SNAPSHOT: OnceLock<RwLock<ConfigSnapshot>> = OnceLock::new();

fn global() -> &'static RwLock<ConfigSnapshot> {
    SNAPSHOT.get_or_init(|| RwLock::new(ConfigSnapshot::from_env()))
}

/// Returns a clone of the current hot-reloadable configuration snapshot.
///
/// This takes a brief read lock and clones a handful of strings — safe to call
/// on every request if needed.
pub fn current() -> ConfigSnapshot {
    global().read().unwrap().clone()
}

/// Re-read `rws.config.toml` and apply all hot-reloadable changes in-place.
///
/// Called automatically when [`RELOAD_REQUESTED`] is set (SIGHUP on Unix) or
/// from the `POST /admin/config/reload` handler.
///
/// Settings that cannot be changed without a restart (IP, port, thread count,
/// TLS cert/key) are silently ignored — they are re-parsed from the file but
/// have no effect until the next process start.
pub fn reload() {
    // Re-parse rws.config.toml → updates process env vars for CORS, log format, etc.
    // Only one thread ever calls this (the signal handler or admin endpoint),
    // while workers only read env vars, so the single-writer / many-readers
    // pattern is safe in practice on all supported platforms.
    override_environment_variables_from_config(None);

    let snapshot = ConfigSnapshot::from_env();

    // Apply rate-limit changes to the live global limiter immediately.
    rate_limit::global().set_limits(
        snapshot.rate_limit_max_requests,
        snapshot.rate_limit_window_secs,
    );

    // Publish the new snapshot atomically.
    *global().write().unwrap() = snapshot.clone();

    println!(
        "Config reloaded — cors_allow_all={} rate_limit={}/{} log_format={}",
        snapshot.cors_allow_all,
        snapshot.rate_limit_max_requests,
        snapshot.rate_limit_window_secs,
        snapshot.log_format,
    );
}

/// Install a `SIGHUP` signal handler that sets [`RELOAD_REQUESTED`].
///
/// Call this once at server startup (before the accept loop). Safe to call
/// on non-Unix platforms — it compiles to a no-op.
///
/// The handler itself does the minimum allowed in a signal context: it stores
/// `true` into an `AtomicBool`. The actual [`reload()`] call happens on the
/// main thread between connection accepts.
pub fn install_sighup_handler() {
    #[cfg(all(unix, feature = "http1"))]
    // SAFETY: The handler only writes to a process-global AtomicBool which is
    // async-signal-safe. No allocation, no locks, no I/O.
    unsafe {
        libc::signal(libc::SIGHUP, sighup_handler as *const () as libc::sighandler_t);
    }
}

#[cfg(all(unix, feature = "http1"))]
extern "C" fn sighup_handler(_: libc::c_int) {
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
}
