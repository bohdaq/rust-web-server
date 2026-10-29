//! Non-blocking upstream forwarding for HTTP and HTTPS backends (`http2` builds).
//!
//! The synchronous forwarder (`forward_upstream`) holds a blocking-pool
//! thread for the whole round trip to the backend: under load most threads
//! sat in `recvfrom` waiting for upstream bytes, and each request paid two
//! cross-thread hand-offs. This one runs on the connection's own task with
//! tokio sockets. Same behaviour otherwise: pooled keep-alive connections
//! (per `host:port`), the same request bytes (`build_request`), a stale pooled
//! connection retried once for idempotent methods only, and streamed
//! responses (SSE, chunked, > 1 MB) handed to the existing streaming writer —
//! a plain socket converted to a blocking one, a TLS stream through a small
//! copying task and a bounded channel. TLS backends use pooled tokio-rustls
//! connections, with the upstream's `tls_ca_file` roots when it has them.
//!
//! Used by config-driven proxy routes without middleware
//! (`DynamicProxy::execute_async`); everything else keeps the blocking path.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{
    Peer,
    build_request, is_idempotent, is_interim_response, parse_status_and_headers, HeadFacts, MAX_INTERIM_RESPONSES,
    ConcatReader, Pool, POOL_MAX_IDLE,
};
use crate::request::Request;
use crate::response::Response;

/// A TLS connection to a backend.
pub(crate) type AsyncTlsStream = tokio_rustls::client::TlsStream<TcpStream>;

/// Idle async plain-HTTP backend connections, per `host:port`.
fn pool() -> &'static Pool<TcpStream> {
    static POOL: std::sync::OnceLock<Pool<TcpStream>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))
}

/// Idle async TLS backend connections verified against the WebPKI roots.
pub(crate) fn default_tls_pool() -> &'static Pool<AsyncTlsStream> {
    static POOL: std::sync::OnceLock<Pool<AsyncTlsStream>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))
}

/// How to reach a TLS backend: the client configuration (roots) and the
/// pool of connections made with it.
pub(crate) struct AsyncTls {
    pub(crate) config: Arc<rustls::ClientConfig>,
    pub(crate) pool: &'static Pool<AsyncTlsStream>,
}

/// A backend connection the async forwarder can drive.
trait UpstreamConn: AsyncRead + AsyncWrite + Unpin + Send + 'static {
    /// The rest of a streamed response body as a blocking reader, after
    /// `prefix` (body bytes already read with the head).
    fn into_body_reader(self, prefix: Vec<u8>, read_timeout: Duration) -> Result<Box<dyn Read + Send>, String>;
}

impl UpstreamConn for TcpStream {
    fn into_body_reader(self, prefix: Vec<u8>, read_timeout: Duration) -> Result<Box<dyn Read + Send>, String> {
        let std_stream = self.into_std().map_err(|e| e.to_string())?;
        std_stream.set_nonblocking(false).map_err(|e| e.to_string())?;
        std_stream.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
        Ok(Box::new(ConcatReader::new(prefix, std_stream)))
    }
}

impl UpstreamConn for AsyncTlsStream {
    fn into_body_reader(mut self, prefix: Vec<u8>, read_timeout: Duration) -> Result<Box<dyn Read + Send>, String> {
        // A TLS stream can't become a blocking socket: a task copies it into
        // a bounded channel that the (blocking) streaming writer reads.
        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match tokio::time::timeout(read_timeout, self.read(&mut buf)).await {
                    Ok(Ok(n)) if n > 0 => {
                        if tx.send(buf[..n].to_vec()).await.is_err() {
                            return; // the client went away
                        }
                    }
                    _ => return, // end of body, error or stall: the reader sees EOF
                }
            }
        });
        Ok(Box::new(ChannelBodyReader { rx, chunk: prefix, pos: 0 }))
    }
}

/// Blocking `Read` over the chunks a copying task sends.
struct ChannelBodyReader {
    rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    chunk: Vec<u8>,
    pos: usize,
}

impl Read for ChannelBodyReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.chunk.len() {
            match self.rx.blocking_recv() {
                Some(next) => {
                    self.chunk = next;
                    self.pos = 0;
                }
                None => return Ok(0),
            }
        }
        let n = out.len().min(self.chunk.len() - self.pos);
        out[..n].copy_from_slice(&self.chunk[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

enum Exchange {
    Done(Response),
    /// Failed before any response byte arrived: the backend may have closed
    /// a pooled connection, so an idempotent request can be retried.
    NoResponse(String),
    Failed(String),
}

/// Forward `request` to `host:port` — over TLS when `tls` is given — and
/// return the backend's response.
pub(crate) async fn forward(
    request: &Request,
    client: Peer<'_>,
    host: &str,
    port: u16,
    tls: Option<AsyncTls>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    // `host:port`, pushed rather than `format!`ed (once per proxied request).
    let port_digits = crate::ext::decimal::Decimal::new(port as u128);
    let mut key = String::with_capacity(host.len() + 1 + port_digits.as_str().len());
    key.push_str(host);
    key.push(':');
    key.push_str(port_digits.as_str());
    let pooled = crate::server::per_core::pool_key(&key);
    match tls {
        None => {
            forward_on(pool(), &pooled, request, host, client, read_timeout, || connect(&key, connect_timeout)).await
        }
        Some(tls) => {
            let connect_tls = || async {
                let tcp = connect(&key, connect_timeout).await?;
                let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
                    .map_err(|e| format!("invalid upstream hostname '{}': {}", host, e))?;
                let connector = tokio_rustls::TlsConnector::from(Arc::clone(&tls.config));
                match tokio::time::timeout(connect_timeout, connector.connect(server_name, tcp)).await {
                    Ok(Ok(stream)) => Ok(stream),
                    Ok(Err(e)) => Err(format!("TLS handshake with {} failed: {}", key, e)),
                    Err(_) => Err(format!("TLS handshake with {} timed out", key)),
                }
            };
            forward_on(tls.pool, &pooled, request, host, client, read_timeout, connect_tls).await
        }
    }
}

/// Idle connections the Gateway API router keeps per backend endpoint: a
/// gateway carries many clients' requests to few endpoints, so its pools
/// are larger than the config-file proxy's ([`POOL_MAX_IDLE`]).
#[cfg(feature = "gateway-api")]
const GATEWAY_POOL_MAX_IDLE: usize = 1024;

#[cfg(feature = "gateway-api")]
fn gateway_pool() -> &'static Pool<TcpStream> {
    static POOL: std::sync::OnceLock<Pool<TcpStream>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Pool::new(GATEWAY_POOL_MAX_IDLE, Duration::from_secs(60)))
}

/// The Gateway router's TLS pool for one BackendTLSPolicy configuration
/// (its label).
#[cfg(feature = "gateway-api")]
pub(crate) fn gateway_tls_pool(label: &str) -> &'static Pool<AsyncTlsStream> {
    static POOLS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, &'static Pool<AsyncTlsStream>>>> = std::sync::OnceLock::new();
    let mut pools = POOLS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    pools.entry(label.to_string()).or_insert_with(|| Box::leak(Box::new(Pool::new(GATEWAY_POOL_MAX_IDLE, Duration::from_secs(60)))))
}

/// Forward to an endpoint `ip:port` for the Gateway API router: `host_header`
/// as the `Host` (the client's), and with `tls` = (server name, config) over
/// TLS with that SNI.
#[cfg(feature = "gateway-api")]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn forward_gateway(
    request: &Request,
    client: Peer<'_>,
    ip: &str,
    port: u16,
    host_header: &str,
    tls: Option<(&str, AsyncTls)>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    let key = if ip.contains(':') { format!("[{ip}]:{port}") } else { format!("{ip}:{port}") };
    let pooled = crate::server::per_core::pool_key(&key);
    match tls {
        None => forward_on(gateway_pool(), &pooled, request, host_header, client, read_timeout, || connect(&key, connect_timeout)).await,
        Some((server_name, tls)) => {
            let connect_tls = || async {
                let tcp = connect(&key, connect_timeout).await?;
                let name = rustls::pki_types::ServerName::try_from(server_name.to_string()).map_err(|e| format!("invalid TLS server name '{server_name}': {e}"))?;
                let connector = tokio_rustls::TlsConnector::from(Arc::clone(&tls.config));
                match tokio::time::timeout(connect_timeout, connector.connect(name, tcp)).await {
                    Ok(Ok(stream)) => Ok(stream),
                    Ok(Err(e)) => Err(format!("TLS handshake with {key} failed: {e}")),
                    Err(_) => Err(format!("TLS handshake with {key} timed out")),
                }
            };
            forward_on(tls.pool, &pooled, request, host_header, client, read_timeout, connect_tls).await
        }
    }
}

/// The pooled request/response cycle for any connection type: a pooled
/// connection that fails before any response byte is retried once on a
/// fresh one, for idempotent methods only.
async fn forward_on<S, F, Fut>(
    pool: &Pool<S>,
    key: &str,
    request: &Request,
    host: &str,
    client: Peer<'_>,
    read_timeout: Duration,
    connect: F,
) -> Result<Response, String>
where
    S: UpstreamConn,
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<S, String>>,
{
    if let Some(stream) = pool.acquire(key) {
        match exchange(pool, key, stream, request, host, client, read_timeout).await {
            Exchange::Done(response) => return Ok(response),
            Exchange::NoResponse(_) if is_idempotent(&request.method) => {} // stale: retry fresh
            Exchange::NoResponse(e) | Exchange::Failed(e) => return Err(e),
        }
    }
    let stream = connect().await?;
    match exchange(pool, key, stream, request, host, client, read_timeout).await {
        Exchange::Done(response) => Ok(response),
        Exchange::NoResponse(e) | Exchange::Failed(e) => Err(e),
    }
}

/// Connect to the first address of `key` that accepts (`localhost` may
/// resolve to `::1` before `127.0.0.1`).
async fn connect(key: &str, connect_timeout: Duration) -> Result<TcpStream, String> {
    let addrs = tokio::net::lookup_host(key)
        .await
        .map_err(|e| format!("DNS lookup for {} failed: {}", key, e))?;
    let mut last_error = format!("no address resolved for {}", key);
    for addr in addrs {
        match tokio::time::timeout(connect_timeout, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Ok(Err(e)) => last_error = format!("connect to {} ({}) failed: {}", key, addr, e),
            Err(_) => last_error = format!("connect to {} ({}) timed out", key, addr),
        }
    }
    Err(last_error)
}

/// One request/response on `stream`. A fully read keep-alive response puts
/// the connection back in `pool`.
async fn exchange<S: UpstreamConn>(
    pool: &Pool<S>,
    key: &str,
    mut stream: S,
    request: &Request,
    host: &str,
    client: Peer<'_>,
    read_timeout: Duration,
) -> Exchange {
    let request_bytes = build_request(request, host, client, true);
    if let Err(e) = stream.write_all(&request_bytes).await {
        return Exchange::NoResponse(format!("write to backend failed: {}", e));
    }

    // Reads go straight into `buf`'s spare capacity (at least 8 KiB per
    // read, as before) — no zeroed 8 KiB stack buffer copied from each time.
    let mut buf: Vec<u8> = Vec::with_capacity(8192);

    // The final response head, bounded by the read timeout as a whole.
    // Interim 1xx heads are dropped (see `read_final_response_head`): taking
    // one as the response left the real one for the next pooled request.
    let head = tokio::time::timeout(read_timeout, async {
        let mut interim = 0;
        loop {
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                if !is_interim_response(&buf[..pos + 4]) {
                    return Ok(pos + 4);
                }
                interim += 1;
                if interim > MAX_INTERIM_RESPONSES {
                    return Err((false, "too many interim (1xx) responses from the backend".to_string()));
                }
                buf.drain(..pos + 4);
                continue;
            }
            let nothing = buf.is_empty() && interim == 0;
            match read_more(&mut stream, &mut buf).await {
                Ok(0) => return Err((nothing, "backend closed connection before headers were complete".to_string())),
                Ok(_) => {}
                Err(e) => return Err((nothing, e.to_string())),
            }
        }
    })
    .await;
    let header_end = match head {
        Ok(Ok(end)) => end,
        Ok(Err((true, e))) => return Exchange::NoResponse(e),
        Ok(Err((false, e))) => return Exchange::Failed(e),
        Err(_) => return Exchange::Failed("backend response timed out".to_string()),
    };
    let facts = HeadFacts::scan(&buf[..header_end]);
    let no_body = facts.no_body(&request.method);

    if !no_body && facts.should_stream() {
        // Streaming: the existing (blocking) writer reads the rest of the
        // body; the connection isn't reused.
        let body_prefix = buf.split_off(header_end);
        let body_reader = match stream.into_body_reader(body_prefix, read_timeout) {
            Ok(reader) => reader,
            Err(e) => return Exchange::Failed(e),
        };
        return match parse_status_and_headers(&buf) {
            Ok(mut response) => {
                response.stream_pipe = Some(body_reader);
                Exchange::Done(response)
            }
            Err(e) => Exchange::Failed(e),
        };
    }

    let connection_close = facts.connection_close;
    let content_length = facts.content_length;

    let body = tokio::time::timeout(read_timeout, async {
        match content_length {
            _ if no_body => {
                buf.truncate(header_end);
                Ok(!connection_close)
            }
            Some(len) => {
                while buf.len() < header_end + len {
                    let n = read_more(&mut stream, &mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Err("backend closed connection mid-body".to_string());
                    }
                }
                buf.truncate(header_end + len);
                Ok(!connection_close)
            }
            None => {
                // Delimited by the connection closing.
                loop {
                    let n = read_more(&mut stream, &mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Ok(false);
                    }
                }
            }
        }
    })
    .await;
    let reusable = match body {
        Ok(Ok(reusable)) => reusable,
        Ok(Err(e)) => return Exchange::Failed(e),
        Err(_) => return Exchange::Failed("backend response timed out".to_string()),
    };
    if reusable {
        pool.release(key, stream);
    }
    match Response::parse_upstream(&buf, super::REGENERATED_OR_HOP_BY_HOP) {
        Ok(response) => Exchange::Done(response),
        Err(e) => Exchange::Failed(e),
    }
}

#[cfg(test)]
mod tests;

/// Read once from `stream` into the spare capacity of `buf` (at least 8 KiB
/// of it); the bytes read, 0 at end of stream.
async fn read_more<S: tokio::io::AsyncRead + Unpin>(stream: &mut S, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    use tokio::io::AsyncReadExt;
    buf.reserve(8192);
    stream.read_buf(buf).await
}
