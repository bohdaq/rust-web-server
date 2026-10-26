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
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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

/// Shards in a [`RequestLog`]. Each thread appends to its own shard, so
/// concurrent requests don't contend for one lock (a single global mutex
/// here was the top lock wait under load).
const SHARDS: usize = 16;

/// Ring buffer of recent requests, sharded by thread; entries carry a global
/// sequence number so readers can merge the shards in order. Each shard keeps
/// up to `capacity` entries and readers keep the newest `capacity` overall —
/// exactly what one ring buffer held, since the newest `capacity` entries
/// overall are always among the newest `capacity` of their own shard.
pub struct RequestLog {
    shards: Vec<Mutex<VecDeque<(u64, LogEntry)>>>,
    capacity: usize,
    sequence: AtomicU64,
}

/// This thread's shard index, assigned round-robin on first use.
fn shard_index() -> usize {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    thread_local! {
        static INDEX: usize = NEXT.fetch_add(1, Ordering::Relaxed) % SHARDS;
    }
    INDEX.with(|i| *i)
}

impl RequestLog {
    fn new(capacity: usize) -> Self {
        RequestLog {
            shards: (0..SHARDS).map(|_| Mutex::new(VecDeque::new())).collect(),
            capacity: capacity.max(1),
            sequence: AtomicU64::new(0),
        }
    }

    #[cfg(test)]
    fn push(&self, entry: LogEntry) {
        self.record(entry.timestamp, &entry.method, &entry.path, entry.status, &entry.client_ip, entry.latency_ms);
    }

    /// Append one entry. A full shard reuses its oldest entry's strings, so
    /// the steady state (every request once `capacity` is reached) allocates
    /// nothing — it was three `String`s allocated and three freed per request.
    fn record(&self, timestamp: u64, method: &str, path: &str, status: i16, client_ip: &str, latency_ms: u64) {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        let mut shard = self.shards[shard_index()].lock().unwrap_or_else(|e| e.into_inner());
        let entry = if shard.len() >= self.capacity {
            shard.pop_front().map(|(_, mut old)| {
                for (field, value) in [(&mut old.method, method), (&mut old.path, path), (&mut old.client_ip, client_ip)] {
                    field.clear();
                    field.push_str(value);
                }
                old.timestamp = timestamp;
                old.status = status;
                old.latency_ms = latency_ms;
                old
            })
        } else {
            None
        };
        let entry = entry.unwrap_or_else(|| LogEntry {
            timestamp,
            method: method.to_string(),
            path: path.to_string(),
            status,
            client_ip: client_ip.to_string(),
            latency_ms,
        });
        shard.push_back((seq, entry));
    }

    /// The newest `capacity` entries (oldest first) that match `keep`.
    fn merged(&self, keep: impl Fn(&LogEntry) -> bool) -> Vec<LogEntry> {
        let mut all: Vec<(u64, LogEntry)> = Vec::new();
        for shard in &self.shards {
            let shard = shard.lock().unwrap_or_else(|e| e.into_inner());
            all.extend(shard.iter().cloned());
        }
        all.sort_unstable_by_key(|(seq, _)| *seq);
        let skip = all.len().saturating_sub(self.capacity);
        all.into_iter().skip(skip).map(|(_, e)| e).filter(|e| keep(e)).collect()
    }

    /// Return up to `n` most recent entries (newest last).
    pub fn recent(&self, n: usize) -> Vec<LogEntry> {
        let all = self.merged(|_| true);
        let skip = all.len().saturating_sub(n);
        all.into_iter().skip(skip).collect()
    }

    /// Return up to `n` most recent entries with a 4xx or 5xx status.
    pub fn recent_errors(&self, n: usize) -> Vec<LogEntry> {
        let errors = self.merged(|e| e.status >= 400);
        let skip = errors.len().saturating_sub(n);
        errors.into_iter().skip(skip).collect()
    }

    /// Total number of entries currently held.
    pub fn len(&self) -> usize {
        let held: usize = self.shards.iter().map(|s| s.lock().unwrap_or_else(|e| e.into_inner()).len()).sum();
        held.min(self.capacity)
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
        let start = std::time::Instant::now();
        let result = next.execute(request, connection);
        let latency_ms = start.elapsed().as_millis() as u64;

        let status = match &result {
            Ok(r) => r.status_code,
            Err(_) => 500,
        };
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        global().record(now_secs(), &request.method, path, status, &connection.client.ip, latency_ms);

        result
    }

    /// In-memory only: safe on the event loop.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}
