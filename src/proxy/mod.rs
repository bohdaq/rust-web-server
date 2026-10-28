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
#[cfg(feature = "http2")]
pub(crate) mod async_upstream;
#[cfg(feature = "http2")]
pub(crate) mod h2_upstream;
#[cfg(feature = "http2")]
pub mod grpc_web;
#[cfg(feature = "http2")]
pub use grpc_web::GrpcWebLayer;

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use crate::ext::decimal::Decimal;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use pool::{ConnPool, Pool};

use crate::application::Application;
use crate::core::New;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

// Hop-by-hop headers that must not be forwarded (RFC 9110 §7.6.1), plus
// `proxy-connection` (legacy clients). Headers named in `Connection` are
// dropped too (`connection_listed`).
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
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
/// 64 idle per backend, 60-second timeout).
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
    breaker: Option<Arc<dyn crate::circuit_breaker::Breaker>>,
    trust: Option<Arc<UpstreamTls>>,
    /// What `trust` was built from, so each TLS option adds to the others.
    tls_ca: Option<String>,
    tls_client: Option<(String, String)>,
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
            pool: Arc::new(ConnPool::new(POOL_MAX_IDLE, Duration::from_secs(60))),
            breaker: None,
            trust: None,
            tls_ca: None,
            tls_client: None,
        }
    }

    /// Wire a [`crate::circuit_breaker::CircuitBreaker`] or
    /// [`crate::circuit_breaker::RedisCircuitBreaker`] into this proxy:
    /// before dialing a backend, its breaker state is checked (an `Open`
    /// backend is skipped with no TCP attempt, exactly like a caller
    /// manually checking `is_available` around each proxied call); after a
    /// dial, `record_success`/`record_failure` is called automatically.
    /// No breaker is wired by default — existing `ReverseProxy` callers see
    /// no behavior change unless they opt in here.
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use rust_web_server::circuit_breaker::global as global_breaker;
    /// use rust_web_server::proxy::ReverseProxy;
    ///
    /// let proxy = ReverseProxy::new(["http://backend-1:8080", "http://backend-2:8080"])
    ///     .with_circuit_breaker(Arc::new(global_breaker()));
    /// ```
    pub fn with_circuit_breaker(mut self, breaker: Arc<dyn crate::circuit_breaker::Breaker>) -> Self {
        self.breaker = Some(breaker);
        self
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

    /// Trust the CA certificates in the PEM file `path` for `https://`
    /// backends, **instead of** the public WebPKI roots — for internal
    /// services behind a private CA or a self-signed certificate.
    pub fn tls_ca_file(mut self, path: &str) -> Result<Self, String> {
        self.tls_ca = Some(path.to_string());
        self.rebuild_trust()?;
        Ok(self)
    }

    /// Present the client certificate in the PEM file `cert` (with its
    /// private key in `key`) to `https://` backends that require one —
    /// mutual TLS to the upstream. Since 18.3.0.
    pub fn tls_client_cert(mut self, cert: &str, key: &str) -> Result<Self, String> {
        self.tls_client = Some((cert.to_string(), key.to_string()));
        self.rebuild_trust()?;
        Ok(self)
    }

    fn rebuild_trust(&mut self) -> Result<(), String> {
        let client = self.tls_client.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));
        self.trust = Some(Arc::new(UpstreamTls::new(self.tls_ca.as_deref(), client)?));
        Ok(())
    }

    /// Set the maximum number of idle connections per backend (default: 64).
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
        let mut last_err = "all backends failed".to_string();
        for attempt in 0..n {
            let idx = (start + attempt) % n;
            let backend = &self.backends[idx];
            let key = format!("{}:{}", backend.host, backend.port);

            if let Some(breaker) = &self.breaker {
                if !breaker.is_available(&key) {
                    last_err = format!("circuit open for {}", key);
                    continue;
                }
            }

            match self.try_backend(request, connection, backend) {
                Ok(resp) => {
                    if let Some(breaker) = &self.breaker {
                        breaker.record_success(&key);
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if let Some(breaker) = &self.breaker {
                        breaker.record_failure(&key);
                    }
                    last_err = e;
                }
            }
        }
        Err(last_err)
    }

    fn try_backend(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        backend: &Backend,
    ) -> Result<Response, String> {
        forward_upstream_trusting(
            &self.pool,
            request,
            &connection.client.ip,
            &backend.host,
            backend.port,
            backend.tls,
            self.trust.as_deref(),
            self.connect_timeout,
            self.read_timeout,
        )
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
    // One buffer, sized up front and filled with extend_from_slice — no
    // write!/format machinery, no lower-cased copy of each header name.
    // `Content-Length` is regenerated from the (possibly de-chunked) body and
    // `X-Forwarded-For` is extended below, so the client's copies are dropped:
    // two `Content-Length` lines are a protocol error and a desync risk.
    let connection = request.header(crate::header::Header::_CONNECTION).map(|h| h.value.as_ref());
    // Inside a traced request the current span's `traceparent` (and the
    // request's `baggage`) replace the client's.
    #[cfg(not(target_arch = "wasm32"))]
    let trace = crate::otel::propagation_headers();
    #[cfg(target_arch = "wasm32")]
    let trace: Option<(String, Option<String>)> = None;
    let forwarded = |h: &&crate::header::Header| {
        // An empty name would be sent as a malformed ": value" line.
        !h.name.is_empty()
            && !is_hop_by_hop_or_host(&h.name)
            && !h.name.eq_ignore_ascii_case(crate::header::Header::_CONTENT_LENGTH)
            && !h.name.eq_ignore_ascii_case("X-Forwarded-For")
            && !connection.is_some_and(|c| connection_listed(c, &h.name))
            && !(trace.is_some() && (h.name.eq_ignore_ascii_case("traceparent") || h.name.eq_ignore_ascii_case("baggage")))
    };
    let prior_xff = request.header("X-Forwarded-For").map(|h| h.value.as_ref()).filter(|v| !v.trim().is_empty());
    let body_len = Decimal::new(request.body.len() as u128);
    let capacity = request.method.len() + request.request_uri.len() + backend_host.len() + 25
        + request.headers.iter().filter(forwarded).map(|h| h.name.len() + h.value.len() + 4).sum::<usize>()
        + client_ip.len() + 19 + prior_xff.map_or(0, |v| v.len() + 2)
        + trace.as_ref().map_or(0, |(t, b)| t.len() + 15 + b.as_ref().map_or(0, |b| b.len() + 11))
        + 13
        + 24
        + if request.body.is_empty() { 0 } else { body_len.as_str().len() + 18 }
        + 2
        + request.body.len();
    let mut out: Vec<u8> = Vec::with_capacity(capacity);
    for part in [request.method.as_str(), " ", request.request_uri.as_str(), " HTTP/1.1\r\nHost: ", backend_host, "\r\n"] {
        out.extend_from_slice(part.as_bytes());
    }
    for h in request.headers.iter().filter(forwarded) {
        for part in [h.name.as_ref(), ": ", h.value.as_ref(), "\r\n"] {
            out.extend_from_slice(part.as_bytes());
        }
    }
    out.extend_from_slice(b"X-Forwarded-For: ");
    if let Some(prior) = prior_xff {
        out.extend_from_slice(prior.as_bytes());
        out.extend_from_slice(b", ");
    }
    for part in [client_ip, "\r\n", "Via: 1.1 rws\r\n"] {
        out.extend_from_slice(part.as_bytes());
    }
    if let Some((traceparent, baggage)) = &trace {
        for part in ["traceparent: ", traceparent, "\r\n"] {
            out.extend_from_slice(part.as_bytes());
        }
        if let Some(baggage) = baggage {
            for part in ["baggage: ", baggage, "\r\n"] {
                out.extend_from_slice(part.as_bytes());
            }
        }
    }
    if keep_alive {
        out.extend_from_slice(b"Connection: keep-alive\r\n");
    } else {
        out.extend_from_slice(b"Connection: close\r\n");
    }
    if !request.body.is_empty() {
        out.extend_from_slice(b"Content-Length: ");
        out.extend_from_slice(body_len.as_str().as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&request.body);
    out
}

/// Whether a request header is dropped on the way upstream: hop-by-hop
/// headers and `Host` (rws sends its own). Matches `to_lowercase()` against
/// the lower-case list — Unicode lower-casing, so a non-ASCII name (where
/// e.g. the Kelvin sign lower-cases to `k`) still takes that path.
fn is_hop_by_hop_or_host(name: &str) -> bool {
    if name.is_ascii() {
        name.eq_ignore_ascii_case("host") || HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
    } else {
        let lower = name.to_lowercase();
        lower == "host" || HOP_BY_HOP.contains(&lower.as_str())
    }
}

/// Whether `name` is one of the comma-separated tokens of a `Connection`
/// header value — those headers are hop-by-hop for this connection only.
fn connection_listed(connection: &str, name: &str) -> bool {
    connection.split(',').any(|token| token.trim().eq_ignore_ascii_case(name))
}

/// Decode HTTP/1.1 chunked transfer-encoding from `stream`.
///
/// `buf[header_end..]` may already contain some body bytes that arrived in
/// the same read as the headers.  Returns the fully decoded body.
fn decode_chunked<S: Read>(
    stream: &mut S,
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
            // Last chunk: skip trailer fields up to the empty line that ends
            // the message. Leaving any of it unread would corrupt the next
            // response on a pooled connection.
            loop {
                let line_end = loop {
                    if let Some(p) = raw.windows(2).position(|w| w == b"\r\n") {
                        break p;
                    }
                    let n = stream.read(tmp).map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Err("chunked: premature EOF in trailer".to_string());
                    }
                    raw.extend_from_slice(&tmp[..n]);
                };
                raw.drain(..line_end + 2);
                if line_end == 0 {
                    break;
                }
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

/// Headers rws regenerates for a buffered body, plus hop-by-hop headers
/// (RFC 9110 §7.6.1) that must not be forwarded.
pub(crate) const REGENERATED_OR_HOP_BY_HOP: &[&str] = &[
    "content-length", "transfer-encoding", "connection", "keep-alive",
    "proxy-connection", "te", "trailer", "upgrade",
];

/// Make a buffered upstream response safe to re-serialize. The body is
/// already held in `content_range_list`, from which `Response::generate`
/// (and the HTTP/2 / HTTP/3 writers) emit `Content-Type` and
/// `Content-Length` themselves, so the upstream's own copies would be
/// duplicated — and a duplicate `content-length` is a protocol error that
/// HTTP/2 clients reject outright. The upstream's `Content-Type` is moved
/// onto the body instead; the status line is re-emitted as HTTP/1.1
/// regardless of the backend's version.
#[cfg_attr(not(feature = "http2"), allow(dead_code))]
pub(crate) fn normalize_upstream_response(mut response: Response) -> Response {
    response.http_version.clear();
    response.http_version.push_str(crate::http::VERSION.http_1_1);
    let has_body = !response.content_range_list.is_empty();
    if let [range] = response.content_range_list.as_mut_slice() {
        if let Some(h) = response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(crate::header::Header::_CONTENT_TYPE))
        {
            range.content_type.clear();
            range.content_type.push_str(&h.value);
        }
    }
    // Compared in place (the list is lower-case ASCII), not lower-cased
    // into a new String per header.
    response.headers.retain(|h| {
        if h.name.eq_ignore_ascii_case("content-type") {
            // Keep it only when there's no body to carry it (e.g. HEAD, 304).
            return !has_body;
        }
        !REGENERATED_OR_HOP_BY_HOP.iter().any(|name| h.name.eq_ignore_ascii_case(name))
    });
    response
}

pub(crate) fn bad_gateway() -> Response {
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


/// Idle backend connections kept per `host:port`. 64 matches common nginx
/// `keepalive` settings and the benchmark's concurrency; fewer meant most
/// requests at 64-way concurrency still paid a TCP connect.
pub(crate) const POOL_MAX_IDLE: usize = 64;

/// The process-wide backend connection pool used by config-driven proxy
/// routes (`proxy_config::DynamicProxy`), keyed by `host:port`.
pub(crate) fn shared_pool() -> &'static ConnPool {
    static POOL: std::sync::OnceLock<ConnPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| ConnPool::new(POOL_MAX_IDLE, Duration::from_secs(60)))
}

/// A backend connection the pooled forwarder drives: plain TCP, or TLS
/// over TCP for `https://` backends.
pub(crate) trait UpstreamIo: Read + Write + Send + 'static {
    fn set_io_timeouts(&self, read: Duration, write: Duration) -> std::io::Result<()>;
}

impl UpstreamIo for TcpStream {
    fn set_io_timeouts(&self, read: Duration, write: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(read))?;
        self.set_write_timeout(Some(write))
    }
}

#[cfg(any(feature = "http-client", feature = "http2"))]
pub(crate) type TlsStream = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

#[cfg(any(feature = "http-client", feature = "http2"))]
impl UpstreamIo for TlsStream {
    fn set_io_timeouts(&self, read: Duration, write: Duration) -> std::io::Result<()> {
        self.sock.set_io_timeouts(read, write)
    }
}

/// Forward `request` to an HTTP or HTTPS backend at `host:port`, over a
/// pooled keep-alive connection either way: plain backends use `plain_pool`,
/// TLS backends the process-wide TLS pool (one handshake per connection, not
/// per request). The single entry point for `ReverseProxy`, config-driven
/// routes, `canary` and `ingress`.
pub(crate) fn forward_upstream(
    plain_pool: &ConnPool,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    tls: bool,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    forward_upstream_trusting(plain_pool, request, client_ip, host, port, tls, None, connect_timeout, read_timeout)
}

/// [`forward_upstream`] with the trust settings of one upstream: `trust`
/// replaces the WebPKI roots for TLS backends (a private CA).
#[allow(clippy::too_many_arguments)]
pub(crate) fn forward_upstream_trusting(
    plain_pool: &ConnPool,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    tls: bool,
    trust: Option<&UpstreamTls>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    // Inside a traced request: a client span per forward, current while the
    // request is built, so the upstream's `traceparent` names it.
    #[cfg(not(target_arch = "wasm32"))]
    let span = crate::otel::outbound_span(&request.method);
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(span) = &span {
        let scheme = if tls { "https" } else { "http" };
        let path = request.request_uri.as_str();
        for (key, value) in crate::otel::http_client_attributes(&request.method, &format!("{scheme}://{host}:{port}{path}")) {
            span.set_attribute(key, value);
        }
    }
    let result = forward_upstream_untraced(plain_pool, request, client_ip, host, port, tls, trust, connect_timeout, read_timeout);
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(span) = &span {
        span.finish_http_client(result.as_ref().map(|r| r.status_code as u16).map_err(String::as_str));
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn forward_upstream_untraced(
    plain_pool: &ConnPool,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    tls: bool,
    trust: Option<&UpstreamTls>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    if tls {
        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            let (pool, config) = match trust {
                Some(t) => (t.pool, Arc::clone(&t.config)),
                None => (shared_tls_pool(), tls_client_config()),
            };
            return forward_via(pool, request, client_ip, host, port, host, read_timeout, || {
                connect_tls_with(Arc::clone(&config), host, port, connect_timeout)
            });
        }
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            let _ = trust;
            return Err("HTTPS backends need the http2 or http-client feature, which this build doesn't include".to_string());
        }
    }
    forward_pooled(plain_pool, request, client_ip, host, port, connect_timeout, read_timeout)
}

/// Trust settings for one upstream's TLS backends: the certificates in a CA
/// file **instead of** the WebPKI roots — for internal services behind a
/// private CA or a self-signed certificate (nginx's
/// `proxy_ssl_trusted_certificate`) — and/or a client certificate rws
/// presents to backends that require one (`proxy_ssl_certificate`, since
/// 18.3.0). Connections are pooled per setting, so two upstreams at the same
/// address with different trust never share one.
pub struct UpstreamTls {
    #[cfg(any(feature = "http-client", feature = "http2"))]
    config: Arc<rustls::ClientConfig>,
    #[cfg(any(feature = "http-client", feature = "http2"))]
    pool: &'static Pool<TlsStream>,
    /// Connections for the non-blocking forwarder, made with `config`.
    #[cfg(feature = "http2")]
    async_pool: &'static Pool<async_upstream::AsyncTlsStream>,
    /// `config` offering HTTP/2 by ALPN, for HTTP/2 and gRPC upstreams.
    #[cfg(feature = "http2")]
    h2_config: Arc<rustls::ClientConfig>,
}

impl UpstreamTls {
    /// Load the PEM certificates in `path` as the only trusted roots.
    pub fn from_ca_file(path: &str) -> Result<UpstreamTls, String> {
        UpstreamTls::new(Some(path), None)
    }

    /// Trust the PEM certificates in `ca_file` (the WebPKI roots when `None`)
    /// and, with `client_cert` = `(certificate chain PEM, private key PEM)`,
    /// present that certificate to backends that ask for one.
    pub fn new(ca_file: Option<&str>, client_cert: Option<(&str, &str)>) -> Result<UpstreamTls, String> {
        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
            let roots = match ca_file {
                Some(path) => {
                    let bytes = std::fs::read(path).map_err(|e| format!("cannot read CA file '{path}': {e}"))?;
                    let certs = CertificateDer::pem_slice_iter(&bytes)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("cannot parse CA file '{path}': {e}"))?;
                    if certs.is_empty() {
                        return Err(format!("no certificates in CA file '{path}'"));
                    }
                    let mut roots = rustls::RootCertStore::empty();
                    for cert in certs {
                        roots.add(cert).map_err(|e| format!("invalid CA certificate in '{path}': {e}"))?;
                    }
                    roots
                }
                None => rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
            };
            let builder = crate::tls_client_builder().with_root_certificates(roots);
            let config = match client_cert {
                None => builder.with_no_client_auth(),
                Some((cert_path, key_path)) => {
                    let bytes = std::fs::read(cert_path).map_err(|e| format!("cannot read client certificate '{cert_path}': {e}"))?;
                    let chain = CertificateDer::pem_slice_iter(&bytes)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("cannot parse client certificate '{cert_path}': {e}"))?;
                    if chain.is_empty() {
                        return Err(format!("no certificate in '{cert_path}'"));
                    }
                    let bytes = std::fs::read(key_path).map_err(|e| format!("cannot read client key '{key_path}': {e}"))?;
                    let key = PrivateKeyDer::from_pem_slice(&bytes).map_err(|e| format!("cannot parse client key '{key_path}': {e}"))?;
                    builder.with_client_auth_cert(chain, key).map_err(|e| format!("client certificate '{cert_path}': {e}"))?
                }
            };
            let label = format!("{}|{}", ca_file.unwrap_or("webpki"), client_cert.map_or("", |(cert, _)| cert));
            Ok(UpstreamTls::with_config(config, &label))
        }
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            let _ = client_cert;
            Err(format!("tls_ca_file '{}': HTTPS backends need the http2 or http-client feature", ca_file.unwrap_or("")))
        }
    }

    #[cfg(any(feature = "http-client", feature = "http2"))]
    fn with_config(config: rustls::ClientConfig, label: &str) -> UpstreamTls {
        #[cfg(feature = "http2")]
        let h2_config = Arc::new(h2_upstream::with_h2_alpn(config.clone()));
        UpstreamTls {
            config: Arc::new(config),
            pool: tls_pool_for_ca(label),
            #[cfg(feature = "http2")]
            async_pool: async_tls_pool_for_ca(label),
            #[cfg(feature = "http2")]
            h2_config,
        }
    }

    /// The client configuration for HTTP/2 upstreams (ALPN `h2`).
    #[cfg(feature = "http2")]
    pub(crate) fn h2_config(&self) -> &Arc<rustls::ClientConfig> {
        &self.h2_config
    }

    /// Trust nothing: every TLS handshake fails, so the upstream answers 502.
    /// Used when a configured CA file can't be loaded — failing closed rather
    /// than falling back to the public roots.
    pub(crate) fn reject_all(label: &str) -> UpstreamTls {
        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            let config = crate::tls_client_builder().with_root_certificates(rustls::RootCertStore::empty()).with_no_client_auth();
            UpstreamTls::with_config(config, &format!("reject-all:{label}"))
        }
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            let _ = label;
            UpstreamTls {}
        }
    }

    /// How the non-blocking forwarder reaches these backends.
    #[cfg(feature = "http2")]
    pub(crate) fn async_tls(&self) -> async_upstream::AsyncTls {
        async_upstream::AsyncTls { config: Arc::clone(&self.config), pool: self.async_pool }
    }

    /// The client configuration (for health checks).
    #[cfg(any(feature = "http-client", feature = "http2"))]
    pub(crate) fn client_config(&self) -> Arc<rustls::ClientConfig> {
        Arc::clone(&self.config)
    }
}

/// One non-blocking connection pool per CA file, for the life of the process
/// (a reload that keeps the same file keeps its warm connections).
#[cfg(feature = "http2")]
fn async_tls_pool_for_ca(path: &str) -> &'static Pool<async_upstream::AsyncTlsStream> {
    static POOLS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, &'static Pool<async_upstream::AsyncTlsStream>>>,
    > = std::sync::OnceLock::new();
    let mut pools = POOLS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    pools
        .entry(path.to_string())
        .or_insert_with(|| Box::leak(Box::new(Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))))
}

/// One pool per CA file, for the life of the process.
#[cfg(any(feature = "http-client", feature = "http2"))]
fn tls_pool_for_ca(path: &str) -> &'static Pool<TlsStream> {
    static POOLS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, &'static Pool<TlsStream>>>> =
        std::sync::OnceLock::new();
    let mut pools = POOLS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    pools
        .entry(path.to_string())
        .or_insert_with(|| Box::leak(Box::new(Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))))
}

/// Forward `request` to `host:port` over plain HTTP/1.1, reusing an idle
/// connection from `pool` when there is one and returning the connection to
/// `pool` after a complete, reusable response.
pub(crate) fn forward_pooled(
    pool: &ConnPool,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    forward_via(pool, request, client_ip, host, port, host, read_timeout, || connect_tcp(host, port, connect_timeout))
}

/// [`forward_pooled`], sending `host_header` as the `Host` (the client's, for
/// a gateway that must preserve it) instead of the backend's address.
#[cfg(feature = "gateway-api")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn forward_pooled_with_host(
    pool: &ConnPool,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    host_header: &str,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    forward_via(pool, request, client_ip, host, port, host_header, read_timeout, || connect_tcp(host, port, connect_timeout))
}

/// The pooled request/response cycle, for any connection type.
///
/// A backend may close an idle keep-alive connection at any moment, so a
/// pooled connection that fails before *any* response byte arrives is
/// retried once on a fresh connection — but only for idempotent methods
/// (RFC 9110 §9.2.2): the request may already have reached the backend.
#[allow(clippy::too_many_arguments)]
fn forward_via<S: UpstreamIo>(
    pool: &Pool<S>,
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    host_header: &str,
    read_timeout: Duration,
    connect: impl Fn() -> Result<S, String>,
) -> Result<Response, String> {
    let key = format!("{}:{}", host, port);
    if let Some(stream) = pool.acquire(&key) {
        match exchange(pool, &key, stream, request, host_header, client_ip, read_timeout) {
            Exchange::Done(response) => return Ok(response),
            Exchange::NoResponse(_) if is_idempotent(&request.method) => {} // stale: retry fresh
            Exchange::NoResponse(e) | Exchange::Failed(e) => return Err(e),
        }
    }
    let stream = connect()?;
    match exchange(pool, &key, stream, request, host_header, client_ip, read_timeout) {
        Exchange::Done(response) => Ok(response),
        Exchange::NoResponse(e) | Exchange::Failed(e) => Err(e),
    }
}

/// Connect to the first address of `host:port` that accepts. `localhost`
/// resolves to `::1` before `127.0.0.1` on many systems; a backend listening
/// on IPv4 only must still be reachable by name.
fn connect_tcp(host: &str, port: u16, connect_timeout: Duration) -> Result<TcpStream, String> {
    let key = format!("{}:{}", host, port);
    let addrs: Vec<_> = key
        .to_socket_addrs()
        .map_err(|e| format!("DNS lookup for {} failed: {}", key, e))?
        .collect();
    let mut last_error = format!("no address resolved for {}", key);
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, connect_timeout) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(e) => last_error = format!("connect to {} ({}) failed: {}", key, addr, e),
        }
    }
    Err(last_error)
}

/// The process-wide pool of TLS connections to `https://` backends.
#[cfg(any(feature = "http-client", feature = "http2"))]
fn shared_tls_pool() -> &'static Pool<TlsStream> {
    static POOL: std::sync::OnceLock<Pool<TlsStream>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))
}

/// TLS client settings for backends: the Mozilla root store, no client
/// certificate. Built once — parsing the root store per request was a large
/// part of the per-request cost of HTTPS upstreams.
#[cfg(any(feature = "http-client", feature = "http2"))]
pub(crate) fn tls_client_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: std::sync::OnceLock<Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    Arc::clone(CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth())
    }))
}

#[cfg(any(feature = "http-client", feature = "http2"))]
fn connect_tls_with(config: Arc<rustls::ClientConfig>, host: &str, port: u16, connect_timeout: Duration) -> Result<TlsStream, String> {
    let tcp = connect_tcp(host, port, connect_timeout)?;
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("invalid upstream hostname '{}': {}", host, e))?;
    let conn = rustls::ClientConnection::new(config, server_name).map_err(|e| e.to_string())?;
    Ok(rustls::StreamOwned::new(conn, tcp))
}

fn is_idempotent(method: &str) -> bool {
    matches!(method, "GET" | "HEAD" | "OPTIONS" | "TRACE" | "PUT" | "DELETE")
}

enum Exchange {
    Done(Response),
    /// Failed before any response byte (write error, or closed/errored on the first read).
    NoResponse(String),
    Failed(String),
}

/// One request/response on `stream`. A fully read, keep-alive response puts
/// the connection back in `pool`; a streamed response keeps it until done.
fn exchange<S: UpstreamIo>(
    pool: &Pool<S>,
    key: &str,
    mut stream: S,
    request: &Request,
    host: &str,
    client_ip: &str,
    read_timeout: Duration,
) -> Exchange {
    if let Err(e) = stream.set_io_timeouts(read_timeout, Duration::from_secs(10)) {
        return Exchange::Failed(e.to_string());
    }
    // keep_alive = true: ask the backend to hold the connection open.
    let req_bytes = build_request(request, host, client_ip, true);
    if let Err(e) = stream.write_all(&req_bytes) {
        return Exchange::NoResponse(format!("write to backend failed: {}", e));
    }

    let mut tmp = [0u8; 4096];
    let (header_bytes, body_prefix) = match read_final_response_head(&mut stream, &mut tmp) {
        Ok(head) => head,
        Err(HeadError::NothingReceived(e)) => return Exchange::NoResponse(e),
        Err(HeadError::Partial(e)) => return Exchange::Failed(e),
    };
    let header_lower = std::str::from_utf8(&header_bytes).unwrap_or("").to_ascii_lowercase();

    if response_has_no_body(&request.method, &header_lower) {
        if !header_lower.lines().any(|l| l.starts_with("connection:") && l.contains("close")) {
            pool.release(key, stream);
        }
        return match Response::parse_upstream(&header_bytes, REGENERATED_OR_HOP_BY_HOP) {
            Ok(response) => Exchange::Done(response),
            Err(e) => Exchange::Failed(e),
        };
    }

    if should_stream_response(&header_lower) {
        // Streaming path — pipe bytes straight to the client; the connection
        // can't be reused while the body is in flight.
        return match parse_status_and_headers(&header_bytes) {
            Ok(mut response) => {
                response.stream_pipe = Some(Box::new(ConcatReader::new(body_prefix, stream)));
                Exchange::Done(response)
            }
            Err(e) => Exchange::Failed(e),
        };
    }
    // Buffered path — read the full body, then return the connection to the pool.
    match read_response_from_partial(&mut stream, header_bytes, body_prefix, &mut tmp) {
        Ok((resp_bytes, reusable)) => {
            if reusable {
                pool.release(key, stream);
            }
            match Response::parse_upstream(&resp_bytes, REGENERATED_OR_HOP_BY_HOP) {
                Ok(response) => Exchange::Done(response),
                Err(e) => Exchange::Failed(e),
            }
        }
        Err(e) => Exchange::Failed(e),
    }
}

/// Read exactly the HTTP response headers (up to and including `\r\n\r\n`).
///
/// Returns `(header_bytes, body_prefix)` where `body_prefix` contains any
/// body bytes that arrived in the same TCP segment as the headers.
#[cfg(test)]
fn read_headers_only(stream: &mut TcpStream, tmp: &mut [u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    read_response_head(stream, tmp).map_err(|e| match e {
        HeadError::NothingReceived(msg) | HeadError::Partial(msg) => msg,
    })
}

/// Why reading a response head failed: before any byte arrived (the request
/// may never have been processed — e.g. a pooled connection the backend had
/// already closed) or part-way through.
enum HeadError {
    NothingReceived(String),
    Partial(String),
}

fn read_response_head<S: Read>(stream: &mut S, tmp: &mut [u8]) -> Result<(Vec<u8>, Vec<u8>), HeadError> {
    read_response_head_from(stream, tmp, Vec::with_capacity(4096), false)
}

/// The final response head: interim `1xx` heads (`100 Continue`,
/// `103 Early Hints`, …) are read past (RFC 9110 §15.2). Returning one as
/// the response left the real response in the connection, and once pooled
/// the next request — possibly another client's — received it.
fn read_final_response_head<S: Read>(stream: &mut S, tmp: &mut [u8]) -> Result<(Vec<u8>, Vec<u8>), HeadError> {
    let (mut head, mut rest) = read_response_head(stream, tmp)?;
    let mut interim = 0;
    while is_interim_response(&head) {
        interim += 1;
        if interim > MAX_INTERIM_RESPONSES {
            return Err(HeadError::Partial("too many interim (1xx) responses from the backend".to_string()));
        }
        (head, rest) = read_response_head_from(stream, tmp, rest, true)?;
    }
    Ok((head, rest))
}

/// Read until a complete head is in `buf` (which may already hold bytes
/// that followed a previous head). `received` — bytes already came from this
/// response, so a failure is never "nothing received".
fn read_response_head_from<S: Read>(
    stream: &mut S,
    tmp: &mut [u8],
    mut buf: Vec<u8>,
    received: bool,
) -> Result<(Vec<u8>, Vec<u8>), HeadError> {
    loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let body_prefix = buf[pos + 4..].to_vec();
            buf.truncate(pos + 4);
            return Ok((buf, body_prefix));
        }
        let nothing = buf.is_empty() && !received;
        let n = match stream.read(tmp) {
            Ok(n) => n,
            Err(e) if nothing => return Err(HeadError::NothingReceived(e.to_string())),
            Err(e) => return Err(HeadError::Partial(e.to_string())),
        };
        if n == 0 {
            let msg = "backend closed connection before headers were complete".to_string();
            return Err(if nothing { HeadError::NothingReceived(msg) } else { HeadError::Partial(msg) });
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

/// Interim responses tolerated before the final one.
pub(crate) const MAX_INTERIM_RESPONSES: usize = 16;

/// An interim (1xx) response head other than `101 Switching Protocols`
/// (which ends the HTTP exchange): the final response follows it.
pub(crate) fn is_interim_response(head: &[u8]) -> bool {
    let line_end = head.iter().position(|b| *b == b'\r' || *b == b'\n').unwrap_or(head.len());
    let status = head[..line_end].split(|b| *b == b' ').nth(1).unwrap_or(&[]);
    status.len() == 3 && status.iter().all(u8::is_ascii_digit) && status[0] == b'1' && status != b"101"
}

/// RFC 9112 §6.3: a response to HEAD, and any 1xx, 204 or 304 response, ends
/// with its head — whatever `Content-Length` says (for HEAD it describes the
/// GET body). Waiting for that body hung until the read timeout (a 502).
pub(crate) fn response_has_no_body(method: &str, header_lower: &str) -> bool {
    let status = header_lower.split_whitespace().nth(1).unwrap_or("");
    method.eq_ignore_ascii_case("HEAD") || status.starts_with('1') || status == "204" || status == "304"
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

/// What the forwarders need from an upstream response head, found in one
/// case-insensitive pass over the raw bytes — the same answers as
/// [`response_has_no_body`], [`should_stream_response`] and the
/// `connection:` / `content-length:` scans over a lower-cased copy, without
/// making that copy.
#[cfg_attr(not(feature = "http2"), allow(dead_code))]
pub(crate) struct HeadFacts {
    /// The second whitespace-separated token of the head (the status code).
    pub(crate) status_no_body: bool,
    pub(crate) sse: bool,
    pub(crate) chunked: bool,
    /// The first `content-length:` value that parses.
    pub(crate) content_length: Option<usize>,
    pub(crate) connection_close: bool,
}

#[cfg_attr(not(feature = "http2"), allow(dead_code))]
impl HeadFacts {
    pub(crate) fn scan(head: &[u8]) -> HeadFacts {
        let text = std::str::from_utf8(head).unwrap_or("");
        let status = text.split_whitespace().nth(1).unwrap_or("");
        let mut facts = HeadFacts {
            status_no_body: status.starts_with('1') || status == "204" || status == "304",
            sse: false,
            chunked: false,
            content_length: None,
            connection_close: false,
        };
        for line in text.lines() {
            if let Some(value) = strip_prefix_ignore_ascii_case(line, "content-type:") {
                facts.sse |= contains_ignore_ascii_case(value, "text/event-stream");
            } else if let Some(value) = strip_prefix_ignore_ascii_case(line, "transfer-encoding:") {
                facts.chunked |= contains_ignore_ascii_case(value, "chunked");
            } else if let Some(value) = strip_prefix_ignore_ascii_case(line, "content-length:") {
                if facts.content_length.is_none() {
                    facts.content_length = value.trim().parse().ok();
                }
            } else if let Some(value) = strip_prefix_ignore_ascii_case(line, "connection:") {
                facts.connection_close |= contains_ignore_ascii_case(value, "close");
            }
        }
        facts
    }

    /// [`response_has_no_body`].
    pub(crate) fn no_body(&self, method: &str) -> bool {
        method.eq_ignore_ascii_case("HEAD") || self.status_no_body
    }

    /// [`should_stream_response`].
    pub(crate) fn should_stream(&self) -> bool {
        self.sse || self.chunked || self.content_length.is_some_and(|n| n > STREAM_THRESHOLD)
    }
}

fn strip_prefix_ignore_ascii_case<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let bytes = line.as_bytes();
    // `prefix` is ASCII, so a byte match ends on a char boundary.
    (bytes.len() >= prefix.len() && bytes[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()))
        .then(|| &line[prefix.len()..])
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack.as_bytes().windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
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
                name: line[..colon].trim().to_string().into(),
                value: (line[colon + 1..].trim().to_string()).into(),
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
        trailers: None,
    })
}

/// Read the remaining body after headers have already been read.
///
/// `header_bytes` ends with `\r\n\r\n`; `body_prefix` holds any body bytes
/// that arrived in the same TCP read.  Handles all three body mechanisms
/// (chunked, content-length, read-to-EOF).  Returns `(full_response_bytes, can_reuse)`.
fn read_response_from_partial<S: Read>(
    stream: &mut S,
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

    /// Override the read timeout (default: 30 s). It bounds each wait for
    /// the upstream — the handshake, the response head, every body frame — so
    /// a streaming response (a gRPC server stream) ends only when it goes
    /// quiet for that long.
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.inner = self.inner.read_timeout_ms(ms);
        self
    }

    /// Trust the CA certificates in the PEM file `path` for `h2s://` /
    /// `https://` backends instead of the WebPKI roots. Since 18.3.0.
    pub fn tls_ca_file(mut self, path: &str) -> Result<Self, String> {
        self.inner = self.inner.tls_ca_file(path)?;
        Ok(self)
    }

    /// Present a client certificate to TLS backends that require one (mutual
    /// TLS). Since 18.3.0.
    pub fn tls_client_cert(mut self, cert: &str, key: &str) -> Result<Self, String> {
        self.inner = self.inner.tls_client_cert(cert, key)?;
        Ok(self)
    }

    fn matches(&self, request: &Request) -> bool {
        self.inner.path_prefix.as_ref().is_none_or(|prefix| request.request_uri.starts_with(prefix.as_str()))
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
        if !self.matches(request) {
            return next.execute(request, connection);
        }
        if self.inner.backends.is_empty() {
            return Ok(bad_gateway());
        }
        // A streamed request body can be sent once: no second backend then.
        let attempts = if matches!(request.body, crate::request::Body::Stream(_)) { 1 } else { self.inner.backends.len() };
        let n = self.inner.backends.len();
        let start = self.inner.counter.fetch_add(1, Ordering::Relaxed);
        for attempt in 0..attempts {
            let backend = &self.inner.backends[(start + attempt) % n];
            let tls = backend.tls.then(|| match &self.inner.trust {
                Some(trust) => trust.h2_config(),
                None => h2_upstream::default_tls(),
            });
            let target = h2_upstream::Target {
                backend,
                tls,
                connect_timeout: self.inner.connect_timeout,
                read_timeout: self.inner.read_timeout,
                authority: None,
            };
            if let Ok(response) = h2_upstream::forward(request, &connection.client.ip, &target) {
                return Ok(response);
            }
        }
        Ok(bad_gateway())
    }

    /// Headers only: a request this proxy doesn't forward passes through
    /// untouched.
    fn accepts_streamed_body(&self, _request: &Request, _connection: &crate::server::ConnectionInfo) -> bool {
        true
    }

    /// Requests this proxy forwards are sent as their body arrives (gRPC
    /// client streaming and bidirectional calls).
    fn answers_streamed_body(&self, request: &Request, _connection: &crate::server::ConnectionInfo) -> bool {
        self.matches(request)
    }
}

/// Forward `request` over HTTP/2 to `url` (`h2://`, `grpc://`, `h2s://`,
/// `grpcs://`, `https://` or `host:port`) — the config file's `grpc` action.
/// `tls` forces TLS for a bare `host:port` (`[[upstream]] tls = true`).
#[cfg(feature = "http2")]
pub(crate) fn forward_h2(
    request: &Request,
    client_ip: &str,
    url: &str,
    tls: bool,
    trust: Option<&UpstreamTls>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    let mut backend = Backend::parse(url).ok_or_else(|| format!("invalid upstream address '{url}'"))?;
    backend.tls |= tls;
    let tls = backend.tls.then(|| match trust {
        Some(t) => t.h2_config(),
        None => h2_upstream::default_tls(),
    });
    h2_upstream::forward(request, client_ip, &h2_upstream::Target { backend: &backend, tls, connect_timeout, read_timeout, authority: None })
}

/// Forward over TLS to `ip:port` with `config`, `server_name` as SNI (and
/// the name `config`'s verifier checks), `host_header` as the `Host` — the
/// Gateway API router's BackendTLSPolicy backends. Connections are pooled
/// per `label` (one per distinct policy content).
#[cfg(feature = "gateway-api")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn forward_tls_with_host(
    config: &Arc<rustls::ClientConfig>,
    label: &str,
    request: &Request,
    client_ip: &str,
    ip: &str,
    port: u16,
    server_name: &str,
    host_header: &str,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    let pool = tls_pool_for_ca(label);
    forward_via(pool, request, client_ip, ip, port, host_header, read_timeout, || {
        let tcp = connect_tcp(ip, port, connect_timeout)?;
        let name = rustls::pki_types::ServerName::try_from(server_name.to_string()).map_err(|e| format!("invalid TLS server name '{server_name}': {e}"))?;
        let conn = rustls::ClientConnection::new(Arc::clone(config), name).map_err(|e| e.to_string())?;
        Ok(rustls::StreamOwned::new(conn, tcp))
    })
}

/// Forward over cleartext HTTP/2 with prior knowledge (h2c) to `host:port`,
/// sending `authority` as `:authority` — the Gateway API router's
/// `kubernetes.io/h2c` backends.
#[cfg(feature = "gateway-api")]
pub(crate) fn forward_h2c_with_host(
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    authority: &str,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    let backend = Backend { host: host.to_string(), port, tls: false };
    h2_upstream::forward(request, client_ip, &h2_upstream::Target { backend: &backend, tls: None, connect_timeout, read_timeout, authority: Some(authority) })
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

    /// See [`H2ReverseProxy::connect_timeout_ms`].
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.inner = self.inner.connect_timeout_ms(ms);
        self
    }

    /// See [`H2ReverseProxy::read_timeout_ms`]: a stream quiet for longer ends.
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.inner = self.inner.read_timeout_ms(ms);
        self
    }

    /// Trust this CA for `grpcs://` backends. Since 18.3.0.
    pub fn tls_ca_file(mut self, path: &str) -> Result<Self, String> {
        self.inner = self.inner.tls_ca_file(path)?;
        Ok(self)
    }

    /// Present a client certificate to `grpcs://` backends (mutual TLS).
    /// Since 18.3.0.
    pub fn tls_client_cert(mut self, cert: &str, key: &str) -> Result<Self, String> {
        self.inner = self.inner.tls_client_cert(cert, key)?;
        Ok(self)
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
        if is_grpc(request) {
            self.inner.handle(request, connection, next)
        } else {
            next.execute(request, connection)
        }
    }

    fn accepts_streamed_body(&self, _request: &Request, _connection: &crate::server::ConnectionInfo) -> bool {
        true
    }

    fn answers_streamed_body(&self, request: &Request, _connection: &crate::server::ConnectionInfo) -> bool {
        is_grpc(request) && self.inner.matches(request)
    }
}

#[cfg(feature = "http2")]
fn is_grpc(request: &Request) -> bool {
    request.header(crate::header::Header::_CONTENT_TYPE).is_some_and(|h| h.value.starts_with("application/grpc"))
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
