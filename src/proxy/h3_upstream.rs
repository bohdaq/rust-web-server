//! HTTP/3 upstreams (18.13.0, #37): requests to a backend over QUIC, for
//! `[[upstream]] protocol = "h3"` and [`H3ReverseProxy`](super::H3ReverseProxy).
//!
//! Built like [`h2_upstream`](super::h2_upstream): one process-wide
//! runtime, `rws-h3-upstream`, owns the QUIC endpoints and one multiplexed
//! connection per backend and trust setting (tokio and quinn I/O belong to
//! the runtime that made them, and the caller may run under the shared
//! runtime, a per-core one or none). Bodies stream both ways; the response
//! head comes back as soon as it arrives, the body through `stream_pipe` and
//! trailers in [`Response::trailers`]. Small non-gRPC responses with a
//! length are buffered.
//!
//! A failure to *connect* (no QUIC handshake within the connect timeout,
//! an unreachable address, a TLS alert) is [`H3Error::Connect`], so callers
//! can fall back to TCP; anything after that is [`H3Error::Failed`].

use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bytes::{Buf, Bytes};

use crate::core::New;
use crate::header::Header;
use crate::range::Range;
use crate::request::{Body, Request};
use crate::response::{Response, Trailers};

/// Responses up to this size with a `content-length` (and not gRPC) are
/// buffered; larger ones stream.
const BUFFER_LIMIT: u64 = 1 << 20;

/// Upstream body chunks in flight between the connection and the writer.
const BODY_CHANNEL: usize = 4;

/// Request headers HTTP/3 forbids or that describe one connection.
const NOT_FORWARDED: &[&str] = &[
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "proxy-connection",
    "te", "trailer", "transfer-encoding", "upgrade", "host",
];

/// Response headers HTTP/3 forbids or rws regenerates.
const NOT_RETURNED: &[&str] = &["connection", "keep-alive", "transfer-encoding", "upgrade", "proxy-connection", "te"];

type SendRequest = h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>;
type Stream = h3::client::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>;

/// Why a forward failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum H3Error {
    /// No QUIC connection could be made: the caller may try TCP instead.
    Connect(String),
    /// The connection was up; the exchange failed.
    Failed(String),
}

impl std::fmt::Display for H3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            H3Error::Connect(e) | H3Error::Failed(e) => f.write_str(e),
        }
    }
}

/// Where to send a request.
pub(crate) struct Target<'a> {
    pub(crate) host: &'a str,
    pub(crate) port: u16,
    /// The TLS client configuration (trust roots, client certificate); its
    /// ALPN is replaced with `h3`.
    pub(crate) tls: &'a Arc<rustls::ClientConfig>,
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rws-h3-upstream")
            .enable_all()
            .build()
            .expect("the HTTP/3 upstream runtime")
    })
}

/// The client configuration for backends without a `tls_ca_file`: WebPKI.
pub(crate) fn default_tls() -> &'static Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth())
    })
}

/// `tls` as a QUIC client configuration with ALPN `h3`, built once per
/// configuration (the source is kept so its address isn't reused).
fn quic_config(tls: &Arc<rustls::ClientConfig>) -> Result<quinn::ClientConfig, String> {
    type Cache = Mutex<HashMap<usize, (Arc<rustls::ClientConfig>, quinn::ClientConfig)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = Arc::as_ptr(tls) as usize;
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, config)) = cache.get(&key) {
        return Ok(config.clone());
    }
    let mut rustls_config = (**tls).clone();
    rustls_config.alpn_protocols = vec![b"h3".to_vec()];
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(rustls_config)
        .map_err(|e| format!("h3 proxy: the TLS configuration can't be used for QUIC: {e}"))?;
    let config = quinn::ClientConfig::new(Arc::new(quic));
    cache.insert(key, (Arc::clone(tls), config.clone()));
    Ok(config)
}

/// One client endpoint per address family, made on the h3 runtime.
fn endpoint(v6: bool) -> Result<quinn::Endpoint, String> {
    static ENDPOINTS: OnceLock<Mutex<[Option<quinn::Endpoint>; 2]>> = OnceLock::new();
    let mut endpoints = ENDPOINTS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let slot = &mut endpoints[v6 as usize];
    if let Some(endpoint) = slot {
        return Ok(endpoint.clone());
    }
    let bind: SocketAddr = if v6 { "[::]:0".parse().expect("address") } else { "0.0.0.0:0".parse().expect("address") };
    let endpoint = quinn::Endpoint::client(bind).map_err(|e| format!("h3 proxy: can't open a UDP socket: {e}"))?;
    *slot = Some(endpoint.clone());
    Ok(endpoint)
}

/// Pooled connections by key, each with an id so a closing connection
/// removes itself and not a newer one under the same key.
type Pool = Mutex<HashMap<String, (u64, SendRequest)>>;

static NEXT_CONNECTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(Default::default)
}

/// Forward `request` over HTTP/3 and return the response head — its body
/// streaming in `stream_pipe` unless small enough to buffer.
pub(crate) fn forward(request: &Request, client: crate::proxy::Peer<'_>, target: &Target<'_>) -> Result<Response, H3Error> {
    let receiver = start(request, client, target)?;
    let wait = target.connect_timeout + target.read_timeout + Duration::from_secs(1);
    match receiver.recv_timeout(wait) {
        Ok(result) => result,
        Err(_) => Err(H3Error::Failed(format!("h3 proxy: no response head within {} ms", wait.as_millis()))),
    }
}

/// [`forward`] for async callers: awaits the head instead of blocking.
pub(crate) async fn forward_async(request: &Request, client: crate::proxy::Peer<'_>, target: &Target<'_>) -> Result<Response, H3Error> {
    let receiver = start(request, client, target)?;
    let wait = target.connect_timeout + target.read_timeout + Duration::from_secs(1);
    let (tx, rx) = tokio::sync::oneshot::channel();
    runtime().spawn_blocking(move || {
        let _ = tx.send(receiver.recv_timeout(wait));
    });
    match rx.await {
        Ok(Ok(result)) => result,
        _ => Err(H3Error::Failed(format!("h3 proxy: no response head within {} ms", wait.as_millis()))),
    }
}

type HeadSender = std::sync::mpsc::SyncSender<Result<Response, H3Error>>;

fn start(
    request: &Request,
    client: crate::proxy::Peer<'_>,
    target: &Target<'_>,
) -> Result<std::sync::mpsc::Receiver<Result<Response, H3Error>>, H3Error> {
    let addr = format!("{}:{}", target.host, target.port);
    let head = request_head(request, client, &addr).map_err(H3Error::Failed)?;
    let quic = quic_config(target.tls).map_err(H3Error::Connect)?;
    let (head_tx, head_rx) = std::sync::mpsc::sync_channel(1);
    let exchange = Exchange {
        key: format!("{:x}|{addr}", Arc::as_ptr(target.tls) as usize),
        addr,
        host: target.host.to_string(),
        quic,
        connect_timeout: target.connect_timeout,
        read_timeout: target.read_timeout,
    };
    let body = request.body.clone();
    runtime().spawn(async move {
        if let Err(e) = exchange.run(head, body, &head_tx).await {
            // Only reaches the caller if the head wasn't sent yet.
            let _ = head_tx.try_send(Err(e));
        }
    });
    Ok(head_rx)
}

fn request_head(request: &Request, client: crate::proxy::Peer<'_>, addr: &str) -> Result<http::Request<()>, String> {
    let uri: http::Uri = format!("https://{addr}{}", request.request_uri).parse().map_err(|e: http::uri::InvalidUri| e.to_string())?;
    let method = http::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = http::Request::builder().method(method).uri(uri);
    for h in &request.headers {
        let lower = h.name.to_ascii_lowercase();
        if lower.is_empty() || NOT_FORWARDED.contains(&lower.as_str()) {
            if lower == "te" && h.value.split(',').any(|v| v.trim().eq_ignore_ascii_case("trailers")) {
                builder = builder.header("te", "trailers");
            }
            continue;
        }
        builder = builder.header(h.name.as_ref(), h.value.as_ref());
    }
    builder = builder.header("x-forwarded-for", client.ip).header("via", "3 rws");
    if request.header("X-Forwarded-Proto").is_none() {
        builder = builder.header("x-forwarded-proto", client.proto());
    }
    if request.header("X-Forwarded-Host").is_none() {
        if let Some(host) = request.header("host").filter(|h| !h.value.is_empty()) {
            builder = builder.header("x-forwarded-host", host.value.as_ref());
        }
    }
    builder.body(()).map_err(|e| e.to_string())
}

struct Exchange {
    key: String,
    addr: String,
    host: String,
    quic: quinn::ClientConfig,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl Exchange {
    async fn run(self, head: http::Request<()>, body: Body, head_tx: &HeadSender) -> Result<(), H3Error> {
        let stream = self.send(head).await?;
        let (send, recv) = stream.split();
        tokio::spawn(send_body(send, body));
        self.respond(recv, head_tx).await
    }

    /// Open a request stream on a pooled connection, or a new one when the
    /// pooled one is gone. A request is retried only before it was sent.
    async fn send(&self, head: http::Request<()>) -> Result<Stream, H3Error> {
        let pooled = pool().lock().unwrap_or_else(|e| e.into_inner()).get(&self.key).map(|(_, s)| s.clone());
        if let Some(mut sender) = pooled {
            match self.within(sender.send_request(head.clone())).await? {
                Ok(stream) => return Ok(stream),
                Err(_) => {
                    pool().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
                }
            }
        }
        let mut sender = self.connect().await?;
        self.within(sender.send_request(head))
            .await?
            .map_err(|e| H3Error::Failed(format!("h3 proxy: sending to {} failed: {e}", self.addr)))
    }

    async fn connect(&self) -> Result<SendRequest, H3Error> {
        let connect_error = |e: String| H3Error::Connect(format!("h3 proxy: connect to {} failed: {e}", self.addr));
        let remote = tokio::time::timeout(self.connect_timeout, tokio::net::lookup_host(&self.addr))
            .await
            .map_err(|_| connect_error("name lookup timed out".to_string()))?
            .map_err(|e| connect_error(e.to_string()))?
            .next()
            .ok_or_else(|| connect_error("no address".to_string()))?;
        let endpoint = endpoint(remote.is_ipv6()).map_err(H3Error::Connect)?;
        let connecting = endpoint.connect_with(self.quic.clone(), remote, &self.host).map_err(|e| connect_error(e.to_string()))?;
        let connection = tokio::time::timeout(self.connect_timeout, connecting)
            .await
            .map_err(|_| H3Error::Connect(format!("h3 proxy: no QUIC handshake with {} within {} ms", self.addr, self.connect_timeout.as_millis())))?
            .map_err(|e| connect_error(e.to_string()))?;
        let (mut driver, sender) = tokio::time::timeout(self.connect_timeout, h3::client::new(h3_quinn::Connection::new(connection)))
            .await
            .map_err(|_| connect_error("HTTP/3 setup timed out".to_string()))?
            .map_err(|e| connect_error(e.to_string()))?;
        let key = self.key.clone();
        let id = NEXT_CONNECTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        pool().lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone(), (id, sender.clone()));
        tokio::spawn(async move {
            let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
            // Closed (GOAWAY, idle timeout, error): the next request connects again.
            let mut pool = pool().lock().unwrap_or_else(|e| e.into_inner());
            if pool.get(&key).is_some_and(|(current, _)| *current == id) {
                pool.remove(&key);
            }
        });
        Ok(sender)
    }

    /// Return the head to the handler, then deliver the body and trailers.
    async fn respond(
        &self,
        mut stream: h3::client::RequestStream<h3_quinn::RecvStream, Bytes>,
        head_tx: &HeadSender,
    ) -> Result<(), H3Error> {
        let failed = |e: String| H3Error::Failed(format!("h3 proxy: {}: {e}", self.addr));
        let head = self.within(stream.recv_response()).await?.map_err(|e| failed(e.to_string()))?;
        let mut response = Response::new();
        response.status_code = head.status().as_u16() as i16;
        response.reason_phrase = head.status().canonical_reason().unwrap_or("").to_string();
        let content_type = header(head.headers(), "content-type").unwrap_or("application/octet-stream").to_string();
        let length = header(head.headers(), "content-length").and_then(|v| v.parse::<u64>().ok());
        let grpc = content_type.starts_with("application/grpc");
        for (name, value) in head.headers() {
            if NOT_RETURNED.contains(&name.as_str()) {
                continue;
            }
            if let Ok(value) = value.to_str() {
                response.headers.push(Header::new(name.as_str().to_string(), value.to_string()));
            }
        }
        let trailers = Trailers::new();

        if !grpc && length.is_some_and(|n| n <= BUFFER_LIMIT) {
            let mut bytes = Vec::with_capacity(length.unwrap_or(0) as usize);
            loop {
                match self.within(stream.recv_data()).await? {
                    Ok(Some(mut chunk)) => {
                        while chunk.has_remaining() {
                            let part = chunk.chunk();
                            bytes.extend_from_slice(part);
                            let n = part.len();
                            chunk.advance(n);
                        }
                    }
                    Ok(None) => break,
                    Err(e) => return Err(failed(format!("body: {e}"))),
                }
            }
            if let Ok(Ok(Some(fields))) = self.within(stream.recv_trailers()).await {
                trailers.set(header_list(&fields));
                response.trailers = Some(trailers);
            }
            if !bytes.is_empty() {
                response.content_range_list = vec![Range::get_content_range(bytes, content_type)];
            }
            let _ = head_tx.try_send(Ok(super::normalize_upstream_response(response)));
            return Ok(());
        }

        // Streamed: the writer reads the body as it arrives; an HTTP/1.1
        // client gets it chunked.
        response.trailers = Some(trailers.clone());
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
        let (tx, rx) = tokio::sync::mpsc::channel(BODY_CHANNEL);
        response.stream_pipe = Some(Box::new(crate::server::body_stream::ChannelReader::new(rx)));
        if head_tx.try_send(Ok(response)).is_err() {
            return Ok(()); // the handler gave up waiting
        }
        loop {
            let next = match self.within(stream.recv_data()).await {
                Ok(Ok(Some(mut chunk))) => Ok(chunk.copy_to_bytes(chunk.remaining()).to_vec()),
                Ok(Ok(None)) => break,
                Ok(Err(e)) => Err(std::io::Error::other(format!("h3 proxy: body from {}: {e}", self.addr))),
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e.to_string())),
            };
            let failed = next.is_err();
            if tx.send(next).await.is_err() || failed {
                return Ok(()); // the client went away, or the upstream failed: the reader reports it
            }
        }
        match self.within(stream.recv_trailers()).await {
            Ok(Ok(Some(fields))) => trailers.set(header_list(&fields)),
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                let _ = tx.send(Err(std::io::Error::other(format!("h3 proxy: trailers from {}: {e}", self.addr)))).await;
            }
            Err(e) => {
                let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e.to_string()))).await;
            }
        }
        Ok(())
    }

    /// Await `f`, failing after the read timeout — per wait, so a slow but
    /// progressing response isn't cut off.
    async fn within<F: std::future::Future>(&self, f: F) -> Result<F::Output, H3Error> {
        tokio::time::timeout(self.read_timeout, f)
            .await
            .map_err(|_| H3Error::Failed(format!("h3 proxy: {} sent nothing for {} ms", self.addr, self.read_timeout.as_millis())))
    }
}

fn header<'a>(headers: &'a http::HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn header_list(fields: &http::HeaderMap) -> Vec<Header> {
    fields
        .iter()
        .filter_map(|(name, value)| Some(Header::new(name.as_str().to_string(), value.to_str().ok()?.to_string())))
        .collect()
}

/// Send the request body as it becomes available, then finish the stream;
/// a body that fails to read stops the stream with an error.
async fn send_body(mut stream: h3::client::RequestStream<h3_quinn::SendStream<Bytes>, Bytes>, body: Body) {
    match body {
        Body::Bytes(bytes) => {
            if !bytes.is_empty() && stream.send_data(Bytes::from(bytes)).await.is_err() {
                return;
            }
        }
        Body::Stream(_) => {
            let mut chunks = read_in_background(body.into_reader());
            while let Some(chunk) = chunks.recv().await {
                match chunk {
                    Ok(data) => {
                        if stream.send_data(Bytes::from(data)).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        stream.stop_stream(h3::error::Code::H3_REQUEST_CANCELLED);
                        return;
                    }
                }
            }
        }
    }
    let _ = stream.finish().await;
}

/// Read a blocking body on the blocking pool, a chunk at a time.
fn read_in_background(mut reader: Box<dyn Read + Send>) -> tokio::sync::mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(BODY_CHANNEL);
    tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let next = match reader.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => Ok(buf[..n].to_vec()),
                Err(e) => Err(e),
            };
            let failed = next.is_err();
            if tx.blocking_send(next).is_err() || failed {
                return;
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests;
