//! Runtime IP blocklist middleware.
//!
//! Unlike [`crate::ip_filter::IpFilter`] (configured at startup), `Blocklist`
//! is mutable at runtime — add and remove IPs while the server is running.
//! The global singleton is accessible from MCP tools, admin handlers, and
//! middleware without passing explicit references.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::blocklist::{self, BlocklistLayer};
//!
//! let app = App::new().wrap(BlocklistLayer);
//!
//! // Block an IP at runtime.
//! blocklist::global().block("1.2.3.4");
//!
//! // Unblock later.
//! blocklist::global().unblock("1.2.3.4");
//! ```

#[cfg(test)]
mod tests;

use std::sync::{Mutex, OnceLock};

use crate::application::Application;
use crate::error::{AppError, IntoResponse};
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// A thread-safe list of blocked IPv4 addresses.
pub struct Blocklist {
    denied: Mutex<Vec<String>>,
}

impl Blocklist {
    fn new() -> Self {
        Blocklist { denied: Mutex::new(Vec::new()) }
    }

    /// Add `ip` to the blocklist. No-op if already present.
    pub fn block(&self, ip: &str) {
        let mut guard = self.denied.lock().unwrap();
        if !guard.iter().any(|e| e == ip) {
            guard.push(ip.to_string());
        }
    }

    /// Remove `ip` from the blocklist. No-op if not present.
    pub fn unblock(&self, ip: &str) {
        self.denied.lock().unwrap().retain(|e| e != ip);
    }

    /// `true` if `ip` is currently blocked.
    pub fn is_blocked(&self, ip: &str) -> bool {
        self.denied.lock().unwrap().iter().any(|e| e == ip)
    }

    /// Snapshot of all blocked IPs in insertion order.
    pub fn list(&self) -> Vec<String> {
        self.denied.lock().unwrap().clone()
    }

    /// Remove all entries.
    pub fn clear(&self) {
        self.denied.lock().unwrap().clear();
    }
}

static INSTANCE: OnceLock<Blocklist> = OnceLock::new();

/// Return the process-wide `Blocklist` singleton.
pub fn global() -> &'static Blocklist {
    INSTANCE.get_or_init(Blocklist::new)
}

/// Middleware that checks each request's client IP against [`global()`].
///
/// Blocked IPs receive `403 Forbidden`. All other requests pass through
/// to the next layer.
pub struct BlocklistLayer;

impl Middleware for BlocklistLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if global().is_blocked(&connection.client.ip) {
            return Ok(AppError::Forbidden.into_response());
        }
        next.execute(request, connection)
    }
}
