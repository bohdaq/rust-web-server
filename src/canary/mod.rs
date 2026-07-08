//! Weighted canary / A-B traffic splitting middleware.
//!
//! [`CanaryLayer`] implements [`Middleware`] and distributes incoming requests
//! across a set of backends according to configurable weights, using the same
//! *smooth* weighted round-robin (SWRR) algorithm nginx uses: each backend has
//! a `current_weight` that accumulates its configured weight every selection
//! and is decremented by the total weight whenever it's picked. That spreads
//! high-weight backends evenly through the sequence instead of bursting them
//! consecutively — weights `5, 1, 1` select roughly `A A B A C A A` (repeating),
//! never five `A`s in a row, unlike a flat pre-expanded rotation list would.
//!
//! Backends are contacted over plain HTTP/1.1, or over TLS when the backend
//! URL uses an `https://`, `h2s://`, or `grpcs://` scheme (requires the
//! `http-client` or `http2` feature — both pull in `rustls`). If a backend is
//! unavailable the next one in the fallback order is tried; after exhausting
//! all backends the middleware returns `502 Bad Gateway`.
//!
//! A group's members can also come from a [`crate::service_discovery::BackendPool`]
//! instead of a fixed URL — see [`WeightedPool`] / [`CanaryLayer::add_pool`] —
//! so e.g. "10% of traffic to the canary group" keeps working as pods in that
//! group come and go, without touching the weight itself.
//!
//! Weights can be replaced at runtime with [`CanaryLayer::update`] — clone the
//! layer *before* wrapping it to keep a handle for later:
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::canary::{CanaryLayer, WeightedBackend};
//! use rust_web_server::middleware::WithMiddleware;
//!
//! // 75 % of traffic → stable, 25 % → canary
//! let layer = CanaryLayer::new(vec![
//!     WeightedBackend::new("http://stable:8080", 3),
//!     WeightedBackend::new("http://canary:8080", 1),
//! ])
//! .path_prefix("/api");
//!
//! let handle = layer.clone(); // keep this — `layer` moves into `.wrap(...)`
//! let app = WithMiddleware::new(App::new()).wrap(layer);
//!
//! // Later — e.g. from an admin endpoint or a rollout script — shift more
//! // traffic to canary without restarting the process:
//! handle.update(
//!     vec![
//!         WeightedBackend::new("http://stable:8080", 1),
//!         WeightedBackend::new("http://canary:8080", 1),
//!     ],
//!     vec![],
//! );
//! ```

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::application::Application;
use crate::core::New;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::service_discovery::BackendPool;

// ── WeightedBackend / WeightedPool ────────────────────────────────────────────

/// A single fixed backend URL together with a relative traffic weight.
///
/// A weight of 0 causes the backend to be skipped entirely.
#[derive(Clone)]
pub struct WeightedBackend {
    pub url: String,
    pub weight: u32,
}

impl WeightedBackend {
    /// Create a new weighted backend.
    pub fn new(url: impl Into<String>, weight: u32) -> Self {
        Self { url: url.into(), weight }
    }
}

/// A dynamically-discovered group of backends (a [`BackendPool`]) together
/// with a relative traffic weight for the *group as a whole*. Which specific
/// pool member answers a given request is a plain round-robin over
/// `pool.backends()` at the time of selection — the weight only controls how
/// often this group is picked relative to other groups/backends, not which
/// member within it.
///
/// A weight of 0 causes the group to be skipped entirely. Always treated as
/// a plain HTTP/1.1 backend — `BackendPool` addresses are bare `"host:port"`
/// strings with no scheme to carry TLS intent.
#[derive(Clone)]
pub struct WeightedPool {
    pub pool: BackendPool,
    pub weight: u32,
}

impl WeightedPool {
    /// Create a new weighted pool.
    pub fn new(pool: BackendPool, weight: u32) -> Self {
        Self { pool, weight }
    }
}

// ── internal state ────────────────────────────────────────────────────────────

enum Target {
    /// A fixed `(host, port, tls)`, parsed once from a [`WeightedBackend`] URL.
    Static(String, u16, bool),
    /// A live-discovered group; round-robins its own current members.
    Pool(BackendPool, AtomicUsize),
}

struct SwrrEntry {
    target: Target,
    /// Configured weight — fixed until the next [`CanaryLayer::update`] or
    /// [`CanaryLayer::add_pool`] call.
    weight: i64,
    /// Evolves every [`CanaryState::tick`] call; this is the actual SWRR state.
    current_weight: i64,
}

struct CanaryState {
    entries: Vec<SwrrEntry>,
    total_weight: i64,
}

impl CanaryState {
    fn new() -> Self {
        Self { entries: Vec::new(), total_weight: 0 }
    }

    fn push_static(&mut self, host: String, port: u16, tls: bool, weight: u32) {
        if weight == 0 {
            return;
        }
        self.total_weight += weight as i64;
        self.entries.push(SwrrEntry {
            target: Target::Static(host, port, tls),
            weight: weight as i64,
            current_weight: 0,
        });
    }

    fn push_pool(&mut self, pool: BackendPool, weight: u32) {
        if weight == 0 {
            return;
        }
        self.total_weight += weight as i64;
        self.entries.push(SwrrEntry {
            target: Target::Pool(pool, AtomicUsize::new(0)),
            weight: weight as i64,
            current_weight: 0,
        });
    }

    /// One smooth-weighted-round-robin step (the nginx algorithm): every
    /// entry's `current_weight` accumulates its configured weight, the
    /// maximum is picked, then that entry's `current_weight` is reduced by
    /// the total weight. Returns entry indices in fallback-try order — the
    /// primary pick first, then the rest ranked by (post-tick)
    /// `current_weight` descending.
    ///
    /// Ranking the *rest* is a read-only sort with no further mutation, so
    /// trying several backends as failover for one request doesn't perturb
    /// the sequence subsequent requests see — only one entry's state changes
    /// per `tick()` call, exactly as if a single backend had been chosen.
    fn tick(&mut self) -> Vec<usize> {
        if self.entries.is_empty() {
            return Vec::new();
        }
        for e in &mut self.entries {
            e.current_weight += e.weight;
        }
        let best = self
            .entries
            .iter()
            .enumerate()
            .max_by_key(|(_, e)| e.current_weight)
            .map(|(i, _)| i)
            .unwrap();
        self.entries[best].current_weight -= self.total_weight;

        let weights: Vec<i64> = self.entries.iter().map(|e| e.current_weight).collect();
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by(|&a, &b| {
            if a == best {
                return std::cmp::Ordering::Less;
            }
            if b == best {
                return std::cmp::Ordering::Greater;
            }
            weights[b].cmp(&weights[a])
        });
        order
    }
}

// ── CanaryLayer ───────────────────────────────────────────────────────────────

/// Weighted traffic-splitting proxy middleware.
///
/// `Clone` is cheap and shares state — all clones distribute traffic against
/// the same underlying [`CanaryState`], so a [`CanaryLayer::update`] on one
/// clone is immediately visible to a clone already wrapped into a running
/// `Application` (see the module docs for the clone-before-wrapping pattern).
#[derive(Clone)]
pub struct CanaryLayer {
    state: Arc<Mutex<CanaryState>>,
    connect_timeout: Duration,
    read_timeout: Duration,
    path_prefix: Option<String>,
}

impl CanaryLayer {
    /// Build a `CanaryLayer` from the given weighted backends.
    ///
    /// Backends with `weight == 0` are ignored. Equivalent to
    /// `CanaryLayer::from_parts(backends, vec![])`.
    pub fn new(backends: Vec<WeightedBackend>) -> Self {
        Self::from_parts(backends, Vec::new())
    }

    /// Build a `CanaryLayer` whose groups are all dynamically-discovered
    /// [`BackendPool`]s rather than fixed URLs. Equivalent to
    /// `CanaryLayer::from_parts(vec![], pools)`.
    pub fn with_pools(pools: Vec<WeightedPool>) -> Self {
        Self::from_parts(Vec::new(), pools)
    }

    /// Build a `CanaryLayer` from a mix of fixed backends and dynamic pools.
    pub fn from_parts(backends: Vec<WeightedBackend>, pools: Vec<WeightedPool>) -> Self {
        let mut state = CanaryState::new();
        for wb in backends {
            if let Some((host, port, tls)) = parse_backend_url(&wb.url) {
                state.push_static(host, port, tls, wb.weight);
            }
        }
        for wp in pools {
            state.push_pool(wp.pool, wp.weight);
        }
        Self {
            state: Arc::new(Mutex::new(state)),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            path_prefix: None,
        }
    }

    /// Append one more dynamically-discovered group to an already-built
    /// layer (e.g. adding a canary `BackendPool` alongside stable backends
    /// passed to [`CanaryLayer::new`]). A weight of 0 is a no-op.
    ///
    /// This is a builder method for use before `.wrap(...)` — for changing
    /// an already-wrapped, already-running layer's configuration, use
    /// [`CanaryLayer::update`] instead.
    pub fn add_pool(self, pool: BackendPool, weight: u32) -> Self {
        self.state.lock().unwrap().push_pool(pool, weight);
        self
    }

    /// Replace the entire backend/pool configuration in place — the runtime
    /// equivalent of building a fresh layer with
    /// [`CanaryLayer::from_parts`], except every existing clone of this
    /// layer (including one already wrapped into a running `Application`)
    /// picks up the change on its very next request. No restart required.
    pub fn update(&self, backends: Vec<WeightedBackend>, pools: Vec<WeightedPool>) {
        let mut new_state = CanaryState::new();
        for wb in backends {
            if let Some((host, port, tls)) = parse_backend_url(&wb.url) {
                new_state.push_static(host, port, tls, wb.weight);
            }
        }
        for wp in pools {
            new_state.push_pool(wp.pool, wp.weight);
        }
        *self.state.lock().unwrap() = new_state;
    }

    /// Only proxy requests whose URI starts with `prefix`; pass others through.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(prefix.into());
        self
    }

    /// Override the TCP connect timeout (default: 5 000 ms).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.connect_timeout = Duration::from_millis(ms);
        self
    }

    /// Override the response read timeout (default: 30 000 ms).
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.read_timeout = Duration::from_millis(ms);
        self
    }

    /// Runs one SWRR tick and resolves the resulting fallback order into
    /// actual dial targets — a `Pool` entry expands into *all* of its
    /// current members (starting from its own round-robin cursor), so a
    /// request exhausts every live member of its chosen group before
    /// falling through to the next group in the order. Pure/no I/O — safe
    /// to unit test directly.
    fn next_candidates(&self) -> Vec<(String, u16, bool)> {
        let mut state = self.state.lock().unwrap();
        let order = state.tick();
        let mut out = Vec::new();
        for idx in order {
            match &state.entries[idx].target {
                Target::Static(host, port, tls) => out.push((host.clone(), *port, *tls)),
                Target::Pool(pool, cursor) => {
                    let members = pool.backends();
                    if members.is_empty() {
                        continue;
                    }
                    let n = members.len();
                    let start = cursor.fetch_add(1, Ordering::Relaxed);
                    for i in 0..n {
                        if let Some((host, port)) = split_host_port(&members[(start + i) % n]) {
                            out.push((host, port, false));
                        }
                    }
                }
            }
        }
        out
    }

    /// Try candidates in fallback order until one succeeds.
    fn proxy(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let candidates = self.next_candidates();
        if candidates.is_empty() {
            return Err("CanaryLayer: no backends in rotation".to_string());
        }
        for (host, port, tls) in &candidates {
            let result = if *tls {
                #[cfg(any(feature = "http-client", feature = "http2"))]
                {
                    crate::proxy::proxy_https1(
                        request,
                        &connection.client.ip,
                        host,
                        *port,
                        self.connect_timeout,
                        self.read_timeout,
                    )
                }
                #[cfg(not(any(feature = "http-client", feature = "http2")))]
                {
                    Err("CanaryLayer: TLS backend requires the http-client or http2 feature".to_string())
                }
            } else {
                crate::proxy::proxy_http1(
                    request,
                    &connection.client.ip,
                    host,
                    *port,
                    self.connect_timeout,
                    self.read_timeout,
                )
            };
            if let Ok(resp) = result {
                return Ok(resp);
            }
        }
        Err("CanaryLayer: all backends failed".to_string())
    }
}

impl Middleware for CanaryLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if let Some(prefix) = &self.path_prefix {
            if !request.request_uri.starts_with(prefix.as_str()) {
                return next.execute(request, connection);
            }
        }
        match self.proxy(request, connection) {
            Ok(resp) => Ok(resp),
            Err(_) => Ok(bad_gateway()),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse a backend URL of the form `[scheme://]host[:port][/path]` into
/// `(host, port, tls)`.
///
/// `https://`, `h2s://`, and `grpcs://` set `tls = true` and default to port
/// 443; `http://`, `h2://`, `grpc://`, and a bare `host[:port]` set
/// `tls = false` and default to port 80 — matching `proxy::Backend::parse`'s
/// scheme conventions.
fn parse_backend_url(url: &str) -> Option<(String, u16, bool)> {
    let (rest, tls, default_port) = if let Some(r) = url.strip_prefix("https://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("h2s://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("grpcs://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("http://") {
        (r, false, 80u16)
    } else if let Some(r) = url.strip_prefix("h2://") {
        (r, false, 80u16)
    } else if let Some(r) = url.strip_prefix("grpc://") {
        (r, false, 80u16)
    } else {
        (url, false, 80u16)
    };
    // Drop any path component
    let host_port = rest.split('/').next().unwrap_or(rest);
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (host_port[..colon].to_string(), p)
        } else {
            (host_port.to_string(), default_port)
        }
    } else {
        (host_port.to_string(), default_port)
    };
    if host.is_empty() { None } else { Some((host, port, tls)) }
}

/// Splits a [`BackendPool`]-style `"host:port"` address. `BackendPool`
/// addresses have no scheme, so a pool-sourced target is always plain HTTP.
fn split_host_port(addr: &str) -> Option<(String, u16)> {
    let colon = addr.rfind(':')?;
    let port: u16 = addr[colon + 1..].parse().ok()?;
    let host = addr[..colon].to_string();
    if host.is_empty() { None } else { Some((host, port)) }
}

fn bad_gateway() -> Response {
    let cr = Range::get_content_range(
        b"502 Bad Gateway".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}
