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

pub mod pool;

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use pool::ConnPool;

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
/// Idle connections are pooled and reused across requests (up to
/// [`ConnPool::new_default`] limits: 8 idle per backend, 60-second timeout).
/// This eliminates per-request TCP handshake overhead and ephemeral-port
/// exhaustion.  Use [`ReverseProxy::with_pool`] to share a pool across
/// multiple proxy instances or to tune pool parameters.
pub struct ReverseProxy {
    backends: Vec<Backend>,
    path_prefix: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    counter: AtomicUsize,
    pool: Arc<ConnPool>,
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
            pool: Arc::new(ConnPool::new_default()),
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

    /// Attach a shared connection pool.
    ///
    /// Useful for sharing one pool across multiple `ReverseProxy` instances
    /// or for tuning pool parameters (capacity, idle timeout).
    pub fn with_pool(mut self, pool: Arc<ConnPool>) -> Self {
        self.pool = pool;
        self
    }

    /// Set the maximum number of idle connections per backend (default: 8).
    pub fn max_idle_conns(mut self, n: usize) -> Self {
        self.pool = Arc::new(ConnPool::new(n, Duration::from_secs(60)));
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
        let key = format!("{}:{}", backend.host, backend.port);

        // Try a pooled connection first; fall back to a fresh one.
        let stream = if let Some(pooled) = self.pool.acquire(&key) {
            pooled
        } else {
            let addr_str = key.as_str();
            let sock_addr = addr_str
                .to_socket_addrs()
                .map_err(|e| format!("DNS lookup for {} failed: {}", addr_str, e))?
                .next()
                .ok_or_else(|| format!("no address resolved for {}", addr_str))?;
            TcpStream::connect_timeout(&sock_addr, self.connect_timeout)
                .map_err(|e| format!("connect to {} failed: {}", addr_str, e))?
        };

        stream.set_read_timeout(Some(self.read_timeout)).map_err(|e| e.to_string())?;
        stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;

        // keep_alive = true: send Connection: keep-alive so the server holds
        // the connection open after responding.
        let req_bytes = build_request(request, &backend.host, &connection.client.ip, true);
        let mut stream = stream;
        stream.write_all(&req_bytes).map_err(|e| format!("write to backend failed: {}", e))?;

        let mut tmp = [0u8; 4096];
        let (header_bytes, body_prefix) = read_headers_only(&mut stream, &mut tmp)?;
        let header_lower =
            std::str::from_utf8(&header_bytes).unwrap_or("").to_ascii_lowercase();

        if should_stream_response(&header_lower) {
            // Streaming path — pipe bytes straight to the client.
            // The connection cannot be reused while the body is in flight.
            let mut resp = parse_status_and_headers(&header_bytes)?;
            resp.stream_pipe =
                Some(Box::new(ConcatReader::new(body_prefix, stream)));
            Ok(resp)
        } else {
            // Buffered path — read the full body, then optionally return the
            // connection to the pool.
            let (resp_bytes, reusable) =
                read_response_from_partial(&mut stream, header_bytes, body_prefix, &mut tmp)?;
            if reusable {
                self.pool.release(&key, stream);
            }
            Response::parse(&resp_bytes)
        }
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

pub(crate) fn build_request(
    request: &Request,
    backend_host: &str,
    client_ip: &str,
    keep_alive: bool,
) -> Vec<u8> {
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
    if keep_alive {
        let _ = write!(out, "Connection: keep-alive\r\n");
    } else {
        let _ = write!(out, "Connection: close\r\n");
    }
    if !request.body.is_empty() {
        let _ = write!(out, "Content-Length: {}\r\n", request.body.len());
    }
    let _ = write!(out, "\r\n");
    out.extend_from_slice(&request.body);
    out
}

/// Decode HTTP/1.1 chunked transfer-encoding from `stream`.
///
/// `buf[header_end..]` may already contain some body bytes that arrived in
/// the same read as the headers.  Returns the fully decoded body.
fn decode_chunked(
    stream: &mut TcpStream,
    buf: &[u8],
    header_end: usize,
    tmp: &mut [u8],
) -> Result<Vec<u8>, String> {
    // Seed `raw` with any body bytes already buffered alongside the headers.
    let mut raw: Vec<u8> = buf[header_end..].to_vec();
    let mut decoded: Vec<u8> = Vec::new();

    loop {
        // Wait until we have at least one complete chunk-size line (ends with \r\n).
        let crlf = loop {
            if let Some(p) = raw.windows(2).position(|w| w == b"\r\n") {
                break p;
            }
            let n = stream.read(tmp).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("chunked: premature EOF reading chunk size".to_string());
            }
            raw.extend_from_slice(&tmp[..n]);
        };

        // Chunk size is hex, optionally followed by chunk-extensions (";…").
        let size_line = std::str::from_utf8(&raw[..crlf])
            .map_err(|_| "chunked: non-UTF-8 chunk size line".to_string())?;
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_str, 16)
            .map_err(|_| format!("chunked: invalid chunk size '{}'", size_str))?;
        raw.drain(..crlf + 2); // consume "<size>\r\n"

        if chunk_size == 0 {
            // Last chunk — consume the trailing CRLF ("0\r\n\r\n" → trailing "\r\n" still pending).
            while raw.len() < 2 {
                let n = stream.read(tmp).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&tmp[..n]);
            }
            break;
        }

        // Read chunk data + trailing CRLF.
        while raw.len() < chunk_size + 2 {
            let n = stream.read(tmp).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("chunked: premature EOF reading chunk body".to_string());
            }
            raw.extend_from_slice(&tmp[..n]);
        }
        decoded.extend_from_slice(&raw[..chunk_size]);
        raw.drain(..chunk_size + 2); // consume "<data>\r\n"
    }

    Ok(decoded)
}

/// Rewrite `buf` in-place: strip `Transfer-Encoding`, add `Content-Length`,
/// replace the old (undecoded) body with `decoded`.
fn rewrite_as_content_length(buf: &mut Vec<u8>, header_end: usize, decoded: &[u8]) {
    let header_str = std::str::from_utf8(&buf[..header_end]).unwrap_or("").to_string();
    buf.clear();
    for line in header_str.lines() {
        if line.to_ascii_lowercase().starts_with("transfer-encoding:") || line.is_empty() {
            continue;
        }
        buf.extend_from_slice(line.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    let _ = write!(buf, "Content-Length: {}\r\n\r\n", decoded.len());
    buf.extend_from_slice(decoded);
}

/// Non-pooled version of the response reader, used by callers that send
/// `Connection: close` (e.g. `proxy_http1`, `proxy_https1`).
pub(crate) fn read_response(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    read_response_from(stream)
}

pub(crate) fn read_response_from<R: Read>(stream: &mut R) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

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

/// Forward a single HTTP/1.1 request to `host:port` and return the response.
///
/// This is the shared low-level building block used by [`crate::canary`] and
/// [`crate::ingress`] so they don't have to duplicate the TCP + request/response
/// marshalling code.
pub(crate) fn proxy_http1(
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    use std::net::ToSocketAddrs;
    let addr_str = format!("{}:{}", host, port);
    let sock_addr = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS lookup for {} failed: {}", addr_str, e))?
        .next()
        .ok_or_else(|| format!("no address resolved for {}", addr_str))?;
    let stream = TcpStream::connect_timeout(&sock_addr, connect_timeout)
        .map_err(|e| format!("connect to {} failed: {}", addr_str, e))?;
    stream.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
    let req_bytes = build_request(request, host, client_ip, false);
    let mut stream = stream;
    stream.write_all(&req_bytes).map_err(|e| format!("write to backend failed: {}", e))?;
    let resp_bytes = read_response(&mut stream)?;
    Response::parse(&resp_bytes)
}

/// Forward a single HTTPS/1.1 request to `host:port` over TLS and return the
/// response. Requires the `http-client` or `http2` feature (both bring in
/// `rustls` + `webpki-roots`).
#[cfg(any(feature = "http-client", feature = "http2"))]
pub(crate) fn proxy_https1(
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    use rustls::pki_types::ServerName;
    use rustls::ClientConfig;
    use std::net::ToSocketAddrs;
    use std::sync::Arc;

    let addr_str = format!("{}:{}", host, port);
    let sock_addr = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS lookup for {} failed: {}", addr_str, e))?
        .next()
        .ok_or_else(|| format!("no address resolved for {}", addr_str))?;

    let stream = TcpStream::connect_timeout(&sock_addr, connect_timeout)
        .map_err(|e| format!("connect to {} failed: {}", addr_str, e))?;
    stream.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;

    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("invalid upstream hostname '{}': {}", host, e))?;
    let conn = rustls::ClientConnection::new(config, server_name).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, stream);

    let req_bytes = build_request(request, host, client_ip, false);
    tls.write_all(&req_bytes)
        .map_err(|e| format!("write to upstream failed: {}", e))?;

    let resp_bytes = read_response_from(&mut tls)?;
    Response::parse(&resp_bytes)
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

// ── Streaming proxy helpers ───────────────────────────────────────────────────

/// Responses larger than this threshold are streamed instead of buffered.
const STREAM_THRESHOLD: usize = 1024 * 1024; // 1 MB

/// A `Read` implementation that drains a prefix buffer before reading from the
/// inner stream. Used to replay body bytes that arrived with the HTTP headers.
pub(crate) struct ConcatReader<R: Read + Send> {
    prefix: Vec<u8>,
    prefix_pos: usize,
    inner: R,
}

impl<R: Read + Send> ConcatReader<R> {
    fn new(prefix: Vec<u8>, inner: R) -> Self {
        ConcatReader { prefix, prefix_pos: 0, inner }
    }
}

impl<R: Read + Send> Read for ConcatReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.prefix_pos < self.prefix.len() {
            let avail = &self.prefix[self.prefix_pos..];
            let n = buf.len().min(avail.len());
            buf[..n].copy_from_slice(&avail[..n]);
            self.prefix_pos += n;
            return Ok(n);
        }
        self.inner.read(buf)
    }
}

/// Read exactly the HTTP response headers (up to and including `\r\n\r\n`).
///
/// Returns `(header_bytes, body_prefix)` where `body_prefix` contains any
/// body bytes that arrived in the same TCP segment as the headers.
fn read_headers_only(stream: &mut TcpStream, tmp: &mut [u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    loop {
        let n = stream.read(tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("backend closed connection before headers were complete".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let body_prefix = buf[pos + 4..].to_vec();
            buf.truncate(pos + 4);
            return Ok((buf, body_prefix));
        }
    }
}

/// Returns `true` when the response should be streamed rather than buffered.
///
/// Streams when any of the following hold:
/// - `Content-Type: text/event-stream` — SSE
/// - `Transfer-Encoding: chunked` — AI token streams, etc.
/// - `Content-Length` exceeds 1 MB — large file downloads
pub(crate) fn should_stream_response(header_lower: &str) -> bool {
    let is_sse = header_lower.lines().any(|l| {
        l.starts_with("content-type:") && l.contains("text/event-stream")
    });
    let is_chunked = header_lower.lines().any(|l| {
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let content_length: Option<usize> = header_lower.lines().find_map(|l| {
        l.strip_prefix("content-length:")?.trim().parse().ok()
    });
    let is_large = content_length.map_or(false, |n| n > STREAM_THRESHOLD);
    is_sse || is_chunked || is_large
}

/// Parse the status line and headers from raw header bytes (ending at `\r\n\r\n`).
fn parse_status_and_headers(header_bytes: &[u8]) -> Result<Response, String> {
    let s = std::str::from_utf8(header_bytes)
        .map_err(|e| format!("non-UTF-8 response headers: {}", e))?;
    let mut lines = s.lines();
    let status_line = lines.next().ok_or("empty backend response")?;
    let mut parts = status_line.splitn(3, ' ');
    let http_version = parts.next().unwrap_or("HTTP/1.1").to_string();
    let status_code: i16 = parts
        .next()
        .unwrap_or("502")
        .parse()
        .map_err(|_| format!("invalid status code in '{}'", status_line))?;
    let reason_phrase = parts.next().unwrap_or("").trim_end_matches('\r').to_string();
    let mut headers = Vec::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() { break; }
        if let Some(colon) = line.find(':') {
            headers.push(crate::header::Header {
                name: line[..colon].trim().to_string(),
                value: line[colon + 1..].trim().to_string(),
            });
        }
    }
    Ok(Response {
        http_version,
        status_code,
        reason_phrase,
        headers,
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
    })
}

/// Read the remaining body after headers have already been read.
///
/// `header_bytes` ends with `\r\n\r\n`; `body_prefix` holds any body bytes
/// that arrived in the same TCP read.  Handles all three body mechanisms
/// (chunked, content-length, read-to-EOF).  Returns `(full_response_bytes, can_reuse)`.
fn read_response_from_partial(
    stream: &mut TcpStream,
    header_bytes: Vec<u8>,
    body_prefix: Vec<u8>,
    tmp: &mut [u8],
) -> Result<(Vec<u8>, bool), String> {
    let header_end = header_bytes.len();
    let mut buf = header_bytes;
    buf.extend_from_slice(&body_prefix);

    let header_lower =
        std::str::from_utf8(&buf[..header_end]).unwrap_or("").to_ascii_lowercase();
    let connection_close =
        header_lower.lines().any(|l| l.starts_with("connection:") && l.contains("close"));
    let is_chunked = header_lower
        .lines()
        .any(|l| l.starts_with("transfer-encoding:") && l.contains("chunked"));
    let content_length: Option<usize> = header_lower.lines().find_map(|l| {
        l.strip_prefix("content-length:")?.trim().parse().ok()
    });

    if is_chunked {
        let decoded = decode_chunked(stream, &buf, header_end, tmp)?;
        rewrite_as_content_length(&mut buf, header_end, &decoded);
        Ok((buf, !connection_close))
    } else if let Some(len) = content_length {
        while buf.len() < header_end + len {
            let n = stream.read(tmp).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            buf.extend_from_slice(&tmp[..n]);
        }
        Ok((buf, !connection_close))
    } else {
        loop {
            let n = stream.read(tmp).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            buf.extend_from_slice(&tmp[..n]);
        }
        Ok((buf, false))
    }
}

// ── Backend URL parsing ───────────────────────────────────────────────────────

struct Backend {
    host: String,
    port: u16,
    /// Whether the upstream connection should use TLS.
    /// Set when the URL scheme is `https://`, `h2s://`, or `grpcs://`.
    #[cfg_attr(not(feature = "http2"), allow(dead_code))]
    tls: bool,
}

impl Backend {
    fn parse(url: &str) -> Option<Self> {
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
        // Drop any path component.
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
        if host.is_empty() {
            return None;
        }
        Some(Backend { host, port, tls })
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
/// as-is because gRPC is layered directly on HTTP/2.
#[cfg(feature = "http2")]
pub struct H2ReverseProxy {
    inner: ReverseProxy,
}

#[cfg(feature = "http2")]
impl H2ReverseProxy {
    /// Create a proxy distributing requests across `backends` in round-robin order.
    ///
    /// Each backend entry can be:
    /// - `"host:port"` — plain TCP (HTTP/2 cleartext)
    /// - `"h2://host:port"` — plain TCP (explicit scheme)
    /// - `"h2s://host:port"` — TLS (HTTP/2 over HTTPS; port defaults to 443)
    /// - `"https://host:port"` — TLS (same as `h2s://`)
    ///
    /// TLS backends require the `http2` Cargo feature (includes `rustls` +
    /// `webpki-roots`).  Certificate verification uses the WebPKI trust store.
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
    let addr = format!("{}:{}", backend.host, backend.port);
    let tcp = tokio::time::timeout(
        connect_timeout,
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("h2 proxy: connect to {} timed out", addr))?
    .map_err(|e| format!("h2 proxy: connect to {} failed: {}", addr, e))?;

    if backend.tls {
        use rustls::pki_types::ServerName;
        use rustls::ClientConfig;
        use std::sync::Arc;
        use tokio_rustls::TlsConnector;

        let root_store = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        let mut config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        // Advertise h2 via ALPN so the server selects HTTP/2.
        config.alpn_protocols = vec![b"h2".to_vec()];
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::try_from(backend.host.as_str())
            .map_err(|e| format!("invalid upstream hostname '{}': {}", backend.host, e))?
            .to_owned();
        let tls_stream = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| format!("h2 proxy: TLS handshake with {} failed: {}", addr, e))?;
        send_h2_request(request, client_ip, backend, tls_stream).await
    } else {
        send_h2_request(request, client_ip, backend, tcp).await
    }
}

/// Drive the h2 client handshake + request/response over any async I/O stream.
///
/// Accepts both plain `TcpStream` and `TlsStream<TcpStream>` — anything that
/// satisfies `AsyncRead + AsyncWrite + Unpin + Send + 'static`.
#[cfg(feature = "http2")]
async fn send_h2_request<T>(
    request: &Request,
    client_ip: &str,
    backend: &Backend,
    stream: T,
) -> Result<Response, String>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use bytes::Bytes;
    use http as hc;

    let addr = format!("{}:{}", backend.host, backend.port);

    let (send_req, conn) = h2::client::handshake(stream)
        .await
        .map_err(|e| format!("h2 proxy: handshake with {} failed: {}", addr, e))?;

    tokio::spawn(async move {
        let _ = conn.await;
    });

    let scheme = if backend.tls { "https" } else { "http" };
    let uri_str = format!("{}://{}{}", scheme, addr, request.request_uri);
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

    const H2_HOP: &[&str] = &[
        "connection", "keep-alive", "transfer-encoding", "upgrade", "proxy-connection", "te",
    ];
    for (name, value) in &parts.headers {
        let lower = name.as_str().to_lowercase();
        if H2_HOP.contains(&lower.as_str()) {
            continue;
        }
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
    ///
    /// Each backend entry can be:
    /// - `"host:port"` — plain TCP (gRPC cleartext)
    /// - `"grpc://host:port"` — plain TCP (explicit scheme)
    /// - `"grpcs://host:port"` — TLS (gRPC over TLS; port defaults to 443)
    /// - `"https://host:port"` — TLS (same as `grpcs://`)
    ///
    /// TLS backends require the `http2` Cargo feature.
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

// ── Backend::parse unit tests ─────────────────────────────────────────────────

#[cfg(test)]
mod backend_parse_tests {
    use super::Backend;

    fn parse(url: &str) -> Option<(String, u16, bool)> {
        Backend::parse(url).map(|b| (b.host, b.port, b.tls))
    }

    #[test]
    fn bare_host_port() {
        assert_eq!(Some(("api.example.com".into(), 8080, false)), parse("api.example.com:8080"));
    }

    #[test]
    fn http_scheme() {
        assert_eq!(Some(("backend".into(), 3000, false)), parse("http://backend:3000"));
    }

    #[test]
    fn h2_scheme_plain() {
        assert_eq!(Some(("svc".into(), 50051, false)), parse("h2://svc:50051"));
    }

    #[test]
    fn grpc_scheme_plain() {
        assert_eq!(Some(("svc".into(), 50051, false)), parse("grpc://svc:50051"));
    }

    #[test]
    fn https_scheme_sets_tls_and_default_port() {
        assert_eq!(Some(("api.example.com".into(), 443, true)), parse("https://api.example.com"));
    }

    #[test]
    fn https_scheme_explicit_port() {
        assert_eq!(Some(("api.example.com".into(), 8443, true)), parse("https://api.example.com:8443"));
    }

    #[test]
    fn h2s_scheme_sets_tls() {
        assert_eq!(Some(("svc".into(), 443, true)), parse("h2s://svc"));
    }

    #[test]
    fn h2s_scheme_explicit_port() {
        assert_eq!(Some(("svc".into(), 8443, true)), parse("h2s://svc:8443"));
    }

    #[test]
    fn grpcs_scheme_sets_tls() {
        assert_eq!(Some(("grpc-svc".into(), 443, true)), parse("grpcs://grpc-svc"));
    }

    #[test]
    fn grpcs_scheme_explicit_port() {
        assert_eq!(Some(("grpc-svc".into(), 50052, true)), parse("grpcs://grpc-svc:50052"));
    }

    #[test]
    fn empty_host_returns_none() {
        assert_eq!(None, parse("https://"));
    }

    #[test]
    fn bare_host_no_port_defaults_to_80() {
        assert_eq!(Some(("myhost".into(), 80, false)), parse("myhost"));
    }
}
