//! Maintenance mode middleware.
//!
//! Set `MAINTENANCE_MODE` to `true` at runtime to return `503 Service
//! Unavailable` for all requests except `/healthz` and `/readyz`. Clear it to
//! resume normal traffic.
//!
//! # Example
//!
//! ```rust,no_run
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

use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::Application;
use crate::core::New as _;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::range::Range;
use crate::mime_type::MimeType;
use crate::server::ConnectionInfo;

/// Flip to `true` to activate maintenance mode; back to `false` to resume.
pub static MAINTENANCE_MODE: AtomicBool = AtomicBool::new(false);

fn service_unavailable() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n503_service_unavailable.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        b"Service Temporarily Unavailable".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

/// Middleware that returns `503` when [`MAINTENANCE_MODE`] is `true`.
///
/// Health probe paths (`/healthz`, `/readyz`) always pass through so that
/// load balancers can still detect the pod is alive.
pub struct MaintenanceLayer;

impl Middleware for MaintenanceLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if MAINTENANCE_MODE.load(Ordering::Relaxed) {
            let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
            if path != "/healthz" && path != "/readyz" {
                return Ok(service_unavailable());
            }
        }
        next.execute(request, connection)
    }
}
