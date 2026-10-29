//! What an `[[upstream]]` adds to choosing a backend in config mode
//! (since 18.6.0): `weights`, cookie stickiness (`[upstream.sticky]`), a
//! per-backend circuit breaker — passive health checking —
//! (`[upstream.circuit_breaker]`), retries on another backend
//! (`[upstream.retry]`), and the per-upstream metrics on `/metrics`.
//!
//! One [`UpstreamPolicy`] per declared upstream, shared by every route
//! that proxies to it, rebuilt on reload.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

use crate::circuit_breaker::{BreakerState, CircuitBreaker};

/// `[upstream.sticky]`: keep a client on one backend with a cookie.
#[derive(Debug, Clone, PartialEq)]
pub struct StickyConfig {
    /// The cookie's name.
    pub cookie: String,
    /// `Max-Age` in seconds; `None` = a session cookie.
    pub max_age_secs: Option<u64>,
}

/// `[upstream.circuit_breaker]`: take a failing backend out of rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakerConfig {
    /// Consecutive failures (connection errors, 502/503/504) that open it.
    pub failure_threshold: u32,
    /// How long an open backend is left alone before a probe request.
    pub recovery_secs: u64,
    /// Probe requests allowed at once while half-open.
    pub half_open_probes: u32,
}

/// `[upstream.retry]`: try another backend.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryConfig {
    /// Further backends tried after the first (so up to `attempts + 1`).
    pub attempts: u32,
    /// Statuses retried besides connection errors (default 502, 503, 504).
    pub status: Vec<i16>,
}

/// One upstream's selection state.
pub(crate) struct UpstreamPolicy {
    pub(crate) name: String,
    /// The health-checked backend list (for metrics).
    live: Arc<RwLock<Vec<String>>>,
    weights: HashMap<String, u32>,
    /// The configured backends in file order (for listing).
    order: Vec<String>,
    pub(crate) sticky: Option<StickyConfig>,
    breaker: Option<Mutex<CircuitBreaker>>,
    pub(crate) retry: Option<RetryConfig>,
    /// Smooth weighted round-robin's current weights.
    current: Mutex<HashMap<String, i64>>,
}

impl UpstreamPolicy {
    pub(crate) fn new(
        name: &str,
        live: Arc<RwLock<Vec<String>>>,
        backends: &[String],
        weights: &[u32],
        sticky: Option<StickyConfig>,
        breaker: Option<&BreakerConfig>,
        retry: Option<RetryConfig>,
    ) -> Self {
        UpstreamPolicy {
            name: name.to_string(),
            live,
            // Every backend, weight 1 unless given (also the gauges' list).
            weights: backends.iter().enumerate().map(|(i, b)| (b.clone(), weights.get(i).copied().unwrap_or(1))).collect(),
            order: backends.to_vec(),
            sticky,
            breaker: breaker.map(|b| Mutex::new(CircuitBreaker::new(b.failure_threshold.max(1), b.recovery_secs).max_half_open_probes(b.half_open_probes))),
            retry,
            current: Mutex::new(HashMap::new()),
        }
    }

    /// A backend's weight (1 unless configured).
    pub(crate) fn weight(&self, backend: &str) -> u32 {
        self.weights.get(backend).copied().unwrap_or(1).max(1)
    }

    /// Whether any backend has a weight other than 1.
    pub(crate) fn weighted(&self) -> bool {
        self.weights.values().any(|w| *w != 1)
    }

    /// Smooth weighted round-robin (nginx's): each pick raises every
    /// candidate by its weight and lowers the chosen one by the total, so
    /// weights 3:1 give a, a, b, a, ... — evenly interleaved.
    pub(crate) fn weighted_round_robin(&self, candidates: &[String]) -> Option<String> {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let total: i64 = candidates.iter().map(|b| self.weight(b) as i64).sum();
        let mut best: Option<(&String, i64)> = None;
        for backend in candidates {
            let value = current.entry(backend.clone()).or_insert(0);
            *value += self.weight(backend) as i64;
            if best.is_none_or(|(_, top)| *value > top) {
                best = Some((backend, *value));
            }
        }
        let (chosen, _) = best?;
        *current.get_mut(chosen.as_str()).expect("just inserted") -= total;
        Some(chosen.clone())
    }

    /// Weighted pick for `random`: `roll` is any number.
    pub(crate) fn weighted_index(&self, candidates: &[String], roll: usize) -> usize {
        let total: usize = candidates.iter().map(|b| self.weight(b) as usize).sum();
        let mut point = roll % total.max(1);
        for (i, backend) in candidates.iter().enumerate() {
            let weight = self.weight(backend) as usize;
            if point < weight {
                return i;
            }
            point -= weight;
        }
        0
    }

    /// Whether the breaker lets a request through to `backend` now (a
    /// half-open probe counts as one).
    pub(crate) fn admits(&self, backend: &str) -> bool {
        match &self.breaker {
            Some(breaker) => breaker.lock().unwrap_or_else(|e| e.into_inner()).is_available(backend),
            None => true,
        }
    }

    /// Whether the breaker holds any of `backends` open.
    pub(crate) fn any_open(&self, backends: &[String]) -> bool {
        self.breaker.as_ref().is_some_and(|breaker| {
            let breaker = breaker.lock().unwrap_or_else(|e| e.into_inner());
            backends.iter().any(|b| breaker.state(b) != BreakerState::Closed)
        })
    }

    pub(crate) fn record(&self, backend: &str, failed: bool) {
        if let Some(breaker) = &self.breaker {
            let mut breaker = breaker.lock().unwrap_or_else(|e| e.into_inner());
            if failed {
                breaker.record_failure(backend);
            } else {
                breaker.record_success(backend);
            }
        }
    }

    fn breaker_state(&self, backend: &str) -> Option<BreakerState> {
        self.breaker.as_ref().map(|b| b.lock().unwrap_or_else(|e| e.into_inner()).state(backend))
    }

    /// Whether `status` from a backend should be retried elsewhere.
    pub(crate) fn retries_status(&self, status: i16) -> bool {
        self.retry.as_ref().is_some_and(|r| r.status.contains(&status))
    }
}

/// A backend's id in the sticky cookie: 12 hex digits of its URL's SHA-256
/// — stable across restarts and replicas, without revealing the address.
pub(crate) fn backend_id(backend: &str) -> String {
    crate::core::sha256::sha256(backend.as_bytes())[..6].iter().map(|b| format!("{b:02x}")).collect()
}

/// The value of cookie `name` in the request's `Cookie` headers.
pub(crate) fn cookie_value(request: &crate::request::Request, name: &str) -> Option<String> {
    request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("cookie"))
        .flat_map(|h| h.value.split(';').map(str::trim).map(str::to_string).collect::<Vec<_>>())
        .find_map(|pair| pair.split_once('=').filter(|(n, _)| *n == name).map(|(_, v)| v.to_string()))
}

/// Whether a request may be sent again to another backend: an idempotent
/// method and a body still in memory.
pub(crate) fn replayable(request: &crate::request::Request) -> bool {
    matches!(request.method.as_str(), "GET" | "HEAD" | "OPTIONS" | "PUT" | "DELETE" | "TRACE") && !request.body.is_stream()
}

// ── metrics ───────────────────────────────────────────────────────────────────

const BUCKETS: [f64; 11] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

#[derive(Default)]
struct BackendStats {
    /// By status code, `"error"` for a request that got no response.
    requests: HashMap<String, u64>,
    buckets: [u64; 11],
    sum: f64,
    count: u64,
}

#[derive(Default)]
struct Stats {
    /// `(upstream, backend)` → stats.
    backends: HashMap<(String, String), BackendStats>,
    retries: HashMap<String, u64>,
    /// The current configuration's upstreams, for gauges.
    policies: Vec<Arc<UpstreamPolicy>>,
}

fn stats() -> &'static Mutex<Stats> {
    static STATS: OnceLock<Mutex<Stats>> = OnceLock::new();
    STATS.get_or_init(Default::default)
}

/// Make `policies` the ones `/metrics` reports gauges for (each compile).
pub(crate) fn register(policies: Vec<Arc<UpstreamPolicy>>) {
    stats().lock().unwrap_or_else(|e| e.into_inner()).policies = policies;
}

// ── drain ─────────────────────────────────────────────────────────────────────

/// Any backend drained: the per-request check is one load when none is.
static ANY_DRAINED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn drained_set() -> &'static RwLock<std::collections::HashSet<(String, String)>> {
    static SET: OnceLock<RwLock<std::collections::HashSet<(String, String)>>> = OnceLock::new();
    SET.get_or_init(RwLock::default)
}

/// Take `backend` of `upstream` out of rotation (`true`) or put it back.
/// New requests skip a drained backend; requests already on it finish; its
/// health checks go on. Kept across reloads (by name), not across restarts.
pub(crate) fn set_drained(upstream: &str, backend: &str, drained: bool) {
    let mut set = drained_set().write().unwrap_or_else(|e| e.into_inner());
    let key = (upstream.to_string(), backend.to_string());
    if drained {
        set.insert(key);
    } else {
        set.remove(&key);
    }
    ANY_DRAINED.store(!set.is_empty(), std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn is_drained(upstream: &str, backend: &str) -> bool {
    if !ANY_DRAINED.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    let set = drained_set().read().unwrap_or_else(|e| e.into_inner());
    set.iter().any(|(u, b)| u == upstream && b == backend)
}

/// One backend of an upstream as the admin API shows it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BackendView {
    pub(crate) url: String,
    pub(crate) weight: u32,
    /// In the health-checked live list.
    pub(crate) healthy: bool,
    /// `closed`, `open`, `half_open`; `None` without a circuit breaker.
    pub(crate) breaker: Option<&'static str>,
    pub(crate) drained: bool,
}

/// The current configuration's upstreams and their backends, in order.
pub(crate) fn snapshot() -> Vec<(String, Vec<BackendView>)> {
    let policies = stats().lock().unwrap_or_else(|e| e.into_inner()).policies.clone();
    policies
        .iter()
        .map(|p| {
            let live = p.live.read().unwrap_or_else(|e| e.into_inner()).clone();
            let mut backends: Vec<&String> = p.weights.keys().collect();
            backends.sort_by_key(|b| p.order.iter().position(|o| o == *b).unwrap_or(usize::MAX));
            let views = backends
                .into_iter()
                .map(|b| BackendView {
                    url: b.clone(),
                    weight: p.weight(b),
                    healthy: live.contains(b),
                    breaker: p.breaker_state(b).map(|s| match s {
                        BreakerState::Closed => "closed",
                        BreakerState::Open => "open",
                        BreakerState::HalfOpen => "half_open",
                    }),
                    drained: is_drained(&p.name, b),
                })
                .collect();
            (p.name.clone(), views)
        })
        .collect()
}

/// One request to `backend` of `upstream`: its status (`None`: no
/// response) and how long it took.
pub(crate) fn observe(upstream: &str, backend: &str, status: Option<i16>, started: Instant) {
    let secs = started.elapsed().as_secs_f64();
    let mut stats = stats().lock().unwrap_or_else(|e| e.into_inner());
    let entry = stats.backends.entry((upstream.to_string(), backend.to_string())).or_default();
    *entry.requests.entry(status.map_or_else(|| "error".to_string(), |s| s.to_string())).or_default() += 1;
    for (i, upper) in BUCKETS.iter().enumerate() {
        if secs <= *upper {
            entry.buckets[i] += 1;
        }
    }
    entry.sum += secs;
    entry.count += 1;
}

pub(crate) fn observe_retry(upstream: &str) {
    *stats().lock().unwrap_or_else(|e| e.into_inner()).retries.entry(upstream.to_string()).or_default() += 1;
}

fn label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// The `rws_upstream_*` series for `/metrics`; empty without upstreams.
pub(crate) fn prometheus_text() -> String {
    let stats = stats().lock().unwrap_or_else(|e| e.into_inner());
    let mut out = String::new();
    if !stats.backends.is_empty() {
        let mut keys: Vec<&(String, String)> = stats.backends.keys().collect();
        keys.sort();
        out.push_str("# HELP rws_upstream_requests_total Requests sent to each upstream backend, by response status (\"error\": no response)\n");
        out.push_str("# TYPE rws_upstream_requests_total counter\n");
        for key in &keys {
            let entry = &stats.backends[key];
            let mut codes: Vec<&String> = entry.requests.keys().collect();
            codes.sort();
            for code in codes {
                out.push_str(&format!(
                    "rws_upstream_requests_total{{upstream=\"{}\",backend=\"{}\",code=\"{}\"}} {}\n",
                    label(&key.0), label(&key.1), code, entry.requests[code]
                ));
            }
        }
        out.push_str("# HELP rws_upstream_request_duration_seconds Time from sending a request to an upstream backend to its response head\n");
        out.push_str("# TYPE rws_upstream_request_duration_seconds histogram\n");
        for key in &keys {
            let entry = &stats.backends[key];
            let labels = format!("upstream=\"{}\",backend=\"{}\"", label(&key.0), label(&key.1));
            for (i, upper) in BUCKETS.iter().enumerate() {
                out.push_str(&format!("rws_upstream_request_duration_seconds_bucket{{{labels},le=\"{upper}\"}} {}\n", entry.buckets[i]));
            }
            out.push_str(&format!("rws_upstream_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} {}\n", entry.count));
            out.push_str(&format!("rws_upstream_request_duration_seconds_sum{{{labels}}} {:.6}\n", entry.sum));
            out.push_str(&format!("rws_upstream_request_duration_seconds_count{{{labels}}} {}\n", entry.count));
        }
    }
    if !stats.retries.is_empty() {
        let mut names: Vec<&String> = stats.retries.keys().collect();
        names.sort();
        out.push_str("# HELP rws_upstream_retries_total Requests sent again to another backend\n");
        out.push_str("# TYPE rws_upstream_retries_total counter\n");
        for name in names {
            out.push_str(&format!("rws_upstream_retries_total{{upstream=\"{}\"}} {}\n", label(name), stats.retries[name]));
        }
    }
    if !stats.policies.is_empty() {
        out.push_str("# HELP rws_upstream_backend_up Whether a backend is in rotation: passing health checks and its circuit breaker not open (1) or not (0)\n");
        out.push_str("# TYPE rws_upstream_backend_up gauge\n");
        let mut breaker_lines = String::new();
        for policy in &stats.policies {
            let live = policy.live.read().unwrap_or_else(|e| e.into_inner()).clone();
            let mut backends: Vec<&String> = policy.weights.keys().collect();
            backends.sort();
            for backend in backends {
                let state = policy.breaker_state(backend);
                let up = live.contains(backend) && state.as_ref().is_none_or(|s| *s != BreakerState::Open);
                let labels = format!("upstream=\"{}\",backend=\"{}\"", label(&policy.name), label(backend));
                out.push_str(&format!("rws_upstream_backend_up{{{labels}}} {}\n", u8::from(up)));
                if let Some(state) = state {
                    let value = match state {
                        BreakerState::Closed => 0,
                        BreakerState::HalfOpen => 1,
                        BreakerState::Open => 2,
                    };
                    breaker_lines.push_str(&format!("rws_upstream_circuit_breaker_state{{{labels}}} {value}\n"));
                }
            }
        }
        if !breaker_lines.is_empty() {
            out.push_str("# HELP rws_upstream_circuit_breaker_state Each backend's circuit breaker (0=closed, 1=half_open, 2=open)\n");
            out.push_str("# TYPE rws_upstream_circuit_breaker_state gauge\n");
            out.push_str(&breaker_lines);
        }
    }
    out
}

#[cfg(test)]
mod tests;
