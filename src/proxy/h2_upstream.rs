//! HTTP/2 upstreams: [`H2ReverseProxy`](super::H2ReverseProxy),
//! [`GrpcProxy`](super::GrpcProxy) and the config file's `grpc` action.
//!
//! Connections are pooled (one multiplexed connection per backend and trust
//! setting) and live on one process-wide runtime, `rws-h2-upstream`: tokio
//! I/O belongs to the runtime that made it, and the handler calling
//! [`forward`] may run under the shared runtime, a per-core one or none at
//! all (`TestClient`), so none of those can own them.
//!
//! Bodies stream both ways. The request body is sent as the handler's
//! [`Body`](crate::request::Body) yields it — a stream when the server
//! received it as one, so gRPC client streaming and bidirectional calls work —
//! and the response head is returned as soon as it arrives, with the body in
//! `stream_pipe` and the upstream's trailers (`grpc-status`, `grpc-message`)
//! in [`Response::trailers`], filled when the body ends. Small non-gRPC
//! responses with a known length are buffered instead, as the HTTP/1.1
//! forwarder does.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bytes::Bytes;

use super::Backend;
use crate::header::Header;
use crate::range::Range;
use crate::request::{Body, Request};
use crate::core::New;
use crate::response::{Response, Trailers};

/// Responses up to this size with a `content-length` (and not gRPC) are
/// buffered; larger ones, and every gRPC response, stream.
const BUFFER_LIMIT: u64 = 1 << 20;

/// Upstream body chunks in flight between the connection and the writer.
const BODY_CHANNEL: usize = 4;

/// Request headers that describe a single connection and aren't forwarded
/// (`te: trailers` is, separately — gRPC servers require it).
const NOT_FORWARDED: &[&str] = &[
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "proxy-connection",
    "te", "trailer", "transfer-encoding", "upgrade", "host",
];

/// Response headers HTTP/2 forbids or rws regenerates.
const NOT_RETURNED: &[&str] = &["connection", "keep-alive", "transfer-encoding", "upgrade", "proxy-connection", "te"];

/// Where to send a request: a backend and, for TLS, the client configuration
/// (its ALPN must offer `h2`).
pub(crate) struct Target<'a> {
    pub(super) backend: &'a Backend,
    pub(super) tls: Option<&'a Arc<rustls::ClientConfig>>,
    pub(super) connect_timeout: Duration,
    pub(super) read_timeout: Duration,
    /// `:authority` to send instead of the backend's address (the Gateway
    /// API router keeps the client's `Host`).
    pub(super) authority: Option<&'a str>,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rws-h2-upstream")
            .enable_all()
            .build()
            .expect("the HTTP/2 upstream runtime")
    })
}

/// Pooled connections by key, each with an id so a closing connection
/// removes itself and not a newer one under the same key.
type Pool = Mutex<HashMap<String, (u64, h2::client::SendRequest<Bytes>)>>;

static NEXT_CONNECTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(Default::default)
}

/// The client configuration rws uses for `h2s://` / `grpcs://` backends
/// without a `tls_ca_file`: the WebPKI roots, ALPN `h2`.
pub(crate) fn default_tls() -> &'static Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(with_h2_alpn(crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth()))
    })
}

/// `config` offering only HTTP/2 by ALPN.
pub(crate) fn with_h2_alpn(mut config: rustls::ClientConfig) -> rustls::ClientConfig {
    config.alpn_protocols = vec![b"h2".to_vec()];
    config
}

/// Forward `request` and return the response head — its body streaming in
/// `stream_pipe` unless it was small enough to buffer. Errors before the
/// head arrives come back as `Err` (the caller answers 502 or tries another
/// backend); errors after it end the body with an error, which the HTTP/2 and
/// HTTP/3 writers turn into a stream reset.
pub(crate) fn forward(request: &Request, client_ip: &str, target: &Target<'_>) -> Result<Response, String> {
    let backend = target.backend;
    let addr = format!("{}:{}", backend.host, backend.port);
    let head = request_head(request, client_ip, target.authority.unwrap_or(&addr), target.tls.is_some())?;
    let key = pool_key(&addr, target.tls);
    let (head_tx, head_rx) = std::sync::mpsc::sync_channel(1);
    let exchange = Exchange {
        key,
        addr,
        host: backend.host.clone(),
        tls: target.tls.cloned(),
        connect_timeout: target.connect_timeout,
        read_timeout: target.read_timeout,
    };
    let body = request.body.clone();
    runtime().spawn(async move {
        let result = exchange.run(head, body, &head_tx).await;
        if let Err(e) = result {
            // Only reaches the caller if the head wasn't sent yet.
            let _ = head_tx.try_send(Err(e));
        }
    });
    let wait = target.connect_timeout + target.read_timeout + Duration::from_secs(1);
    match head_rx.recv_timeout(wait) {
        Ok(result) => result,
        Err(_) => Err(format!("h2 proxy: no response head within {} ms", wait.as_millis())),
    }
}

/// One pool entry per backend address and trust setting (a `tls_ca_file`
/// upstream doesn't share connections with a WebPKI one).
fn pool_key(addr: &str, tls: Option<&Arc<rustls::ClientConfig>>) -> String {
    match tls {
        None => format!("h2c|{addr}"),
        Some(config) => format!("h2s:{:x}|{addr}", Arc::as_ptr(config) as usize),
    }
}

fn request_head(request: &Request, client_ip: &str, addr: &str, tls: bool) -> Result<http::Request<()>, String> {
    let scheme = if tls { "https" } else { "http" };
    let uri: http::Uri = format!("{scheme}://{addr}{}", request.request_uri).parse().map_err(|e: http::uri::InvalidUri| e.to_string())?;
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
    builder = builder.header("x-forwarded-for", client_ip).header("via", "2 rws");
    builder.body(()).map_err(|e| e.to_string())
}

type HeadSender = std::sync::mpsc::SyncSender<Result<Response, String>>;

struct Exchange {
    key: String,
    addr: String,
    host: String,
    tls: Option<Arc<rustls::ClientConfig>>,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl Exchange {
    async fn run(self, head: http::Request<()>, body: Body, head_tx: &HeadSender) -> Result<(), String> {
        let end_of_stream = matches!(&body, Body::Bytes(bytes) if bytes.is_empty());
        let (response, send_stream) = self.send(head, end_of_stream).await?;
        if !end_of_stream {
            tokio::spawn(send_body(send_stream, body));
        }
        let response = self.within(response).await?.map_err(|e| format!("h2 proxy: {} answered with an error: {e}", self.addr))?;
        self.respond(response, head_tx).await;
        Ok(())
    }

    /// Send the request head on a pooled connection, or a new one when the
    /// pooled one is gone. A request is retried only before it was sent.
    async fn send(
        &self,
        head: http::Request<()>,
        end_of_stream: bool,
    ) -> Result<(h2::client::ResponseFuture, h2::SendStream<Bytes>), String> {
        let pooled = pool().lock().unwrap_or_else(|e| e.into_inner()).get(&self.key).map(|(_, s)| s.clone());
        let ready = match pooled {
            Some(sender) => match self.within(sender.ready()).await? {
                Ok(ready) => ready,
                Err(_) => {
                    pool().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
                    self.connect().await?
                }
            },
            None => self.connect().await?,
        };
        let mut ready = ready;
        ready.send_request(head, end_of_stream).map_err(|e| format!("h2 proxy: sending to {} failed: {e}", self.addr))
    }

    async fn connect(&self) -> Result<h2::client::SendRequest<Bytes>, String> {
        let tcp = tokio::time::timeout(self.connect_timeout, tokio::net::TcpStream::connect(&self.addr))
            .await
            .map_err(|_| format!("h2 proxy: connect to {} timed out", self.addr))?
            .map_err(|e| format!("h2 proxy: connect to {} failed: {e}", self.addr))?;
        let _ = tcp.set_nodelay(true);
        let sender = match &self.tls {
            None => self.handshake(tcp).await?,
            Some(config) => {
                let name = rustls::pki_types::ServerName::try_from(self.host.clone())
                    .map_err(|e| format!("invalid upstream hostname '{}': {e}", self.host))?;
                let tls = self
                    .within(tokio_rustls::TlsConnector::from(Arc::clone(config)).connect(name, tcp))
                    .await?
                    .map_err(|e| format!("h2 proxy: TLS handshake with {} failed: {e}", self.addr))?;
                self.handshake(tls).await?
            }
        };
        let ready = self.within(sender.clone().ready()).await?.map_err(|e| format!("h2 proxy: {} refused: {e}", self.addr))?;
        Ok(ready)
    }

    async fn handshake<T>(&self, io: T) -> Result<h2::client::SendRequest<Bytes>, String>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (sender, connection) = self
            .within(h2::client::handshake(io))
            .await?
            .map_err(|e| format!("h2 proxy: handshake with {} failed: {e}", self.addr))?;
        let key = self.key.clone();
        let id = NEXT_CONNECTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        pool().lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone(), (id, sender.clone()));
        tokio::spawn(async move {
            let _ = connection.await;
            // Closed (GOAWAY, error, idle close): the next request connects again.
            let mut pool = pool().lock().unwrap_or_else(|e| e.into_inner());
            if pool.get(&key).is_some_and(|(current, _)| *current == id) {
                pool.remove(&key);
            }
        });
        Ok(sender)
    }

    /// Return the head to the handler, then deliver the body and trailers.
    async fn respond(&self, upstream: http::Response<h2::RecvStream>, head_tx: &HeadSender) {
        let (parts, mut body) = upstream.into_parts();
        let mut response = Response::new();
        response.status_code = parts.status.as_u16() as i16;
        response.reason_phrase = parts.status.canonical_reason().unwrap_or("").to_string();
        let content_type = header(&parts.headers, "content-type").unwrap_or("application/octet-stream").to_string();
        let length = header(&parts.headers, "content-length").and_then(|v| v.parse::<u64>().ok());
        let grpc = content_type.starts_with("application/grpc");
        for (name, value) in &parts.headers {
            if NOT_RETURNED.contains(&name.as_str()) {
                continue;
            }
            if let Ok(value) = value.to_str() {
                response.headers.push(Header::new(name.as_str().to_string(), value.to_string()));
            }
        }
        // Headers only, the stream already ended (a gRPC trailers-only
        // error): no body, so the client's HEADERS end the stream too — gRPC
        // clients read the status from them only then.
        if body.is_end_stream() {
            response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
            return send_head(head_tx, Ok(response));
        }
        let trailers = Trailers::new();
        response.trailers = Some(trailers.clone());

        if !grpc && length.is_some_and(|n| n <= BUFFER_LIMIT) {
            let mut bytes = Vec::with_capacity(length.unwrap_or(0) as usize);
            loop {
                match self.within(body.data()).await {
                    Ok(Some(Ok(chunk))) => {
                        let _ = body.flow_control().release_capacity(chunk.len());
                        bytes.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Ok(Some(Err(e))) => return send_head(head_tx, Err(format!("h2 proxy: body from {}: {e}", self.addr))),
                    Err(e) => return send_head(head_tx, Err(e)),
                }
            }
            if let Ok(Ok(Some(fields))) = self.within(body.trailers()).await {
                trailers.set(header_list(&fields));
            }
            if !bytes.is_empty() {
                response.content_range_list = vec![Range::get_content_range(bytes, content_type)];
            }
            return send_head(head_tx, Ok(super::normalize_upstream_response(response)));
        }

        // Streamed: the writer reads the body as it arrives. Its length isn't
        // repeated: an HTTP/1.1 client gets it chunked.
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
        let (tx, rx) = tokio::sync::mpsc::channel(BODY_CHANNEL);
        response.stream_pipe = Some(Box::new(crate::server::body_stream::ChannelReader::new(rx)));
        if head_tx.try_send(Ok(response)).is_err() {
            return; // the handler gave up waiting
        }
        loop {
            let next = match self.within(body.data()).await {
                Ok(Some(Ok(chunk))) => {
                    let _ = body.flow_control().release_capacity(chunk.len());
                    Ok(chunk.to_vec())
                }
                Ok(None) => break,
                Ok(Some(Err(e))) => Err(std::io::Error::other(format!("h2 proxy: body from {}: {e}", self.addr))),
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e)),
            };
            let failed = next.is_err();
            if tx.send(next).await.is_err() || failed {
                return; // the client went away, or the upstream failed: the reader reports it
            }
        }
        match self.within(body.trailers()).await {
            Ok(Ok(Some(fields))) => trailers.set(header_list(&fields)),
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                let _ = tx.send(Err(std::io::Error::other(format!("h2 proxy: trailers from {}: {e}", self.addr)))).await;
            }
            Err(e) => {
                let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e))).await;
            }
        }
        // Dropping `tx` ends the body; the trailers are set by then.
    }

    /// Await `f`, failing after the read timeout — applied to each wait, so a
    /// slow but progressing response (or a quiet gRPC stream shorter than the
    /// timeout) isn't cut off.
    async fn within<F: std::future::Future>(&self, f: F) -> Result<F::Output, String> {
        tokio::time::timeout(self.read_timeout, f)
            .await
            .map_err(|_| format!("h2 proxy: {} sent nothing for {} ms", self.addr, self.read_timeout.as_millis()))
    }
}

fn send_head(head_tx: &HeadSender, result: Result<Response, String>) {
    let _ = head_tx.try_send(result);
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

/// Send the request body as it becomes available; a body that fails to read
/// (the client reset its stream) resets the upstream stream.
async fn send_body(mut stream: h2::SendStream<Bytes>, body: Body) {
    let mut chunks = match body {
        Body::Bytes(bytes) => {
            let _ = send_data(&mut stream, Bytes::from(bytes), true).await;
            return;
        }
        Body::Stream(_) => read_in_background(body.into_reader()),
    };
    while let Some(chunk) = chunks.recv().await {
        match chunk {
            Ok(data) => {
                if send_data(&mut stream, Bytes::from(data), false).await.is_err() {
                    return;
                }
            }
            Err(_) => {
                stream.send_reset(h2::Reason::CANCEL);
                return;
            }
        }
    }
    let _ = stream.send_data(Bytes::new(), true);
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

/// `data` within the upstream's flow-control window.
async fn send_data(stream: &mut h2::SendStream<Bytes>, mut data: Bytes, end: bool) -> Result<(), h2::Error> {
    if data.is_empty() {
        return if end { stream.send_data(Bytes::new(), true) } else { Ok(()) };
    }
    while !data.is_empty() {
        stream.reserve_capacity(data.len());
        let granted = match std::future::poll_fn(|cx| stream.poll_capacity(cx)).await {
            Some(granted) => granted?,
            None => return Err(h2::Error::from(h2::Reason::CANCEL)),
        };
        if granted == 0 {
            continue;
        }
        let chunk = data.split_to(granted.min(data.len()));
        stream.send_data(chunk, end && data.is_empty())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
