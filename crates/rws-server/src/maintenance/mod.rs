//! Maintenance mode middleware.
//!
//! Set `MAINTENANCE_MODE` to `true` at runtime to return `503 Service
//! Unavailable` for all requests except `/healthz` and `/readyz`. Clear it to
//! resume normal traffic. [`turn_on`] does the same with a time to turn off
//! by itself and a `Retry-After` of your choice (18.38.0); [`turn_off`]
//! ends it.
//!
//! # Example
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::maintenance::{MAINTENANCE_MODE, MaintenanceLayer};
//! use std::sync::atomic::Ordering;
//!
//! let app = App::new().wrap(MaintenanceLayer);
//!
//! // Enable maintenance mode at runtime.
//! MAINTENANCE_MODE.store(true, Ordering::SeqCst);
//! ```

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Flip to `true` to activate maintenance mode; back to `false` to resume.
pub static MAINTENANCE_MODE: AtomicBool = AtomicBool::new(false);

/// Unix seconds maintenance mode was turned on with [`turn_on`]; 0 = unknown.
static SINCE: AtomicU64 = AtomicU64::new(0);
/// Unix seconds it turns off by itself; 0 = stays on until turned off.
static UNTIL: AtomicU64 = AtomicU64::new(0);
/// `Retry-After` chosen when it was turned on; -1 = `[maintenance] retry_after_secs`.
static RETRY_AFTER: AtomicI64 = AtomicI64::new(-1);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// What maintenance mode is doing now (18.38.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub active: bool,
    /// Unix seconds it was turned on; `None` when off or switched through
    /// [`MAINTENANCE_MODE`] directly.
    pub since: Option<u64>,
    /// Unix seconds it turns off by itself; `None` = until turned off.
    pub until: Option<u64>,
    /// The `Retry-After` a visitor gets now, in seconds; 0 = none.
    pub retry_after_secs: u64,
}

/// Turn maintenance mode on (18.38.0). `for_secs`: turn off by itself after
/// that long (`None` or 0: stay on until turned off). `retry_after_secs`:
/// the `Retry-After` to send instead of `[maintenance] retry_after_secs`
/// (0 sends none). Calling it again while on replaces both and keeps the
/// start time. End it with [`turn_off`], not by clearing
/// [`MAINTENANCE_MODE`], which would leave the timer behind.
pub fn turn_on(for_secs: Option<u64>, retry_after_secs: Option<u64>) {
    let now = now();
    if !MAINTENANCE_MODE.load(Ordering::SeqCst) || SINCE.load(Ordering::SeqCst) == 0 {
        SINCE.store(now, Ordering::SeqCst);
    }
    UNTIL.store(for_secs.filter(|s| *s > 0).map(|s| now.saturating_add(s)).unwrap_or(0), Ordering::SeqCst);
    RETRY_AFTER.store(retry_after_secs.map(|s| s.min(i64::MAX as u64) as i64).unwrap_or(-1), Ordering::SeqCst);
    MAINTENANCE_MODE.store(true, Ordering::SeqCst);
}

/// Keep maintenance mode on `add_secs` longer than its timer says
/// (18.38.0). `false` when it's off or has no timer to extend.
pub fn extend(add_secs: u64) -> bool {
    if !active() {
        return false;
    }
    let mut until = UNTIL.load(Ordering::SeqCst);
    while until != 0 {
        match UNTIL.compare_exchange(until, until.saturating_add(add_secs), Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return true,
            Err(now) => until = now,
        }
    }
    false
}

/// Turn maintenance mode off and forget its timer and `Retry-After` choice.
pub fn turn_off() {
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    SINCE.store(0, Ordering::SeqCst);
    UNTIL.store(0, Ordering::SeqCst);
    RETRY_AFTER.store(-1, Ordering::SeqCst);
}

/// Whether maintenance mode is on. A timer set with [`turn_on`] that has
/// run out turns it off here, so it ends on time whoever asks first.
pub fn active() -> bool {
    if !MAINTENANCE_MODE.load(Ordering::Relaxed) {
        return false;
    }
    let until = UNTIL.load(Ordering::Relaxed);
    if until != 0 && now() >= until {
        // Only the timer that ran out: a newer turn_on stays.
        if UNTIL.compare_exchange(until, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            turn_off();
        }
        return MAINTENANCE_MODE.load(Ordering::Relaxed);
    }
    true
}

/// The `Retry-After` for a maintenance response: the choice made when it
/// was turned on, else the configured one; never past the time it turns
/// off by itself.
fn retry_after() -> u64 {
    let chosen = RETRY_AFTER.load(Ordering::Relaxed);
    let retry = if chosen >= 0 { chosen as u64 } else { crate::error::page::maintenance_retry_after() };
    let until = UNTIL.load(Ordering::Relaxed);
    if retry > 0 && until != 0 {
        return retry.min(until.saturating_sub(now()).max(1));
    }
    retry
}

/// Maintenance mode now: on or off, since when, until when, and the
/// `Retry-After` in effect.
pub fn status() -> Status {
    let active = active();
    let some = |v: u64| (active && v != 0).then_some(v);
    Status { active, since: some(SINCE.load(Ordering::Relaxed)), until: some(UNTIL.load(Ordering::Relaxed)), retry_after_secs: retry_after() }
}

/// The maintenance response: problem details (unchanged for API clients)
/// or, for a browser, the HTML page with `[maintenance] message`; with
/// `Retry-After` (`[maintenance] retry_after_secs`, default 120) since 18.37.0;
/// the one chosen with [`turn_on`] since 18.38.0.
fn service_unavailable(request: &Request) -> Response {
    let mut response = crate::error::problem::problem_with_text(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "maintenance", "Service Temporarily Unavailable");
    let retry = retry_after();
    if retry > 0 {
        response.headers.push(crate::header::Header { name: "Retry-After".into(), value: retry.to_string().into() });
    }
    crate::error::page::negotiated(response, request)
}

/// Middleware that returns `503` when [`MAINTENANCE_MODE`] is `true`.
///
/// Health probe paths (`/healthz`, `/readyz`) always pass through so that
/// load balancers can still detect the pod is alive.
pub struct MaintenanceLayer;

impl Middleware for MaintenanceLayer {
    /// Headers only: a streamed body passes through untouched.
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }


    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if active() {
            let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
            if path != "/healthz" && path != "/readyz" {
                return Ok(service_unavailable(request));
            }
        }
        next.execute(request, connection)
    }

    /// In-memory only: safe on the event loop.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    /// Off, it does nothing: config-driven proxy routes keep their
    /// non-blocking forwarder (since 18.10.0, when the binary added this
    /// layer to proxy mode for the admin API's maintenance switch).
    fn is_passthrough(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        !active()
    }
}
