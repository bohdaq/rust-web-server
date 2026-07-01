//! In-memory ring buffer of recent HTTP requests.
//!
//! `LogLayer` middleware records each completed request. The global
//! `RequestLog` holds the most recent N entries (default 1000). MCP tools
//! `recent_requests` and `recent_errors` read from it.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::request_log::{self, LogLayer};
//!
//! let app = App::new().wrap(LogLayer);
//!
//! let entries = request_log::global().recent(20);
//! ```

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// A single recorded request.
#[derive(Clone)]
pub struct LogEntry {
    /// Unix timestamp (seconds) when the request completed.
    pub timestamp: u64,
    pub method: String,
    pub path: String,
    pub status: i16,
    pub client_ip: String,
    pub latency_ms: u64,
}

/// Ring buffer of recent requests.
pub struct RequestLog {
    entries: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl RequestLog {
    fn new(capacity: usize) -> Self {
        RequestLog {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    fn push(&self, entry: LogEntry) {
        let mut guard = self.entries.lock().unwrap();
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back(entry);
    }

    /// Return up to `n` most recent entries (newest last).
    pub fn recent(&self, n: usize) -> Vec<LogEntry> {
        let guard = self.entries.lock().unwrap();
        let skip = guard.len().saturating_sub(n);
        guard.iter().skip(skip).cloned().collect()
    }

    /// Return up to `n` most recent entries with a 4xx or 5xx status.
    pub fn recent_errors(&self, n: usize) -> Vec<LogEntry> {
        let guard = self.entries.lock().unwrap();
        let errors: Vec<LogEntry> = guard.iter()
            .filter(|e| e.status >= 400)
            .cloned()
            .collect();
        let skip = errors.len().saturating_sub(n);
        errors.into_iter().skip(skip).collect()
    }

    /// Total number of entries currently held.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

static INSTANCE: OnceLock<RequestLog> = OnceLock::new();

/// Return the process-wide `RequestLog` singleton (capacity 1000).
pub fn global() -> &'static RequestLog {
    INSTANCE.get_or_init(|| RequestLog::new(1000))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Middleware that records each request into the global [`RequestLog`].
pub struct LogLayer;

impl Middleware for LogLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let start = std::time::Instant::now();
        let result = next.execute(request, connection);
        let latency_ms = start.elapsed().as_millis() as u64;

        let status = match &result {
            Ok(r) => r.status_code,
            Err(_) => 500,
        };
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri).to_string();

        global().push(LogEntry {
            timestamp: now_secs(),
            method: request.method.clone(),
            path,
            status,
            client_ip: connection.client.ip.clone(),
            latency_ms,
        });

        result
    }
}
