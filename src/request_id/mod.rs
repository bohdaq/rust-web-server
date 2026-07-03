//! Request ID / correlation ID middleware.
//!
//! Distributed tracing ([`crate::otel`]) creates a span per request but
//! doesn't give handlers a simple, stable identifier to put in their own log
//! lines — and doesn't propagate one across service boundaries unless the
//! caller already sends a W3C `traceparent`. [`RequestIdLayer`] fills that
//! gap: it's a plain string ID, present on both the request (so your handler
//! can read and log it) and the response (so the caller can log the same
//! value), that survives even in builds/setups without OpenTelemetry wired up.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::request_id::RequestIdLayer;
//!
//! let app = App::new().wrap(RequestIdLayer::new());
//! ```
//!
//! Reading it in a handler — the header is just a normal request header,
//! visible through any of the usual ways to read one:
//!
//! ```rust,no_run
//! use rust_web_server::request::Request;
//! use rust_web_server::request_id::DEFAULT_HEADER;
//!
//! fn handler(request: &Request) {
//!     let id = request.get_header(DEFAULT_HEADER.to_string()).map(|h| h.value.as_str()).unwrap_or("");
//!     println!("[{}] handling request", id);
//! }
//! ```

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::Application;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Default header name used for the request ID, both incoming and outgoing.
pub const DEFAULT_HEADER: &str = "X-Request-Id";

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a UUID-v4-*shaped* identifier (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`).
///
/// Not cryptographically random and not a spec-compliant UUID (version/variant
/// bits aren't forced) — built from a monotonic counter mixed with the
/// current time via a splitmix64 finalizer, the same non-crypto technique
/// already used elsewhere in this crate for unique-but-not-secret IDs (e.g.
/// session IDs). Good for correlating log lines across services; do not use
/// this as a security token, session ID, or anywhere uniqueness must be
/// adversarially guaranteed.
pub fn generate_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let count = ID_COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut x = nanos ^ count.wrapping_mul(0x9e3779b97f4a7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;

    let mut y = count ^ nanos.wrapping_mul(0x517cc1b727220a95);
    y ^= y >> 30;
    y = y.wrapping_mul(0xbf58476d1ce4e5b9);
    y ^= y >> 27;
    y = y.wrapping_mul(0x94d049bb133111eb);
    y ^= y >> 31;

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (x >> 32) as u32,
        ((x >> 16) & 0xffff) as u16,
        (x & 0xffff) as u16,
        ((y >> 48) & 0xffff) as u16,
        y & 0xffff_ffff_ffff,
    )
}

/// Middleware that ensures every request/response pair carries a stable
/// correlation ID.
///
/// - If the incoming request already has the header (e.g. set by an
///   upstream gateway, load balancer, or calling service), that exact value
///   is kept and echoed back unchanged — this lets one ID follow a request
///   across multiple services instead of getting a new one at each hop.
/// - Otherwise, a fresh ID is generated with [`generate_request_id`] and
///   injected into the request *before* it reaches your handler, so handlers
///   can read it like any other header.
/// - The same value is always set on the response, so the caller can log it
///   too — even if it arrived with no ID and this middleware generated one.
pub struct RequestIdLayer {
    header: String,
}

impl RequestIdLayer {
    /// Uses the default header name, [`DEFAULT_HEADER`] (`X-Request-Id`).
    pub fn new() -> Self {
        RequestIdLayer { header: DEFAULT_HEADER.to_string() }
    }

    /// Use a different header name, e.g. `"X-Correlation-Id"`.
    pub fn header(mut self, name: impl Into<String>) -> Self {
        self.header = name.into();
        self
    }
}

impl Default for RequestIdLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for RequestIdLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let existing = request.get_header(self.header.clone()).map(|h| h.value.clone());

        let (id, mut response) = match existing {
            Some(id) => (id, next.execute(request, connection)?),
            None => {
                let id = generate_request_id();
                let mut req = request.clone();
                req.headers.push(Header { name: self.header.clone(), value: id.clone() });
                (id, next.execute(&req, connection)?)
            }
        };

        response.headers.push(Header { name: self.header.clone(), value: id });
        Ok(response)
    }
}
