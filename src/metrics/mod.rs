//! Server-wide and per-route Prometheus metrics.
//!
//! **Server-wide counters** (`REQUESTS_TOTAL`, `ERRORS_TOTAL`,
//! `ACTIVE_CONNECTIONS`) are updated by the server core automatically.
//!
//! **Per-route metrics** are opt-in: wrap your application with
//! [`MetricsLayer`] and each request will be attributed to its
//! `(method, path)` pair, emitting:
//! - `rws_route_requests_total{method,path,status}` — request counts
//! - `rws_route_duration_seconds{method,path}` — latency histogram
//!
//! **Circuit breaker state** — `rws_circuit_breaker_state{backend}` (gauge,
//! `0`=closed, `1`=half_open, `2`=open) is emitted automatically for every
//! backend known to [`crate::circuit_breaker::global`], with no opt-in layer
//! needed — see [`crate::circuit_breaker`] and
//! [`crate::proxy::ReverseProxy::with_circuit_breaker`].
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::metrics::MetricsLayer;
//!
//! let app = App::new().wrap(MetricsLayer);
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

// ── server-wide atomics ───────────────────────────────────────────────────────

/// Set to `true` after [`crate::server::Server::setup`] completes.
/// The `/readyz` controller returns `503` until this is `true`.
/// Set back to `false` when a shutdown signal is received so that
/// Kubernetes stops routing traffic before the pod exits.
pub static SERVER_READY: AtomicBool = AtomicBool::new(false);

/// Total HTTP requests handled across all connections and protocols.
pub static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Requests that caused an application-level error (app.execute returned Err).
pub static ERRORS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Number of currently open TCP/QUIC connections.
pub static ACTIVE_CONNECTIONS: AtomicI64 = AtomicI64::new(0);

/// Jobs queued in the thread pool waiting for a free worker.
pub static THREAD_POOL_QUEUED: AtomicI64 = AtomicI64::new(0);

pub fn record_request() {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_error() {
    ERRORS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn connection_open() {
    ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
}

pub fn connection_close() {
    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
}

// ── per-route store ───────────────────────────────────────────────────────────

/// Histogram bucket upper bounds (seconds). Matches the default Prometheus
/// buckets used by `prometheus_client` and most Go instrumentation libraries.
const BUCKET_BOUNDS: [f64; 11] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

struct HistogramEntry {
    /// `buckets[i]` = cumulative count of observations where duration ≤ `BUCKET_BOUNDS[i]`.
    buckets: [u64; 11],
    sum: f64,
    count: u64,
}

impl HistogramEntry {
    fn new() -> Self {
        HistogramEntry { buckets: [0; 11], sum: 0.0, count: 0 }
    }

    fn observe(&mut self, secs: f64) {
        for (i, &upper) in BUCKET_BOUNDS.iter().enumerate() {
            if secs <= upper {
                self.buckets[i] += 1;
            }
        }
        self.sum += secs;
        self.count += 1;
    }
}

struct RouteEntry {
    counts: HashMap<i16, u64>,
    latency: HistogramEntry,
}

impl RouteEntry {
    fn new() -> Self {
        RouteEntry { counts: HashMap::new(), latency: HistogramEntry::new() }
    }
}

struct RouteStore {
    /// Key: `(method, path)` — path has query string stripped.
    entries: HashMap<(String, String), RouteEntry>,
}

static ROUTE_STORE: OnceLock<Mutex<RouteStore>> = OnceLock::new();

fn route_store() -> &'static Mutex<RouteStore> {
    ROUTE_STORE.get_or_init(|| Mutex::new(RouteStore { entries: HashMap::new() }))
}

/// Record a completed request in the per-route store.
///
/// `path` must have the query string already stripped. Called automatically by
/// [`MetricsLayer`]; exposed publicly for testing and custom instrumentation.
pub fn record_route(method: &str, path: &str, status: i16, elapsed_secs: f64) {
    let key = (method.to_string(), path.to_string());
    let mut guard = route_store().lock().unwrap();
    let entry = guard.entries.entry(key).or_insert_with(RouteEntry::new);
    *entry.counts.entry(status).or_insert(0) += 1;
    entry.latency.observe(elapsed_secs);
}

/// Strip query string from a URI so `/users?page=2` → `/users`.
fn strip_query(uri: &str) -> &str {
    match uri.find('?') {
        Some(i) => &uri[..i],
        None => uri,
    }
}

// ── MetricsLayer middleware ───────────────────────────────────────────────────

/// Middleware that records per-route request counts and latency histograms.
///
/// Wrap any application with this layer once at startup; the data is collected
/// into a global store and emitted via `GET /metrics`.
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::metrics::MetricsLayer;
///
/// let app = App::new().wrap(MetricsLayer);
/// ```
pub struct MetricsLayer;

impl Middleware for MetricsLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let start = Instant::now();
        let result = next.execute(request, connection);
        let elapsed = start.elapsed().as_secs_f64();

        let path = strip_query(&request.request_uri).to_string();
        let status = match &result {
            Ok(r) => r.status_code,
            Err(_) => 500,
        };
        record_route(&request.method, &path, status, elapsed);

        result
    }
}

// ── prometheus output ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;

/// Returns a Prometheus text-format snapshot of all server-wide and per-route metrics.
pub fn prometheus_text() -> String {
    let requests = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let errors   = ERRORS_TOTAL.load(Ordering::Relaxed);
    let active   = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);

    let mut out = format!(
        "# HELP rws_requests_total Total HTTP requests handled\n\
         # TYPE rws_requests_total counter\n\
         rws_requests_total {}\n\n\
         # HELP rws_errors_total HTTP requests that returned an application error\n\
         # TYPE rws_errors_total counter\n\
         rws_errors_total {}\n\n\
         # HELP rws_active_connections Currently open connections\n\
         # TYPE rws_active_connections gauge\n\
         rws_active_connections {}\n",
        requests, errors, active
    );

    let route_text = route_prometheus_text();
    if !route_text.is_empty() {
        out.push('\n');
        out.push_str(&route_text);
    }

    let breaker_text = circuit_breaker_prometheus_text();
    if !breaker_text.is_empty() {
        out.push('\n');
        out.push_str(&breaker_text);
    }

    out
}

/// `rws_circuit_breaker_state{backend}` — the state of every backend known to
/// the process-wide [`crate::circuit_breaker::global`] breaker, encoded as
/// `0` (Closed/healthy), `1` (HalfOpen/probing), or `2` (Open/unhealthy).
///
/// Only the in-memory global breaker is covered — `RedisCircuitBreaker`
/// state can't be enumerated here (no `SCAN`/`KEYS` support in the minimal
/// hand-rolled RESP client; see [`crate::circuit_breaker::CircuitBreaker::all_states`]
/// docs). A backend never seen by the global breaker (e.g. one whose
/// `ReverseProxy` was wired to a different, non-global breaker instance)
/// emits no line rather than a synthetic `0` — there's nothing to report.
// `circuit_breaker` is native-only (backed by `redis_protocol`'s `TcpStream`)
// — see spec/WASM_SHIM.md. No breaker state exists to report under wasm32.
#[cfg(target_arch = "wasm32")]
fn circuit_breaker_prometheus_text() -> String {
    String::new()
}

#[cfg(not(target_arch = "wasm32"))]
fn circuit_breaker_prometheus_text() -> String {
    let mut states = crate::circuit_breaker::global().lock().unwrap().all_states();
    if states.is_empty() {
        return String::new();
    }
    states.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    out.push_str("# HELP rws_circuit_breaker_state Circuit breaker state per backend (0=closed, 1=half_open, 2=open)\n");
    out.push_str("# TYPE rws_circuit_breaker_state gauge\n");
    for (backend, state) in states {
        let value = match state {
            crate::circuit_breaker::BreakerState::Closed => 0,
            crate::circuit_breaker::BreakerState::HalfOpen => 1,
            crate::circuit_breaker::BreakerState::Open => 2,
        };
        out.push_str(&format!("rws_circuit_breaker_state{{backend=\"{}\"}} {}\n", backend, value));
    }
    out
}

fn route_prometheus_text() -> String {
    let guard = route_store().lock().unwrap();
    if guard.entries.is_empty() {
        return String::new();
    }

    // Sort for deterministic output.
    let mut keys: Vec<&(String, String)> = guard.entries.keys().collect();
    keys.sort();

    let mut out = String::new();

    // ── rws_route_requests_total ──────────────────────────────────────────────
    out.push_str("# HELP rws_route_requests_total Total requests handled per route\n");
    out.push_str("# TYPE rws_route_requests_total counter\n");
    for key in &keys {
        let entry = &guard.entries[key];
        let (method, path) = key;
        let mut statuses: Vec<i16> = entry.counts.keys().cloned().collect();
        statuses.sort();
        for status in statuses {
            let count = entry.counts[&status];
            out.push_str(&format!(
                "rws_route_requests_total{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}\n",
                method, path, status, count,
            ));
        }
    }

    // ── rws_route_duration_seconds histogram ──────────────────────────────────
    out.push('\n');
    out.push_str("# HELP rws_route_duration_seconds Request duration in seconds per route\n");
    out.push_str("# TYPE rws_route_duration_seconds histogram\n");
    for key in &keys {
        let entry = &guard.entries[key];
        let (method, path) = key;
        let lat = &entry.latency;

        for (i, &upper) in BUCKET_BOUNDS.iter().enumerate() {
            out.push_str(&format!(
                "rws_route_duration_seconds_bucket{{method=\"{}\",path=\"{}\",le=\"{}\"}} {}\n",
                method, path, upper, lat.buckets[i],
            ));
        }
        out.push_str(&format!(
            "rws_route_duration_seconds_bucket{{method=\"{}\",path=\"{}\",le=\"+Inf\"}} {}\n",
            method, path, lat.count,
        ));
        out.push_str(&format!(
            "rws_route_duration_seconds_sum{{method=\"{}\",path=\"{}\"}} {:.9}\n",
            method, path, lat.sum,
        ));
        out.push_str(&format!(
            "rws_route_duration_seconds_count{{method=\"{}\",path=\"{}\"}} {}\n",
            method, path, lat.count,
        ));
    }

    out
}
