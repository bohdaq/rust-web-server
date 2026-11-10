//! Health checks for Kubernetes probes (18.36.0): register what "alive",
//! "ready" and "started" mean for your app, and `/livez`, `/readyz` and
//! `/startupz` run them.
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::health;
//!
//! health::readiness("cache", || {
//!     // e.g. ping Redis; Err(message) takes the pod out of the Service.
//!     Ok(())
//! });
//! health::startup("warm-up", || if warmed() { Ok(()) } else { Err("loading the index".into()) });
//! # fn warmed() -> bool { true }
//! assert!(health::report(health::Probe::Readiness).ok);
//! ```
//!
//! | Endpoint | Probe | `200` when |
//! |---|---|---|
//! | `/livez` (also `/healthz`) | liveness | every liveness check passes (none by default: the process answers) |
//! | `/readyz` | readiness | the server is up and not shutting down, and every readiness check passes |
//! | `/startupz` | startup | every startup check has passed once (then it stays `200`) |
//!
//! Each check runs with a timeout ([`set_timeout`], default 2 s; a check
//! that takes longer fails). Add `?verbose` for one line per check
//! (`[+]cache ok`, `[-]db failed: …`), as the Kubernetes API server does;
//! otherwise the body is `OK` or the failing checks. Endpoints with checks
//! run off the event loop, so a check may block (a database ping).
//!
//! Keep liveness checks to what a restart fixes (a deadlocked worker);
//! a database outage belongs in readiness, or every pod restarts at once.

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

/// Which probe a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Probe {
    Liveness,
    Readiness,
    Startup,
}

impl Probe {
    /// The endpoint path.
    pub fn path(self) -> &'static str {
        match self {
            Probe::Liveness => "/livez",
            Probe::Readiness => "/readyz",
            Probe::Startup => "/startupz",
        }
    }
}

type Check = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// A set of checks. The module functions use the process-wide one the
/// probe endpoints answer from; build your own for tests.
pub struct Registry {
    checks: RwLock<Vec<(Probe, String, Check)>>,
    timeout_ms: AtomicU64,
    started: AtomicBool,
}

impl Default for Registry {
    fn default() -> Self {
        Registry { checks: RwLock::new(Vec::new()), timeout_ms: AtomicU64::new(2000), started: AtomicBool::new(false) }
    }
}

/// The process-wide registry.
pub fn global() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::default)
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }

    /// Add a check to `probe` (one with the same probe and name is replaced).
    pub fn register(&self, probe: Probe, name: &str, check: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
        let mut checks = self.checks.write().unwrap_or_else(|e| e.into_inner());
        checks.retain(|(p, n, _)| !(*p == probe && n == name));
        checks.push((probe, name.to_string(), Arc::new(check)));
    }

    /// Remove a check; whether it was there.
    pub fn unregister(&self, probe: Probe, name: &str) -> bool {
        let mut checks = self.checks.write().unwrap_or_else(|e| e.into_inner());
        let before = checks.len();
        checks.retain(|(p, n, _)| !(*p == probe && n == name));
        checks.len() != before
    }

    /// How long one check may take before it counts as failed (default 2 s).
    pub fn set_timeout(&self, timeout: Duration) {
        self.timeout_ms.store(timeout.as_millis().max(1) as u64, Ordering::Relaxed);
    }

    /// Whether `probe` has checks.
    pub fn has_checks(&self, probe: Probe) -> bool {
        self.checks.read().unwrap_or_else(|e| e.into_inner()).iter().any(|(p, _, _)| *p == probe)
    }

    /// Run `probe`'s checks now, concurrently, each within the timeout.
    /// Readiness also fails while the server isn't ready (starting or
    /// shutting down); startup, once passed, stays passed.
    pub fn report(&self, probe: Probe) -> Report {
        let checks: Vec<(String, Check)> = self
            .checks
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(p, _, _)| *p == probe)
            .map(|(_, n, c)| (n.clone(), Arc::clone(c)))
            .collect();
        if probe == Probe::Startup && self.started.load(Ordering::Relaxed) {
            return Report { probe, ok: true, checks: checks.into_iter().map(|(name, _)| CheckResult { name, error: None }).collect(), note: None };
        }
        let timeout = Duration::from_millis(self.timeout_ms.load(Ordering::Relaxed));
        let handles: Vec<(String, std::thread::JoinHandle<Result<(), String>>)> =
            checks.into_iter().map(|(name, check)| (name, std::thread::spawn(move || run_one(check, timeout)))).collect();
        let results: Vec<CheckResult> = handles
            .into_iter()
            .map(|(name, h)| CheckResult { name, error: h.join().unwrap_or_else(|_| Err("the check panicked".into())).err() })
            .collect();
        let mut note = None;
        if probe == Probe::Readiness && !crate::metrics::SERVER_READY.load(Ordering::Relaxed) {
            note = Some("server not ready (starting or shutting down)".to_string());
        }
        let ok = note.is_none() && results.iter().all(|c| c.error.is_none());
        if probe == Probe::Startup && ok {
            self.started.store(true, Ordering::Relaxed);
        }
        Report { probe, ok, checks: results, note }
    }
}

/// Add a check to `probe` in the process-wide registry.
pub fn register(probe: Probe, name: &str, check: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
    global().register(probe, name, check)
}

/// A liveness check (`/livez`, `/healthz`).
pub fn liveness(name: &str, check: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
    register(Probe::Liveness, name, check)
}

/// A readiness check (`/readyz`).
pub fn readiness(name: &str, check: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
    register(Probe::Readiness, name, check)
}

/// A startup check (`/startupz`).
pub fn startup(name: &str, check: impl Fn() -> Result<(), String> + Send + Sync + 'static) {
    register(Probe::Startup, name, check)
}

/// An async check, awaited on the shared runtime (`http2`).
#[cfg(feature = "http2")]
pub fn register_async<F, Fut>(probe: Probe, name: &str, check: F)
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    register(probe, name, move || crate::async_bridge::block_on_shared(check()))
}

/// A readiness check that the database answers `SELECT 1` (`model-*`).
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub fn database(name: &str, pool: crate::model::DbPool) {
    register_async(Probe::Readiness, name, move || {
        let pool = pool.clone();
        async move { pool.query_rows("SELECT 1", &[]).await.map(|_| ()).map_err(|e| e.to_string()) }
    })
}

/// Remove a check from the process-wide registry.
pub fn unregister(probe: Probe, name: &str) -> bool {
    global().unregister(probe, name)
}

/// The process-wide check timeout (default 2 s).
pub fn set_timeout(timeout: Duration) {
    global().set_timeout(timeout)
}

/// Whether `probe` has checks in the process-wide registry.
pub fn has_checks(probe: Probe) -> bool {
    global().has_checks(probe)
}

/// One check's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub name: String,
    /// `None`: passed; `Some(message)`: failed.
    pub error: Option<String>,
}

/// A probe's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub probe: Probe,
    pub ok: bool,
    pub checks: Vec<CheckResult>,
    /// Why it failed besides its checks (`server not ready`).
    pub note: Option<String>,
}

impl Report {
    /// The probe's body: `OK`, or one line per failed check; with
    /// `verbose`, one line per check and a summary.
    pub fn body(&self, verbose: bool) -> String {
        let mut lines = Vec::new();
        if let Some(note) = &self.note {
            lines.push(format!("[-]{note}"));
        }
        for c in &self.checks {
            match &c.error {
                None if verbose => lines.push(format!("[+]{} ok", c.name)),
                None => {}
                Some(e) => lines.push(format!("[-]{} failed: {e}", c.name)),
            }
        }
        if verbose {
            let what = &self.probe.path()[1..];
            lines.push(if self.ok { format!("{what} check passed") } else { format!("{what} check failed") });
            lines.join("\n")
        } else if self.ok {
            "OK".to_string()
        } else {
            lines.join("\n")
        }
    }
}

fn run_one(check: Check, timeout: Duration) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("rws-health".into()).spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check())).unwrap_or_else(|_| Err("the check panicked".into()));
        let _ = tx.send(result);
    });
    if spawned.is_err() {
        return Err("could not start the check".into());
    }
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => Err(format!("timed out after {} ms", timeout.as_millis())),
    }
}

/// Run `probe`'s checks in the process-wide registry.
pub fn report(probe: Probe) -> Report {
    global().report(probe)
}

/// The probe a request path asks for (`/healthz` is liveness).
pub fn probe_for_path(path: &str) -> Option<Probe> {
    match path {
        "/livez" | "/healthz" => Some(Probe::Liveness),
        "/readyz" => Some(Probe::Readiness),
        "/startupz" => Some(Probe::Startup),
        _ => None,
    }
}

/// The response for a probe request: `200` or `503`, text.
pub fn respond(probe: Probe, verbose: bool) -> crate::response::Response {
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    let report = report(probe);
    let status = if report.ok { STATUS_CODE_REASON_PHRASE.n200_ok } else { STATUS_CODE_REASON_PHRASE.n503_service_unavailable };
    let body = report.body(verbose);
    let body = if !report.ok && body.is_empty() { "not ready".to_string() } else { body };
    let mut response = Response::text(status, &body);
    response.headers.push(crate::header::Header::new(crate::header::Header::_CACHE_CONTROL, "no-store"));
    response
}

