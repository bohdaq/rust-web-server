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
//! | `DnsSrv`     | SRV record lookup — weight-expanded `host:port` list.     |
//! | `Consul`     | Consul HTTP API `/v1/health/service/:name`.                |
//! | `Docker`     | Docker Engine API — containers carrying a given label.    |
//! | `EtcdWatch`  | etcd v3 watch stream — incremental, low-latency updates.   |
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
// `pub(crate)` (not just `mod`) so `secrets`'s Vault/AWS-SM/Key-Vault backends
// can reuse this parser for their own small JSON responses, instead of a
// third hand-rolled JSON parser alongside this one and `mcp::json_rpc`.
// Pure parsing, no sockets — stays available on every target, unlike the
// rest of this module (DNS/etcd/Consul/Docker all need real network access
// no wasi:http guest has). See spec/WASM_SHIM.md.
pub(crate) mod json_lite;

#[cfg(not(target_arch = "wasm32"))]
mod consul;
#[cfg(not(target_arch = "wasm32"))]
mod dns_srv;
#[cfg(not(target_arch = "wasm32"))]
mod docker;
#[cfg(not(target_arch = "wasm32"))]
mod etcd;

#[cfg(not(target_arch = "wasm32"))]
use std::net::ToSocketAddrs;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

// ── DiscoverySource ───────────────────────────────────────────────────────────

/// Controls how [`BackendPool`] discovers backend addresses.
#[cfg(not(target_arch = "wasm32"))]
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
    /// Resolve `record` (e.g. `_http._tcp.example.com`) via SRV lookup. Only the
    /// lowest-priority tier of records is used; within that tier each `target:port`
    /// is repeated `weight.clamp(1, 20)` times in the returned list so a plain
    /// round-robin consumer still sees proportional selection frequency — see
    /// [`dns_srv`] module docs for the exact algorithm and its bound.
    DnsSrv { record: String },
    /// Query a Consul agent's `/v1/health/service/:name` endpoint. `addr` is
    /// `host:port` of the agent (e.g. `127.0.0.1:8500`); only instances passing
    /// all health checks are returned.
    Consul { addr: String, service: String },
    /// Query the Docker Engine API (over a Unix domain socket) for running
    /// containers carrying `label`, using each container's *value* for that
    /// label as the backend address directly (e.g. `rws.backend=10.0.0.5:8080`)
    /// — see [`docker`] module docs for why the label value, not a published
    /// port guess, is the address. Unix-only; a no-op elsewhere.
    Docker { label: String, socket_path: String },
    /// Watch an etcd v3 key prefix via its gRPC-gateway JSON/HTTP `/v3/watch`
    /// endpoint. Unlike every other source, this is *not* driven by the
    /// generic poll loop — [`BackendPool::start`] spawns a dedicated
    /// long-lived connection instead, applying `PUT`/`DELETE` events
    /// incrementally as they arrive rather than re-listing on each one.
    /// `resolve()` (and therefore plain `refresh()`) still performs a one-shot
    /// `/v3/kv/range` listing, so a pool built with this source is usable
    /// without ever calling `start()`, just without live updates. Plain HTTP
    /// only — no TLS support yet.
    EtcdWatch { endpoints: Vec<String>, prefix: String },
}

#[cfg(not(target_arch = "wasm32"))]
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

            DiscoverySource::DnsSrv { record } => dns_srv::resolve(record),

            DiscoverySource::Consul { addr, service } => consul::discover(addr, service),

            DiscoverySource::Docker { label, socket_path } => docker::discover(socket_path, label),

            DiscoverySource::EtcdWatch { endpoints, prefix } => {
                let Some(endpoint) = endpoints.first() else { return Vec::new() };
                match etcd::kv_range(endpoint, prefix) {
                    Ok(map) => map.into_values().collect(),
                    Err(e) => {
                        eprintln!("service_discovery: etcd range query failed: {}", e);
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
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct BackendPool {
    backends: Arc<RwLock<Vec<String>>>,
    source: Arc<DiscoverySource>,
    poll_interval_secs: u64,
}

#[cfg(not(target_arch = "wasm32"))]
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

    /// Create a pool whose backends are discovered via DNS SRV lookup (e.g.
    /// `_http._tcp.my-service.default.svc.cluster.local` for a Kubernetes
    /// headless Service). See [`DiscoverySource::DnsSrv`] for the
    /// priority/weight handling.
    pub fn dns_srv(record: impl Into<String>) -> Self {
        Self::new(DiscoverySource::DnsSrv { record: record.into() })
    }

    /// Create a pool whose backends are discovered via a Consul agent's
    /// `/v1/health/service/:name` endpoint. `addr` is `host:port` of the
    /// agent, e.g. `"127.0.0.1:8500"`. Only instances passing all health
    /// checks are returned.
    pub fn consul(addr: impl Into<String>, service: impl Into<String>) -> Self {
        Self::new(DiscoverySource::Consul { addr: addr.into(), service: service.into() })
    }

    /// Create a pool whose backends are discovered from running Docker
    /// containers carrying `label`, using the default socket path
    /// (`/var/run/docker.sock`). See [`DiscoverySource::Docker`] for the
    /// label-value-is-the-address convention. Unix-only; a no-op elsewhere.
    pub fn docker(label: impl Into<String>) -> Self {
        Self::new(DiscoverySource::Docker {
            label: label.into(),
            socket_path: "/var/run/docker.sock".to_string(),
        })
    }

    /// Same as [`BackendPool::docker`] but against a non-default Docker
    /// socket path.
    pub fn docker_with_socket(label: impl Into<String>, socket_path: impl Into<String>) -> Self {
        Self::new(DiscoverySource::Docker { label: label.into(), socket_path: socket_path.into() })
    }

    /// Create a pool whose backends are discovered from an etcd v3 key
    /// `prefix`, kept live via a watch stream once [`BackendPool::start`] is
    /// called. `endpoints` is a list of `host:port` etcd cluster members —
    /// only the first is used today (no client-side failover yet). Plain
    /// HTTP only. See [`DiscoverySource::EtcdWatch`].
    pub fn etcd(endpoints: Vec<String>, prefix: impl Into<String>) -> Self {
        Self::new(DiscoverySource::EtcdWatch { endpoints, prefix: prefix.into() })
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
    /// For `Static` sources this is a no-op. For all others, an immediate
    /// `refresh()` is performed before spawning the background thread so that
    /// the first `backends()` call returns a populated list.
    ///
    /// `EtcdWatch` is special-cased: instead of the generic sleep-and-poll
    /// loop, a dedicated thread holds a long-lived connection to etcd's watch
    /// stream and applies `PUT`/`DELETE` events to the backend list as they
    /// arrive — no polling interval involved after the initial `refresh()`.
    pub fn start(&self) {
        if matches!(self.source.as_ref(), DiscoverySource::Static(_)) {
            return;
        }
        self.refresh();

        if let DiscoverySource::EtcdWatch { endpoints, prefix } = self.source.as_ref() {
            let endpoints = endpoints.clone();
            let prefix = prefix.clone();
            let pool = self.clone();
            std::thread::spawn(move || etcd::watch_forever(&endpoints, &prefix, &pool));
            return;
        }

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
