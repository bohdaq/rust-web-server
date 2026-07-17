//! Outbound HTTP/1.1 client.
//!
//! A minimal synchronous HTTP/1.1 + HTTPS client with no third-party HTTP
//! dependency.  TLS is backed by `rustls` (the same crate used by the server's
//! inbound TLS stack).
//!
//! # Plain HTTP (always available)
//!
//! ```rust,no_run
//! use rust_web_server::http_client::Client;
//!
//! let resp = Client::new()
//!     .get("http://httpbin.org/get")
//!     .header("X-Request-Id", "abc123")
//!     .timeout_ms(5_000)
//!     .send()
//!     .unwrap();
//!
//! assert!(resp.is_success());
//! println!("{}", resp.text().unwrap());
//! ```
//!
//! # HTTPS
//!
//! Requires the `http-client` feature (or `http2`/`http3`, which already pull
//! in `rustls`):
//!
//! ```toml
//! [dependencies]
//! rust-web-server = { version = "17", features = ["http-client"] }
//! ```
//!
//! Then use exactly the same API — the scheme in the URL selects the transport.
//!
//! # Async client
//!
//! Gated on the `http2` feature:
//!
//! ```rust,no_run
//! # #[cfg(feature = "http2")]
//! # async fn example() -> Result<(), rust_web_server::http_client::HttpClientError> {
//! use rust_web_server::http_client::AsyncClient;
//!
//! let resp = AsyncClient::new()
//!     .get("https://api.example.com/users")
//!     .header("Authorization", "Bearer tok_…")
//!     .send()
//!     .await?;
//!
//! println!("{}", resp.text()?);
//! # Ok(())
//! # }
//! ```

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::net::TcpStream;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(any(feature = "http-client", feature = "http2"))]
use std::sync::Arc;

// ── form encoding ─────────────────────────────────────────────────────────────

/// Percent-encode a single `application/x-www-form-urlencoded` value.
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Encode `pairs` as an `application/x-www-form-urlencoded` body.
fn form_urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", form_encode(k), form_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Error returned by the HTTP client.
#[derive(Debug)]
pub struct HttpClientError(pub String);

impl std::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HttpClientError {}

// ── URL parser ────────────────────────────────────────────────────────────────

struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    path_and_query: String,
}

impl ParsedUrl {
    fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Expect "scheme://rest"
        let rest = if let Some(r) = url.strip_prefix("https://") {
            ("https", r)
        } else if let Some(r) = url.strip_prefix("http://") {
            ("http", r)
        } else {
            return Err(HttpClientError(format!(
                "unsupported or missing URL scheme in '{url}'"
            )));
        };

        let (scheme, authority_and_path) = rest;
        let default_port: u16 = if scheme == "https" { 443 } else { 80 };

        // Split authority from path at the first '/'
        let (authority, path_and_query) = match authority_and_path.find('/') {
            Some(idx) => {
                let (a, p) = authority_and_path.split_at(idx);
                (a, p.to_string())
            }
            None => (authority_and_path, "/".to_string()),
        };

        // Split host and optional port
        let (host, port) = if let Some(bracket_end) = authority.find(']') {
            // IPv6 literal: [::1]:port
            let host = &authority[..=bracket_end];
            let port_part = &authority[bracket_end + 1..];
            let port = if let Some(p) = port_part.strip_prefix(':') {
                p.parse::<u16>().map_err(|_| {
                    HttpClientError(format!("invalid port in URL '{url}'"))
                })?
            } else {
                default_port
            };
            (host.to_string(), port)
        } else {
            match authority.rfind(':') {
                Some(idx) => {
                    let port_str = &authority[idx + 1..];
                    let port = port_str.parse::<u16>().map_err(|_| {
                        HttpClientError(format!("invalid port in URL '{url}'"))
                    })?;
                    (authority[..idx].to_string(), port)
                }
                None => (authority.to_string(), default_port),
            }
        };

        if host.is_empty() {
            return Err(HttpClientError(format!("missing host in URL '{url}'")));
        }

        Ok(ParsedUrl {
            scheme: scheme.to_string(),
            host,
            port,
            path_and_query,
        })
    }
}

/// Resolve `location` against `base_url`.  If `location` is already absolute
/// it is returned as-is.  A path starting with '/' is resolved against the
/// origin of `base_url`.
fn resolve_url(base_url: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // relative — reconstruct origin from base
    if let Ok(base) = ParsedUrl::parse(base_url) {
        let default_port = if base.scheme == "https" { 443 } else { 80 };
        let port_str = if base.port == default_port {
            String::new()
        } else {
            format!(":{}", base.port)
        };
        if location.starts_with('/') {
            return format!("{}://{}{}{}", base.scheme, base.host, port_str, location);
        }
        // relative path — resolve against directory of current path
        let base_path = base.path_and_query;
        let dir = match base_path.rfind('/') {
            Some(i) => &base_path[..=i],
            None => "/",
        };
        return format!(
            "{}://{}{}{}{}",
            base.scheme, base.host, port_str, dir, location
        );
    }
    location.to_string()
}

// ── Response ──────────────────────────────────────────────────────────────────

/// HTTP response from the outbound client.
#[derive(Debug)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// HTTP status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// `true` if the status code is in the 200–299 range.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// `true` if the status code is 301, 302, 303, 307, or 308.
    pub fn is_redirect(&self) -> bool {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
    }

    /// Look up a response header by name (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    /// All response headers, in the order the server sent them. Use this
    /// when you need to enumerate every header rather than look one up by
    /// name — e.g. forwarding a response verbatim.
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Raw response body bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// Decode the body as UTF-8.
    pub fn text(&self) -> Result<String, HttpClientError> {
        String::from_utf8(self.body.clone())
            .map_err(|e| HttpClientError(format!("body is not valid UTF-8: {e}")))
    }

    /// Parse the body as JSON.
    #[cfg(feature = "serde")]
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, HttpClientError> {
        serde_json::from_slice(&self.body)
            .map_err(|e| HttpClientError(format!("JSON parse error: {e}")))
    }
}

// ── Wire-level helpers ────────────────────────────────────────────────────────

/// Build the HTTP/1.1 request bytes.
fn build_request_bytes(
    method: &str,
    path_and_query: &str,
    host: &str,
    headers: &[(String, String)],
    body: &Option<Vec<u8>>,
) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();

    // Status line
    let _ = write!(
        out,
        "{method} {path_and_query} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: rust-web-server/{}\r\n",
        env!("CARGO_PKG_VERSION"),
    );

    // Content-Length (before custom headers, so callers can override)
    if let Some(b) = body {
        if !b.is_empty() {
            let _ = write!(out, "Content-Length: {}\r\n", b.len());
        }
    }

    // Custom headers
    for (k, v) in headers {
        let _ = write!(out, "{k}: {v}\r\n");
    }

    out.extend_from_slice(b"\r\n");

    if let Some(b) = body {
        out.extend_from_slice(b);
    }

    out
}

/// Parse an HTTP/1.1 response from any `Read` source.
fn read_response(stream: &mut dyn Read, is_head: bool) -> Result<Response, HttpClientError> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

    // Read until we find the end of headers (\r\n\r\n)
    let header_end = loop {
        let n = stream
            .read(&mut tmp)
            .map_err(|e| HttpClientError(format!("read error: {e}")))?;
        if n == 0 {
            if buf.is_empty() {
                return Err(HttpClientError(
                    "server closed connection without sending a response".into(),
                ));
            }
            // EOF before \r\n\r\n — try to parse whatever we got
            break buf.len();
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    // Split header block
    let header_block = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| HttpClientError("response headers are not valid UTF-8".into()))?;

    let mut lines = header_block.lines();

    // Status line
    let status_line = lines
        .next()
        .ok_or_else(|| HttpClientError("empty response".into()))?;
    let status = parse_status(status_line)?;

    // Headers
    let response_headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let mut parts = line.splitn(2, ':');
            let name = parts.next()?.trim().to_string();
            let value = parts.next()?.trim().to_string();
            if name.is_empty() {
                None
            } else {
                Some((name, value))
            }
        })
        .collect();

    // Body — already-buffered bytes beyond the header block
    let mut body = buf[header_end..].to_vec();

    if !is_head {
        // Determine body reading strategy from headers
        let transfer_encoding = response_headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == "transfer-encoding")
            .map(|(_, v)| v.to_lowercase());

        let content_length: Option<usize> = response_headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == "content-length")
            .and_then(|(_, v)| v.trim().parse().ok());

        if transfer_encoding
            .as_deref()
            .map(|te| te.contains("chunked"))
            .unwrap_or(false)
        {
            // Read remaining chunked data then decode
            loop {
                let n = stream
                    .read(&mut tmp)
                    .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body = decode_chunked(&body)?;
        } else if let Some(len) = content_length {
            while body.len() < len {
                let n = stream
                    .read(&mut tmp)
                    .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body.truncate(len);
        } else {
            // Read until EOF (Connection: close)
            loop {
                let n = stream
                    .read(&mut tmp)
                    .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
        }
    } else {
        body.clear();
    }

    Ok(Response {
        status,
        headers: response_headers,
        body,
    })
}

fn parse_status(line: &str) -> Result<u16, HttpClientError> {
    // "HTTP/1.x 200 Reason ..."
    let mut parts = line.splitn(3, ' ');
    let _version = parts
        .next()
        .ok_or_else(|| HttpClientError("malformed status line".into()))?;
    let code_str = parts
        .next()
        .ok_or_else(|| HttpClientError("missing status code".into()))?;
    code_str
        .parse::<u16>()
        .map_err(|_| HttpClientError(format!("invalid status code '{code_str}'")))
}

/// Decode chunked transfer encoding.
fn decode_chunked(data: &[u8]) -> Result<Vec<u8>, HttpClientError> {
    let mut out = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        // Find end of chunk-size line
        let line_end = data[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| HttpClientError("invalid chunked encoding: missing CRLF".into()))?;
        let size_line = std::str::from_utf8(&data[pos..pos + line_end])
            .map_err(|_| HttpClientError("chunked size is not ASCII".into()))?
            .trim();
        // Strip optional chunk extensions (;ext)
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_str, 16)
            .map_err(|_| HttpClientError(format!("invalid chunk size '{size_str}'")))?;
        pos += line_end + 2; // skip size line + CRLF

        if chunk_size == 0 {
            break; // last chunk
        }

        let end = pos + chunk_size;
        if end > data.len() {
            return Err(HttpClientError("chunked body truncated".into()));
        }
        out.extend_from_slice(&data[pos..end]);
        pos = end + 2; // skip trailing CRLF after chunk data
    }

    Ok(out)
}

// ── TLS connector (sync) ──────────────────────────────────────────────────────

#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
fn tls_connect(
    host: &str,
    tcp: TcpStream,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, HttpClientError> {
    use rustls::pki_types::ServerName;
    use rustls::ClientConfig;

    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| HttpClientError(format!("invalid hostname '{host}': {e}")))?;
    let conn = rustls::ClientConnection::new(config, server_name)
        .map_err(|e| HttpClientError(e.to_string()))?;
    Ok(rustls::StreamOwned::new(conn, tcp))
}

// ── Core send (one hop, no redirect) ─────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn send_once(
    method: &str,
    parsed: &ParsedUrl,
    headers: &[(String, String)],
    body: &Option<Vec<u8>>,
    timeout_ms: u64,
) -> Result<Response, HttpClientError> {
    let addr = format!("{}:{}", parsed.host, parsed.port);
    let timeout = Duration::from_millis(timeout_ms);

    // Resolve + connect
    let sock_addr = addr
        .parse::<std::net::SocketAddr>()
        .or_else(|_| {
            use std::net::ToSocketAddrs;
            addr.to_socket_addrs()
                .map_err(|e| HttpClientError(format!("DNS lookup for '{addr}' failed: {e}")))?
                .next()
                .ok_or_else(|| HttpClientError(format!("no address for '{addr}'")))
        })
        .map_err(|e: HttpClientError| e)?;

    let tcp = TcpStream::connect_timeout(&sock_addr, timeout)
        .map_err(|e| HttpClientError(format!("connect to '{addr}' failed: {e}")))?;
    tcp.set_read_timeout(Some(timeout))
        .map_err(|e| HttpClientError(e.to_string()))?;
    tcp.set_write_timeout(Some(timeout))
        .map_err(|e| HttpClientError(e.to_string()))?;

    let request_bytes =
        build_request_bytes(method, &parsed.path_and_query, &parsed.host, headers, body);

    let is_head = method.eq_ignore_ascii_case("HEAD");

    // Dispatch on scheme
    #[cfg(any(feature = "http-client", feature = "http2"))]
    if parsed.scheme == "https" {
        let mut tls_stream = tls_connect(&parsed.host, tcp)?;
        tls_stream
            .write_all(&request_bytes)
            .map_err(|e| HttpClientError(format!("write error: {e}")))?;
        return read_response(&mut tls_stream, is_head);
    }

    // Plain HTTP
    let mut stream = tcp;
    stream
        .write_all(&request_bytes)
        .map_err(|e| HttpClientError(format!("write error: {e}")))?;
    read_response(&mut stream, is_head)
}

/// wasm32 backend: no `TcpStream`/`rustls` in a wasi:http guest, so outbound
/// requests go through the `wasi:http` `outgoing-handler` interface instead
/// (imported by the same `wasi:http/proxy` world `rws-wasm-shim` already
/// builds against for the incoming side — see spec/WASM_SHIM.md Phase 2
/// item 7). The host performs the actual connect/TLS; this function only
/// builds the outgoing-request, blocks on the response future, and reads
/// the result back into the same `Response` shape the native backend uses.
///
/// Not exercised by `cargo test` — constructing real `wasi:http` resources
/// (`OutgoingRequest`, `outgoing_handler::handle`, ...) requires a live WASI
/// host underneath, same limitation `rws-wasm-shim`'s own tests document.
/// Verified instead against a real `wasmtime serve` process: both plain HTTP
/// and HTTPS (TLS terminated by the host, not this code) round-tripped
/// correctly through this exact `Client`/`RequestBuilder` API.
#[cfg(target_arch = "wasm32")]
fn send_once(
    method: &str,
    parsed: &ParsedUrl,
    headers: &[(String, String)],
    body: &Option<Vec<u8>>,
    timeout_ms: u64,
) -> Result<Response, HttpClientError> {
    use wasip2::http::outgoing_handler;
    use wasip2::http::types::{Fields, OutgoingBody, OutgoingRequest, RequestOptions, Scheme};

    let wasi_headers = Fields::new();
    for (name, value) in headers {
        let _ = wasi_headers.append(name, value.as_bytes());
    }

    let request = OutgoingRequest::new(wasi_headers);
    request
        .set_method(&to_wasi_method(method))
        .map_err(|_| HttpClientError(format!("invalid method '{method}'")))?;
    let scheme = if parsed.scheme == "https" { Scheme::Https } else { Scheme::Http };
    request
        .set_scheme(Some(&scheme))
        .map_err(|_| HttpClientError(format!("invalid scheme '{}'", parsed.scheme)))?;
    let authority = format!("{}:{}", parsed.host, parsed.port);
    request
        .set_authority(Some(&authority))
        .map_err(|_| HttpClientError(format!("invalid authority '{authority}'")))?;
    request
        .set_path_with_query(Some(&parsed.path_and_query))
        .map_err(|_| HttpClientError(format!("invalid path '{}'", parsed.path_and_query)))?;

    let outgoing_body = request
        .body()
        .map_err(|_| HttpClientError("outgoing-request body handle taken twice".to_string()))?;
    if let Some(bytes) = body {
        let mut out = outgoing_body
            .write()
            .map_err(|_| HttpClientError("outgoing-body write stream taken twice".to_string()))?;
        out.write_all(bytes)
            .map_err(|e| HttpClientError(format!("write error: {e}")))?;
    }
    OutgoingBody::finish(outgoing_body, None)
        .map_err(|e| HttpClientError(format!("failed to finish request body: {e:?}")))?;

    // Best-effort — a host that doesn't support transport timeouts returns an
    // error from these setters, which we don't treat as fatal (the request
    // just proceeds without a host-enforced timeout in that case).
    let options = RequestOptions::new();
    let timeout_ns = timeout_ms.saturating_mul(1_000_000);
    let _ = options.set_connect_timeout(Some(timeout_ns));
    let _ = options.set_first_byte_timeout(Some(timeout_ns));
    let _ = options.set_between_bytes_timeout(Some(timeout_ns));

    let future_response = outgoing_handler::handle(request, Some(options))
        .map_err(|e| HttpClientError(format!("failed to dispatch request: {e:?}")))?;

    future_response.subscribe().block();

    let incoming_response = match future_response.get() {
        Some(Ok(Ok(response))) => response,
        Some(Ok(Err(code))) => return Err(HttpClientError(format!("{code:?}"))),
        Some(Err(())) => {
            return Err(HttpClientError("response already retrieved".to_string()))
        }
        None => {
            return Err(HttpClientError(
                "response not ready after blocking on it".to_string(),
            ))
        }
    };

    let status = incoming_response.status();
    let resp_headers = incoming_response
        .headers()
        .entries()
        .into_iter()
        .map(|(name, value)| (name, String::from_utf8_lossy(&value).to_string()))
        .collect();

    let mut resp_body = Vec::new();
    if !method.eq_ignore_ascii_case("HEAD") {
        if let Ok(incoming_body) = incoming_response.consume() {
            if let Ok(mut stream) = incoming_body.stream() {
                let _ = stream.read_to_end(&mut resp_body);
            }
        }
    }

    Ok(Response { status, headers: resp_headers, body: resp_body })
}

#[cfg(target_arch = "wasm32")]
fn to_wasi_method(method: &str) -> wasip2::http::types::Method {
    use wasip2::http::types::Method;
    match method.to_ascii_uppercase().as_str() {
        "GET" => Method::Get,
        "HEAD" => Method::Head,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "CONNECT" => Method::Connect,
        "OPTIONS" => Method::Options,
        "TRACE" => Method::Trace,
        "PATCH" => Method::Patch,
        other => Method::Other(other.to_string()),
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

/// Synchronous HTTP/1.1 client.
///
/// Construct with [`Client::new()`], then call one of the method helpers
/// (`.get()`, `.post()`, …) to get a [`RequestBuilder`], configure it, and
/// call `.send()`.
pub struct Client {
    timeout_ms: u64,
    max_redirects: u8,
}

impl Client {
    /// Create a client with default settings:
    /// - `timeout_ms`: 30 000 (30 seconds)
    /// - `max_redirects`: 10
    pub fn new() -> Self {
        Self {
            timeout_ms: 30_000,
            max_redirects: 10,
        }
    }

    /// Override the per-request timeout (connect + read combined).
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Maximum number of redirects to follow (default: 10).
    pub fn max_redirects(mut self, n: u8) -> Self {
        self.max_redirects = n;
        self
    }

    /// Start building a GET request.
    pub fn get(&self, url: &str) -> RequestBuilder<'_> {
        self.request("GET", url)
    }

    /// Start building a POST request.
    pub fn post(&self, url: &str) -> RequestBuilder<'_> {
        self.request("POST", url)
    }

    /// Start building a PUT request.
    pub fn put(&self, url: &str) -> RequestBuilder<'_> {
        self.request("PUT", url)
    }

    /// Start building a PATCH request.
    pub fn patch(&self, url: &str) -> RequestBuilder<'_> {
        self.request("PATCH", url)
    }

    /// Start building a DELETE request.
    pub fn delete(&self, url: &str) -> RequestBuilder<'_> {
        self.request("DELETE", url)
    }

    /// Start building a HEAD request.
    pub fn head(&self, url: &str) -> RequestBuilder<'_> {
        self.request("HEAD", url)
    }

    /// Start building a request with an arbitrary HTTP method.
    pub fn request(&self, method: &str, url: &str) -> RequestBuilder<'_> {
        RequestBuilder {
            client: self,
            method: method.to_uppercase(),
            url: url.to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: None,
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

// ── RequestBuilder ────────────────────────────────────────────────────────────

/// Builder for a single HTTP request.
pub struct RequestBuilder<'a> {
    client: &'a Client,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    timeout_ms: Option<u64>,
}

impl<'a> RequestBuilder<'a> {
    /// Add a request header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Set a raw byte body.
    pub fn body(mut self, bytes: Vec<u8>) -> Self {
        self.body = Some(bytes);
        self
    }

    /// Set a plain-text body (also sets `Content-Type: text/plain`).
    pub fn body_text(mut self, s: &str) -> Self {
        self.headers
            .push(("Content-Type".to_string(), "text/plain".to_string()));
        self.body = Some(s.as_bytes().to_vec());
        self
    }

    /// Set a JSON body (also sets `Content-Type: application/json`).
    pub fn body_json(mut self, s: &str) -> Self {
        self.headers.push((
            "Content-Type".to_string(),
            "application/json".to_string(),
        ));
        self.body = Some(s.as_bytes().to_vec());
        self
    }

    /// Set an `application/x-www-form-urlencoded` body from key/value pairs
    /// (also sets `Content-Type: application/x-www-form-urlencoded`).
    ///
    /// This is the body shape OAuth 2.0 token endpoints require, e.g.:
    ///
    /// ```rust,no_run
    /// use rust_web_server::http_client::Client;
    ///
    /// let resp = Client::new()
    ///     .post("https://oauth2.googleapis.com/token")
    ///     .form(&[("grant_type", "authorization_code"), ("code", "abc123")])
    ///     .send()
    ///     .unwrap();
    /// ```
    pub fn form(mut self, pairs: &[(&str, &str)]) -> Self {
        self.headers.push((
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ));
        self.body = Some(form_urlencode(pairs).into_bytes());
        self
    }

    /// Override the timeout for this request.
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Send the request and return the response.
    ///
    /// Automatically follows redirects up to the client's `max_redirects`
    /// limit.
    pub fn send(self) -> Result<Response, HttpClientError> {
        let timeout = self.timeout_ms.unwrap_or(self.client.timeout_ms);
        let max_redirects = self.client.max_redirects;

        let mut method = self.method;
        let mut url = self.url;
        let headers = self.headers;
        let mut body = self.body;
        let mut redirects = 0u8;

        loop {
            let parsed = ParsedUrl::parse(&url)?;
            let resp = send_once(&method, &parsed, &headers, &body, timeout)?;

            if resp.is_redirect() && redirects < max_redirects {
                let location = resp
                    .header("location")
                    .ok_or_else(|| HttpClientError("redirect with no Location header".into()))?
                    .to_string();
                url = resolve_url(&url, &location);
                redirects += 1;
                if matches!(resp.status(), 301 | 302 | 303) {
                    method = "GET".to_string();
                    body = None;
                }
                continue;
            }

            return Ok(resp);
        }
    }
}

// ── Async client (`http2` feature) ───────────────────────────────────────────

#[cfg(feature = "http2")]
pub use async_impl::{AsyncClient, AsyncRequestBuilder};

#[cfg(feature = "http2")]
mod async_impl {
    use super::{
        build_request_bytes, decode_chunked, parse_status, resolve_url, HttpClientError,
        ParsedUrl, Response,
    };
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn async_tls_connect(
        host: &str,
        stream: tokio::net::TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>, HttpClientError> {
        use rustls::pki_types::ServerName;
        use rustls::ClientConfig;
        use tokio_rustls::TlsConnector;

        let root_store = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        let config = Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );
        let connector = TlsConnector::from(config);
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| HttpClientError(format!("invalid hostname '{host}': {e}")))?;
        connector
            .connect(server_name, stream)
            .await
            .map_err(|e| HttpClientError(format!("TLS handshake failed: {e}")))
    }

    async fn async_read_response(
        stream: &mut (impl AsyncReadExt + Unpin),
        is_head: bool,
    ) -> Result<Response, HttpClientError> {
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        let mut tmp = vec![0u8; 4096];

        let header_end = loop {
            let n = stream
                .read(&mut tmp)
                .await
                .map_err(|e| HttpClientError(format!("read error: {e}")))?;
            if n == 0 {
                if buf.is_empty() {
                    return Err(HttpClientError(
                        "server closed connection without a response".into(),
                    ));
                }
                break buf.len();
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };

        let header_block = std::str::from_utf8(&buf[..header_end])
            .map_err(|_| HttpClientError("response headers not UTF-8".into()))?;

        let mut lines = header_block.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| HttpClientError("empty response".into()))?;
        let status = parse_status(status_line)?;

        let response_headers: Vec<(String, String)> = lines
            .filter_map(|line| {
                let mut parts = line.splitn(2, ':');
                let name = parts.next()?.trim().to_string();
                let value = parts.next()?.trim().to_string();
                if name.is_empty() { None } else { Some((name, value)) }
            })
            .collect();

        let mut body = buf[header_end..].to_vec();

        if !is_head {
            let transfer_encoding = response_headers
                .iter()
                .find(|(k, _)| k.to_lowercase() == "transfer-encoding")
                .map(|(_, v)| v.to_lowercase());

            let content_length: Option<usize> = response_headers
                .iter()
                .find(|(k, _)| k.to_lowercase() == "content-length")
                .and_then(|(_, v)| v.trim().parse().ok());

            if transfer_encoding
                .as_deref()
                .map(|te| te.contains("chunked"))
                .unwrap_or(false)
            {
                loop {
                    let n = stream.read(&mut tmp).await
                        .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                    if n == 0 { break; }
                    body.extend_from_slice(&tmp[..n]);
                }
                body = decode_chunked(&body)?;
            } else if let Some(len) = content_length {
                while body.len() < len {
                    let n = stream.read(&mut tmp).await
                        .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                    if n == 0 { break; }
                    body.extend_from_slice(&tmp[..n]);
                }
                body.truncate(len);
            } else {
                loop {
                    let n = stream.read(&mut tmp).await
                        .map_err(|e| HttpClientError(format!("read error: {e}")))?;
                    if n == 0 { break; }
                    body.extend_from_slice(&tmp[..n]);
                }
            }
        } else {
            body.clear();
        }

        Ok(Response { status, headers: response_headers, body })
    }

    async fn async_send_once(
        method: &str,
        parsed: &ParsedUrl,
        headers: &[(String, String)],
        body: &Option<Vec<u8>>,
        timeout_ms: u64,
    ) -> Result<Response, HttpClientError> {
        use std::time::Duration;
        use tokio::net::TcpStream;
        use tokio::time::timeout;

        let addr = format!("{}:{}", parsed.host, parsed.port);
        let dur = Duration::from_millis(timeout_ms);
        let request_bytes =
            build_request_bytes(method, &parsed.path_and_query, &parsed.host, headers, body);
        let is_head = method.eq_ignore_ascii_case("HEAD");

        let tcp = timeout(dur, TcpStream::connect(&addr))
            .await
            .map_err(|_| HttpClientError(format!("connect to '{addr}' timed out")))?
            .map_err(|e| HttpClientError(format!("connect to '{addr}' failed: {e}")))?;

        if parsed.scheme == "https" {
            let tls_stream = timeout(dur, async_tls_connect(&parsed.host, tcp))
                .await
                .map_err(|_| HttpClientError("TLS handshake timed out".into()))??;
            let mut stream = tls_stream;
            timeout(dur, stream.write_all(&request_bytes))
                .await
                .map_err(|_| HttpClientError("write timed out".into()))?
                .map_err(|e| HttpClientError(format!("write error: {e}")))?;
            return timeout(dur, async_read_response(&mut stream, is_head))
                .await
                .map_err(|_| HttpClientError("read timed out".into()))?;
        }

        let mut stream = tcp;
        timeout(dur, stream.write_all(&request_bytes))
            .await
            .map_err(|_| HttpClientError("write timed out".into()))?
            .map_err(|e| HttpClientError(format!("write error: {e}")))?;
        timeout(dur, async_read_response(&mut stream, is_head))
            .await
            .map_err(|_| HttpClientError("read timed out".into()))?
    }

    /// Asynchronous HTTP/1.1 client (`http2` feature required).
    pub struct AsyncClient {
        timeout_ms: u64,
        max_redirects: u8,
    }

    impl AsyncClient {
        /// Create with default settings (30 s timeout, 10 redirects).
        pub fn new() -> Self {
            Self {
                timeout_ms: 30_000,
                max_redirects: 10,
            }
        }

        /// Override the per-request timeout.
        pub fn timeout_ms(mut self, ms: u64) -> Self {
            self.timeout_ms = ms;
            self
        }

        /// Maximum redirects to follow.
        pub fn max_redirects(mut self, n: u8) -> Self {
            self.max_redirects = n;
            self
        }

        /// Start a GET request.
        pub fn get(&self, url: &str) -> AsyncRequestBuilder<'_> {
            self.request("GET", url)
        }

        /// Start a POST request.
        pub fn post(&self, url: &str) -> AsyncRequestBuilder<'_> {
            self.request("POST", url)
        }

        /// Start a PUT request.
        pub fn put(&self, url: &str) -> AsyncRequestBuilder<'_> {
            self.request("PUT", url)
        }

        /// Start a PATCH request.
        pub fn patch(&self, url: &str) -> AsyncRequestBuilder<'_> {
            self.request("PATCH", url)
        }

        /// Start a DELETE request.
        pub fn delete(&self, url: &str) -> AsyncRequestBuilder<'_> {
            self.request("DELETE", url)
        }

        /// Start a request with an arbitrary method.
        pub fn request(&self, method: &str, url: &str) -> AsyncRequestBuilder<'_> {
            AsyncRequestBuilder {
                client: self,
                method: method.to_uppercase(),
                url: url.to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: None,
            }
        }
    }

    impl Default for AsyncClient {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Builder for an async HTTP request.
    pub struct AsyncRequestBuilder<'a> {
        client: &'a AsyncClient,
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        timeout_ms: Option<u64>,
    }

    impl<'a> AsyncRequestBuilder<'a> {
        /// Add a request header.
        pub fn header(mut self, name: &str, value: &str) -> Self {
            self.headers.push((name.to_string(), value.to_string()));
            self
        }

        /// Set a raw byte body.
        pub fn body(mut self, bytes: Vec<u8>) -> Self {
            self.body = Some(bytes);
            self
        }

        /// Set a plain-text body (sets `Content-Type: text/plain`).
        pub fn body_text(mut self, s: &str) -> Self {
            self.headers
                .push(("Content-Type".to_string(), "text/plain".to_string()));
            self.body = Some(s.as_bytes().to_vec());
            self
        }

        /// Set a JSON body (sets `Content-Type: application/json`).
        pub fn body_json(mut self, s: &str) -> Self {
            self.headers.push((
                "Content-Type".to_string(),
                "application/json".to_string(),
            ));
            self.body = Some(s.as_bytes().to_vec());
            self
        }

        /// Set an `application/x-www-form-urlencoded` body from key/value
        /// pairs (sets `Content-Type: application/x-www-form-urlencoded`).
        pub fn form(mut self, pairs: &[(&str, &str)]) -> Self {
            self.headers.push((
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            ));
            self.body = Some(super::form_urlencode(pairs).into_bytes());
            self
        }

        /// Override the timeout for this request.
        pub fn timeout_ms(mut self, ms: u64) -> Self {
            self.timeout_ms = Some(ms);
            self
        }

        /// Send the request asynchronously.
        pub async fn send(self) -> Result<Response, HttpClientError> {
            let timeout = self.timeout_ms.unwrap_or(self.client.timeout_ms);
            let max_redirects = self.client.max_redirects;

            let mut method = self.method;
            let mut url = self.url;
            let headers = self.headers;
            let mut body = self.body;
            let mut redirects = 0u8;

            loop {
                let parsed = ParsedUrl::parse(&url)?;
                let resp = async_send_once(&method, &parsed, &headers, &body, timeout).await?;

                if resp.is_redirect() && redirects < max_redirects {
                    let location = resp
                        .header("location")
                        .ok_or_else(|| {
                            HttpClientError("redirect with no Location header".into())
                        })?
                        .to_string();
                    url = resolve_url(&url, &location);
                    redirects += 1;
                    if matches!(resp.status(), 301 | 302 | 303) {
                        method = "GET".to_string();
                        body = None;
                    }
                    continue;
                }

                return Ok(resp);
            }
        }
    }
}
