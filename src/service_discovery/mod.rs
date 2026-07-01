//! Dynamic backend pool with pluggable discovery sources.
//!
//! [`BackendPool`] maintains a thread-safe list of `"host:port"` addresses that
//! can be refreshed on a background thread.  Discovery is delegated to a
//! [`DiscoverySource`]:
//!
//! | Variant      | Description                                               |
//! |------------- |---------------------------------------------------------- |
//! | `Static`     | Fixed list — no polling required.                         |
//! | `EnvPrefix`  | Scan `PREFIX_0`, `PREFIX_1`, … environment variables.     |
//! | `File`       | Read one `host:port` per line from a file.                |
//! | `Dns`        | A-record lookup — resolve hostname to all IPs.            |
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::service_discovery::BackendPool;
//!
//! // Fixed list — no background thread needed.
//! let pool = BackendPool::r#static(vec!["10.0.0.1:8080".into(), "10.0.0.2:8080".into()]);
//! println!("{:?}", pool.backends());
//!
//! // Env-var discovery, refreshed every 60 seconds.
//! let pool = BackendPool::env_prefix("MY_SVC_BACKEND")
//!     .poll_interval_secs(60);
//! pool.start();
//! println!("{:?}", pool.backends());
//! ```

#[cfg(test)]
mod tests;

use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// ── DiscoverySource ───────────────────────────────────────────────────────────

/// Controls how [`BackendPool`] discovers backend addresses.
pub enum DiscoverySource {
    /// Fixed list of `"host:port"` addresses — never refreshed.
    Static(Vec<String>),
    /// Scan environment variables `PREFIX_0`, `PREFIX_1`, … until one is absent.
    EnvPrefix(String),
    /// Read one `host:port` per line from a file.  Blank lines and lines starting
    /// with `#` are ignored.
    File(String),
    /// Resolve `hostname` via A-record DNS lookup; format each IP as `ip:port`.
    Dns { hostname: String, port: u16 },
}

impl DiscoverySource {
    /// Perform a single discovery cycle and return the current backend list.
    fn resolve(&self) -> Vec<String> {
        match self {
            DiscoverySource::Static(v) => v.clone(),

            DiscoverySource::EnvPrefix(prefix) => {
                let mut backends = Vec::new();
                let mut i = 0usize;
                loop {
                    let key = format!("{}_{}", prefix, i);
                    match std::env::var(&key) {
                        Ok(val) => { backends.push(val); i += 1; }
                        Err(_) => break,
                    }
                }
                backends
            }

            DiscoverySource::File(path) => {
                match std::fs::read_to_string(path) {
                    Ok(contents) => contents
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .map(str::to_string)
                        .collect(),
                    Err(e) => {
                        eprintln!("service_discovery: cannot read backend file {:?}: {}", path, e);
                        Vec::new()
                    }
                }
            }

            DiscoverySource::Dns { hostname, port } => {
                let addr_str = format!("{}:{}", hostname, port);
                match addr_str.to_socket_addrs() {
                    Ok(addrs) => addrs
                        .map(|sa| format!("{}:{}", sa.ip(), sa.port()))
                        .collect(),
                    Err(e) => {
                        eprintln!("service_discovery: DNS lookup for {} failed: {}", addr_str, e);
                        Vec::new()
                    }
                }
            }
        }
    }
}

// ── BackendPool ───────────────────────────────────────────────────────────────

/// Thread-safe pool of backend addresses, optionally refreshed in the background.
///
/// Clone this type freely — all clones share the same underlying `RwLock<Vec>`.
#[derive(Clone)]
pub struct BackendPool {
    backends: Arc<RwLock<Vec<String>>>,
    source: Arc<DiscoverySource>,
    poll_interval_secs: u64,
}

impl BackendPool {
    fn new(source: DiscoverySource) -> Self {
        Self {
            backends: Arc::new(RwLock::new(Vec::new())),
            source: Arc::new(source),
            poll_interval_secs: 30,
        }
    }

    /// Create a pool from a fixed list of backends.
    ///
    /// The list is available immediately; `start()` is a no-op.
    pub fn r#static(backends: Vec<String>) -> Self {
        let initial = backends.clone();
        let pool = Self::new(DiscoverySource::Static(backends));
        *pool.backends.write().unwrap() = initial;
        pool
    }

    /// Create a pool whose backends are read from environment variables
    /// `PREFIX_0`, `PREFIX_1`, … at startup and every `poll_interval_secs`.
    pub fn env_prefix(prefix: impl Into<String>) -> Self {
        Self::new(DiscoverySource::EnvPrefix(prefix.into()))
    }

    /// Create a pool whose backends are read from a file (one `host:port` per line).
    pub fn file(path: impl Into<String>) -> Self {
        Self::new(DiscoverySource::File(path.into()))
    }

    /// Create a pool whose backends are discovered via DNS A-record lookup.
    pub fn dns(hostname: impl Into<String>, port: u16) -> Self {
        Self::new(DiscoverySource::Dns { hostname: hostname.into(), port })
    }

    /// Override the background refresh interval (default: 30 seconds).
    ///
    /// Only meaningful for `File` and `Dns` sources.
    pub fn poll_interval_secs(mut self, secs: u64) -> Self {
        self.poll_interval_secs = secs;
        self
    }

    /// Start the background refresh thread.
    ///
    /// For `Static` sources this is a no-op.  For all others, an immediate
    /// `refresh()` is performed before spawning the background thread so that
    /// the first `backends()` call returns a populated list.
    pub fn start(&self) {
        if matches!(self.source.as_ref(), DiscoverySource::Static(_)) {
            return;
        }
        self.refresh();
        let pool = self.clone();
        let interval = Duration::from_secs(self.poll_interval_secs);
        std::thread::spawn(move || loop {
            std::thread::sleep(interval);
            pool.refresh();
        });
    }

    /// Return a snapshot of the current backend list.
    pub fn backends(&self) -> Vec<String> {
        self.backends.read().unwrap().clone()
    }

    /// Replace the current backend list with `backends`.
    ///
    /// Useful for testing or for external control planes that push updates.
    pub fn update(&self, backends: Vec<String>) {
        *self.backends.write().unwrap() = backends;
    }

    /// Perform one synchronous refresh cycle.
    ///
    /// Called automatically by `start()` and by the background thread.
    pub fn refresh(&self) {
        let resolved = self.source.resolve();
        *self.backends.write().unwrap() = resolved;
    }
}
