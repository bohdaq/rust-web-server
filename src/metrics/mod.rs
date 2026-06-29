/// Global server-state metrics — thread-safe counters accessible from all code paths.
///
/// All fields are static atomics; no allocation or locking needed.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

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

/// Returns a Prometheus text-format snapshot of all metrics.
pub fn prometheus_text() -> String {
    let requests = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let errors   = ERRORS_TOTAL.load(Ordering::Relaxed);
    let active   = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);

    format!(
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
    )
}
