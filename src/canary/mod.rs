//! Weighted canary / A-B traffic splitting middleware.
//!
//! [`CanaryLayer`] implements [`Middleware`] and distributes incoming requests
//! across a set of backends according to configurable weights.  A backend with
//! weight 3 receives three times as many requests as one with weight 1.
//!
//! Backends are contacted over plain HTTP/1.1, or over TLS when the backend
//! URL uses an `https://`, `h2s://`, or `grpcs://` scheme (requires the
//! `http-client` or `http2` feature — both pull in `rustls`). If a backend is
//! unavailable the next one in the rotation is tried; after exhausting all
//! backends the middleware returns `502 Bad Gateway`.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::canary::{CanaryLayer, WeightedBackend};
//! use rust_web_server::middleware::WithMiddleware;
//!
//! // 75 % of traffic → stable, 25 % → canary
//! let app = WithMiddleware::new(App::new())
//!     .wrap(
//!         CanaryLayer::new(vec![
//!             WeightedBackend::new("http://stable:8080", 3),
//!             WeightedBackend::new("http://canary:8080", 1),
//!         ])
//!         .path_prefix("/api"),
//!     );
//! ```

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::application::Application;
use crate::core::New;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

// ── WeightedBackend ───────────────────────────────────────────────────────────

/// A backend URL together with a relative traffic weight.
///
/// A weight of 0 causes the backend to be skipped entirely.
#[derive(Clone)]
pub struct WeightedBackend {
    pub url: String,
    pub weight: u32,
}

impl WeightedBackend {
    /// Create a new weighted backend.
    pub fn new(url: impl Into<String>, weight: u32) -> Self {
        Self { url: url.into(), weight }
    }
}

// ── CanaryLayer ───────────────────────────────────────────────────────────────

/// Weighted traffic-splitting proxy middleware.
///
/// The rotation is pre-expanded so that each backend appears exactly `weight`
/// times.  An atomic counter selects the next entry in the rotation on every
/// request, giving a deterministic, lock-free weighted round-robin distribution.
pub struct CanaryLayer {
    /// Expanded rotation: each entry is `(host, port, tls)` and appears
    /// `weight` times. `tls` is set when the backend's URL used an
    /// `https://`/`h2s://`/`grpcs://` scheme.
    pub(crate) rotation: Vec<(String, u16, bool)>,
    counter: AtomicUsize,
    connect_timeout: Duration,
    read_timeout: Duration,
    path_prefix: Option<String>,
}

impl CanaryLayer {
    /// Build a `CanaryLayer` from the given weighted backends.
    ///
    /// Backends with `weight == 0` are ignored.
    pub fn new(backends: Vec<WeightedBackend>) -> Self {
        let mut rotation: Vec<(String, u16, bool)> = Vec::new();
        for wb in &backends {
            if wb.weight == 0 {
                continue;
            }
            if let Some((host, port, tls)) = parse_backend_url(&wb.url) {
                for _ in 0..wb.weight {
                    rotation.push((host.clone(), port, tls));
                }
            }
        }
        Self {
            rotation,
            counter: AtomicUsize::new(0),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            path_prefix: None,
        }
    }

    /// Only proxy requests whose URI starts with `prefix`; pass others through.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(prefix.into());
        self
    }

    /// Override the TCP connect timeout (default: 5 000 ms).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.connect_timeout = Duration::from_millis(ms);
        self
    }

    /// Override the response read timeout (default: 30 000 ms).
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.read_timeout = Duration::from_millis(ms);
        self
    }

    /// Try every backend in rotation order until one succeeds.
    fn proxy(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if self.rotation.is_empty() {
            return Err("CanaryLayer: no backends in rotation".to_string());
        }
        let n = self.rotation.len();
        let start = self.counter.fetch_add(1, Ordering::Relaxed);
        // Deduplicate by (host, port) so we don't hit the same backend twice
        // when it appears multiple times in the rotation.
        let mut tried: Vec<usize> = Vec::new();
        for attempt in 0..n {
            let idx = (start + attempt) % n;
            let backend = &self.rotation[idx];
            // Check if we already tried this (host, port) pair.
            let already_tried = tried.iter().any(|&i| self.rotation[i] == *backend);
            if already_tried {
                continue;
            }
            tried.push(idx);
            let (host, port, tls) = backend;
            let result = if *tls {
                #[cfg(any(feature = "http-client", feature = "http2"))]
                {
                    crate::proxy::proxy_https1(
                        request,
                        &connection.client.ip,
                        host,
                        *port,
                        self.connect_timeout,
                        self.read_timeout,
                    )
                }
                #[cfg(not(any(feature = "http-client", feature = "http2")))]
                {
                    Err("CanaryLayer: TLS backend requires the http-client or http2 feature".to_string())
                }
            } else {
                crate::proxy::proxy_http1(
                    request,
                    &connection.client.ip,
                    host,
                    *port,
                    self.connect_timeout,
                    self.read_timeout,
                )
            };
            match result {
                Ok(resp) => return Ok(resp),
                Err(_) => continue,
            }
        }
        Err("CanaryLayer: all backends failed".to_string())
    }
}

impl Middleware for CanaryLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if let Some(prefix) = &self.path_prefix {
            if !request.request_uri.starts_with(prefix.as_str()) {
                return next.execute(request, connection);
            }
        }
        match self.proxy(request, connection) {
            Ok(resp) => Ok(resp),
            Err(_) => Ok(bad_gateway()),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse a backend URL of the form `[scheme://]host[:port][/path]` into
/// `(host, port, tls)`.
///
/// `https://`, `h2s://`, and `grpcs://` set `tls = true` and default to port
/// 443; `http://`, `h2://`, `grpc://`, and a bare `host[:port]` set
/// `tls = false` and default to port 80 — matching `proxy::Backend::parse`'s
/// scheme conventions.
fn parse_backend_url(url: &str) -> Option<(String, u16, bool)> {
    let (rest, tls, default_port) = if let Some(r) = url.strip_prefix("https://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("h2s://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("grpcs://") {
        (r, true, 443u16)
    } else if let Some(r) = url.strip_prefix("http://") {
        (r, false, 80u16)
    } else if let Some(r) = url.strip_prefix("h2://") {
        (r, false, 80u16)
    } else if let Some(r) = url.strip_prefix("grpc://") {
        (r, false, 80u16)
    } else {
        (url, false, 80u16)
    };
    // Drop any path component
    let host_port = rest.split('/').next().unwrap_or(rest);
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (host_port[..colon].to_string(), p)
        } else {
            (host_port.to_string(), default_port)
        }
    } else {
        (host_port.to_string(), default_port)
    };
    if host.is_empty() { None } else { Some((host, port, tls)) }
}

fn bad_gateway() -> Response {
    let cr = Range::get_content_range(
        b"502 Bad Gateway".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}
