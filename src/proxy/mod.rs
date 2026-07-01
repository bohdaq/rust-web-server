//! Reverse proxy middleware with round-robin load balancing.
//!
//! `ReverseProxy` implements [`Middleware`] — wrap any application with it and
//! all matching requests are forwarded to one of the configured backends over
//! plain HTTP/1.1.  Failed backends are skipped and the next one is tried
//! before returning `502 Bad Gateway`.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::proxy::{LoadBalancing, ReverseProxy};
//!
//! // Proxy every request across two backends in round-robin order.
//! let app = App::new()
//!     .wrap(ReverseProxy::new(["http://backend-1:8080", "http://backend-2:8080"])
//!         .strategy(LoadBalancing::RoundRobin));
//!
//! // Only proxy /api/* requests; everything else is handled locally.
//! let app2 = App::new()
//!     .wrap(ReverseProxy::new(["http://api-service:3000"])
//!         .path_prefix("/api"));
//! ```

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
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

// Hop-by-hop headers that must not be forwarded (RFC 7230 §6.1)
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Load balancing strategy used by [`ReverseProxy`].
pub enum LoadBalancing {
    /// Distribute requests across backends in a cyclic order.
    RoundRobin,
}

/// Reverse proxy middleware.
///
/// Forwards incoming requests to one of the configured backends over HTTP/1.1.
/// On connection failure the next backend in the list is tried; when all
/// backends have failed the middleware returns `502 Bad Gateway`.
///
/// Hop-by-hop headers are stripped before forwarding.  `X-Forwarded-For` and
/// `Via` are added to every forwarded request.
///
/// # Limitations
///
/// * Only plain HTTP backends are supported (no TLS to the upstream).
/// * Chunked transfer encoding from the backend is forwarded as-is; callers
///   that need decoded bodies should set `Content-Length` on the upstream.
pub struct ReverseProxy {
    backends: Vec<Backend>,
    path_prefix: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    counter: AtomicUsize,
}

impl ReverseProxy {
    /// Create a proxy that distributes requests across `backends` in
    /// round-robin order.  Each entry must be `"http://host:port"` or
    /// `"host:port"` (port defaults to 80).
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            backends: backends
                .into_iter()
                .filter_map(|u| Backend::parse(u.as_ref()))
                .collect(),
            path_prefix: None,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            counter: AtomicUsize::new(0),
        }
    }

    /// Only proxy requests whose URI starts with `prefix`.
    ///
    /// Other requests are passed through to the next layer in the middleware
    /// chain (or the inner application).
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(prefix.into());
        self
    }

    /// Override the load balancing strategy (currently only `RoundRobin`).
    pub fn strategy(self, _strategy: LoadBalancing) -> Self {
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

    fn proxy(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if self.backends.is_empty() {
            return Err("no backends configured".to_string());
        }
        let n = self.backends.len();
        let start = self.counter.fetch_add(1, Ordering::Relaxed);
        for attempt in 0..n {
            let idx = (start + attempt) % n;
            match self.try_backend(request, connection, &self.backends[idx]) {
                Ok(resp) => return Ok(resp),
                Err(_) if attempt + 1 < n => continue,
                Err(e) => return Err(e),
            }
        }
        Err("all backends failed".to_string())
    }

    fn try_backend(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        backend: &Backend,
    ) -> Result<Response, String> {
        let addr_str = format!("{}:{}", backend.host, backend.port);
        let sock_addr = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("DNS lookup for {} failed: {}", addr_str, e))?
            .next()
            .ok_or_else(|| format!("no address resolved for {}", addr_str))?;

        let stream = TcpStream::connect_timeout(&sock_addr, self.connect_timeout)
            .map_err(|e| format!("connect to {} failed: {}", addr_str, e))?;
        stream
            .set_read_timeout(Some(self.read_timeout))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;

        let req_bytes = build_request(request, &backend.host, &connection.client.ip);
        let mut stream = stream;
        stream
            .write_all(&req_bytes)
            .map_err(|e| format!("write to backend failed: {}", e))?;

        let resp_bytes = read_response(&mut stream)?;
        Response::parse(&resp_bytes)
    }
}

impl Middleware for ReverseProxy {
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

fn build_request(request: &Request, backend_host: &str, client_ip: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let _ = write!(
        out,
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        request.method, request.request_uri, backend_host
    );
    for h in &request.headers {
        let lower = h.name.to_lowercase();
        if HOP_BY_HOP.contains(&lower.as_str()) || lower == "host" {
            continue;
        }
        let _ = write!(out, "{}: {}\r\n", h.name, h.value);
    }
    let _ = write!(out, "X-Forwarded-For: {}\r\n", client_ip);
    let _ = write!(out, "Via: 1.1 rws\r\n");
    let _ = write!(out, "Connection: close\r\n");
    if !request.body.is_empty() {
        let _ = write!(out, "Content-Length: {}\r\n", request.body.len());
    }
    let _ = write!(out, "\r\n");
    out.extend_from_slice(&request.body);
    out
}

fn read_response(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

    // Read until the header block ends (\r\n\r\n)
    let header_end = loop {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return if buf.is_empty() {
                Err("backend closed connection without sending a response".to_string())
            } else {
                Ok(buf)
            };
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    // Parse Content-Length from headers
    let content_length = std::str::from_utf8(&buf[..header_end])
        .unwrap_or("")
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .starts_with("content-length:")
                .then(|| line.splitn(2, ':').nth(1)?.trim().parse::<usize>().ok())
                .flatten()
        });

    match content_length {
        Some(len) => {
            while buf.len() < header_end + len {
                let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
        }
        None => loop {
            let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        },
    }

    Ok(buf)
}

fn bad_gateway() -> Response {
    let cr = Range::get_content_range(
        b"502 Bad Gateway".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE
        .n502_bad_gateway
        .reason_phrase
        .to_string();
    r.content_range_list = vec![cr];
    r
}

// ── Backend URL parsing ───────────────────────────────────────────────────────

struct Backend {
    host: String,
    port: u16,
}

impl Backend {
    fn parse(url: &str) -> Option<Self> {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("h2://"))
            .unwrap_or(url);
        // Drop any path component
        let host_port = rest.split('/').next().unwrap_or(rest);
        let (host, port) = if let Some(colon) = host_port.rfind(':') {
            let port_str = &host_port[colon + 1..];
            if let Ok(p) = port_str.parse::<u16>() {
                (host_port[..colon].to_string(), p)
            } else {
                (host_port.to_string(), 80)
            }
        } else {
            (host_port.to_string(), 80)
        };
        if host.is_empty() {
            return None;
        }
        Some(Backend { host, port })
    }
}

// ── HTTP/2 reverse proxy ──────────────────────────────────────────────────────

/// Reverse proxy that forwards requests to HTTP/2 backends.
///
/// Wraps [`ReverseProxy`] and forces HTTP/2 (`h2`) for all upstream connections.
/// Requires the `http2` Cargo feature.
///
/// This proxy also transparently handles gRPC traffic
/// (`Content-Type: application/grpc*`) — gRPC DATA frames are forwarded
/// as-is because gRPC is HTTP/2. Note that HTTP/2 trailers (used by gRPC for
/// `grpc-status` and `grpc-message`) are not yet propagated.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::proxy::H2ReverseProxy;
///
/// let app = App::new()
///     .wrap(H2ReverseProxy::new(["grpc-service:9090"])
///         .path_prefix("/svc.MyService"));
/// ```
#[cfg(feature = "http2")]
pub struct H2ReverseProxy {
    inner: ReverseProxy,
}

#[cfg(feature = "http2")]
impl H2ReverseProxy {
    /// Create a proxy distributing requests across `backends` in round-robin order.
    /// Each entry must be `"host:port"` or `"h2://host:port"`.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        H2ReverseProxy {
            inner: ReverseProxy::new(backends),
        }
    }

    /// Only proxy requests whose URI starts with `prefix`; pass others through.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.inner = self.inner.path_prefix(prefix);
        self
    }

    /// Override the TCP connect timeout (default: 5 s).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.inner = self.inner.connect_timeout_ms(ms);
        self
    }

    /// Override the response read timeout (default: 30 s).
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.inner = self.inner.read_timeout_ms(ms);
        self
    }
}

#[cfg(feature = "http2")]
impl crate::middleware::Middleware for H2ReverseProxy {
    fn handle(
        &self,
        request: &crate::request::Request,
        connection: &crate::server::ConnectionInfo,
        next: &dyn crate::application::Application,
    ) -> Result<crate::response::Response, String> {
        if let Some(prefix) = &self.inner.path_prefix {
            if !request.request_uri.starts_with(prefix.as_str()) {
                return next.execute(request, connection);
            }
        }
        if self.inner.backends.is_empty() {
            return Ok(bad_gateway());
        }
        let n = self.inner.backends.len();
        let start = self.inner.counter.fetch_add(1, Ordering::Relaxed);
        for attempt in 0..n {
            let idx = (start + attempt) % n;
            match try_backend_h2(request, &connection.client.ip, &self.inner.backends[idx],
                                  self.inner.connect_timeout, self.inner.read_timeout) {
                Ok(resp) => return Ok(resp),
                Err(_) if attempt + 1 < n => continue,
                Err(_) => break,
            }
        }
        Ok(bad_gateway())
    }
}

#[cfg(feature = "http2")]
fn try_backend_h2(
    request: &Request,
    client_ip: &str,
    backend: &Backend,
    connect_timeout: Duration,
    _read_timeout: Duration,
) -> Result<Response, String> {
    use tokio::runtime::Handle;
    match Handle::try_current() {
        Ok(_) => tokio::task::block_in_place(|| {
            Handle::current().block_on(forward_h2_async(request, client_ip, backend, connect_timeout))
        }),
        Err(_) => {
            // No tokio runtime (http1-only path): fall back to HTTP/1.1 upstream.
            Err("no async runtime for H2 proxy; falling back to 502".to_string())
        }
    }
}

#[cfg(feature = "http2")]
async fn forward_h2_async(
    request: &Request,
    client_ip: &str,
    backend: &Backend,
    connect_timeout: Duration,
) -> Result<Response, String> {
    use bytes::Bytes;
    use http as hc;

    let addr = format!("{}:{}", backend.host, backend.port);

    let tcp = tokio::time::timeout(
        connect_timeout,
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("h2 proxy: connect to {} timed out", addr))?
    .map_err(|e| format!("h2 proxy: connect to {} failed: {}", addr, e))?;

    let (send_req, conn) = h2::client::handshake(tcp)
        .await
        .map_err(|e| format!("h2 proxy: handshake with {} failed: {}", addr, e))?;

    tokio::spawn(async move {
        let _ = conn.await;
    });

    let uri_str = format!("http://{}{}", addr, request.request_uri);
    let uri: hc::Uri = uri_str.parse().map_err(|e: hc::uri::InvalidUri| e.to_string())?;
    let method = hc::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;

    let mut builder = hc::Request::builder().method(method).uri(uri);
    builder = builder.header("host", &backend.host);
    for h in &request.headers {
        let lower = h.name.to_lowercase();
        if HOP_BY_HOP.contains(&lower.as_str()) || lower == "host" {
            continue;
        }
        builder = builder.header(&h.name, &h.value);
    }
    builder = builder.header("x-forwarded-for", client_ip);
    builder = builder.header("via", "2 rws");

    let body_bytes = Bytes::from(request.body.clone());
    let end_of_stream = body_bytes.is_empty();
    let http_req = builder.body(()).map_err(|e| e.to_string())?;

    let mut send_req = send_req.ready().await.map_err(|e| e.to_string())?;
    let (resp_future, mut req_body) = send_req
        .send_request(http_req, end_of_stream)
        .map_err(|e| e.to_string())?;
    if !end_of_stream {
        req_body.send_data(body_bytes, true).map_err(|e| e.to_string())?;
    }

    let resp = resp_future.await.map_err(|e| e.to_string())?;
    let (parts, mut body) = resp.into_parts();

    let content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let mut body_bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = body.data().await {
        body_bytes.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
    }

    let mut response = Response::new();
    response.status_code = parts.status.as_u16() as i16;
    response.reason_phrase = parts.status.canonical_reason().unwrap_or("").to_string();

    const H2_HOP: &[&str] = &["connection", "keep-alive", "transfer-encoding",
                                "upgrade", "proxy-connection", "te"];
    for (name, value) in &parts.headers {
        let lower = name.as_str().to_lowercase();
        if H2_HOP.contains(&lower.as_str()) { continue; }
        if let Ok(v) = value.to_str() {
            response.headers.push(crate::header::Header {
                name: name.as_str().to_string(),
                value: v.to_string(),
            });
        }
    }

    if !body_bytes.is_empty() {
        response.content_range_list = vec![Range::get_content_range(body_bytes, content_type)];
    }

    Ok(response)
}

// ── gRPC proxy ────────────────────────────────────────────────────────────────

/// gRPC reverse proxy middleware.
///
/// Recognises requests with `Content-Type: application/grpc*` and forwards them
/// to a backend over HTTP/2, leaving all other requests to the next layer.
///
/// gRPC DATA frames are forwarded as-is because gRPC is layered directly on
/// HTTP/2. HTTP/2 trailers (`grpc-status`, `grpc-message`) are not yet
/// propagated — a known limitation of the current implementation.
///
/// Requires the `http2` Cargo feature.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::proxy::GrpcProxy;
///
/// let app = App::new()
///     .wrap(GrpcProxy::new(["grpc-service:50051"]));
/// ```
#[cfg(feature = "http2")]
pub struct GrpcProxy {
    inner: H2ReverseProxy,
}

#[cfg(feature = "http2")]
impl GrpcProxy {
    /// Create a proxy distributing gRPC connections across `backends` in round-robin order.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        GrpcProxy { inner: H2ReverseProxy::new(backends) }
    }

    /// Only proxy requests whose URI starts with `prefix`; pass others through.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.inner = self.inner.path_prefix(prefix);
        self
    }
}

#[cfg(feature = "http2")]
impl crate::middleware::Middleware for GrpcProxy {
    fn handle(
        &self,
        request: &crate::request::Request,
        connection: &crate::server::ConnectionInfo,
        next: &dyn crate::application::Application,
    ) -> Result<crate::response::Response, String> {
        let ct = request
            .get_header("content-type".to_string())
            .map(|h| h.value.as_str())
            .unwrap_or("");
        if ct.starts_with("application/grpc") {
            self.inner.handle(request, connection, next)
        } else {
            next.execute(request, connection)
        }
    }
}
