//! Tower interoperability (`tower` feature): adapters between rws's own
//! [`Application`]/[`Middleware`](crate::middleware::Middleware) and the
//! [Tower](https://docs.rs/tower) `Service`/`Layer` traits, so tonic gRPC
//! services and tower-http layers work in an rws app, and an rws app works in
//! a Tower stack. rws itself doesn't run on Tower: its request path stays the
//! native one, and nothing here is compiled without the feature.
//!
//! - [`TowerApp`] / [`mount`] — a Tower service (a tonic server, a
//!   tower-http `ServeDir`, an axum `Router`) as an rws [`Application`].
//! - [`TowerLayer`] — a Tower layer (tower-http's `SetResponseHeader`,
//!   `ValidateRequestHeader`, `Timeout`, …) as rws middleware.
//! - [`TowerService`] — an rws app as a Tower service, for hyper or axum.
//!
//! Requests and responses cross as `http::Request<Body>` /
//! `http::Response<Body>` ([`Body`] implements `http_body::Body`).
//!
//! Tower futures run on one process-wide runtime, `rws-tower` (2 threads):
//! tokio I/O and timers belong to the runtime that made them, and an rws
//! handler may run under the shared runtime, a per-core one or none at all.
//! Its threads have no trace context, so automatic client spans
//! ([`crate::otel`]) don't nest under requests a Tower service answers.

#[cfg(test)]
mod tests;

mod layer;
mod service;

pub use layer::{Next, TowerLayer};
pub use service::TowerService;

use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;
use http_body_util::BodyExt;

use crate::application::{Application, AsyncResponse};
use crate::core::New;
use crate::header::Header;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, Trailers};
use crate::server::ConnectionInfo;

/// The error type bodies and services are converted to.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Bodies up to this size with a known length are collected (not streamed)
/// when they come back from a Tower service, unless they're gRPC.
const BUFFER_LIMIT: u64 = 1024 * 1024;

/// Bytes per chunk when an rws body (a file, a pipe, a streamed request) is
/// read into a [`Body`].
const CHUNK: usize = 64 * 1024;

/// Hop-by-hop and framing headers rws regenerates; never copied across.
const NOT_COPIED: &[&str] = &["connection", "keep-alive", "transfer-encoding", "upgrade", "proxy-connection", "te", "content-length"];

pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rws-tower")
            .enable_all()
            .build()
            .expect("the tower runtime")
    })
}

// ── Body ──────────────────────────────────────────────────────────────────────

/// The `http_body::Body` rws hands to Tower and gets back from its own
/// handlers: bytes in one frame, or chunks (and trailers) from a channel.
pub struct Body {
    inner: Inner,
}

enum Inner {
    Full(Option<Bytes>),
    Channel { rx: tokio::sync::mpsc::Receiver<std::io::Result<Bytes>>, trailers: Option<Trailers> },
}

impl Body {
    /// No bytes.
    pub fn empty() -> Body {
        Body { inner: Inner::Full(None) }
    }

    /// `bytes` in one frame.
    pub fn full(bytes: impl Into<Bytes>) -> Body {
        let bytes = bytes.into();
        Body { inner: Inner::Full((!bytes.is_empty()).then_some(bytes)) }
    }

    /// Chunks read from `reader` on the `rws-tower` blocking pool; `trailers`,
    /// when given, are sent after the last chunk (read once the reader ends).
    fn from_reader(mut reader: Box<dyn Read + Send>, trailers: Option<Trailers>) -> Body {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        runtime().spawn_blocking(move || loop {
            let mut buf = vec![0u8; CHUNK];
            match reader.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => {
                    buf.truncate(n);
                    if tx.blocking_send(Ok(Bytes::from(buf))).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e));
                    return;
                }
            }
        });
        Body { inner: Inner::Channel { rx, trailers } }
    }

    /// An rws request body: bytes as they are, a stream read as it arrives.
    pub fn from_request_body(body: crate::request::Body) -> Body {
        if body.is_stream() {
            Body::from_reader(body.into_reader(), None)
        } else {
            Body::full(body.to_vec())
        }
    }
}

impl Default for Body {
    fn default() -> Body {
        Body::empty()
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        match &mut self.get_mut().inner {
            Inner::Full(bytes) => Poll::Ready(bytes.take().map(|b| Ok(Frame::data(b)))),
            Inner::Channel { rx, trailers } => match rx.poll_recv(cx) {
                Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(Frame::data(chunk)))),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(Box::new(e)))),
                Poll::Ready(None) => {
                    let fields = trailers.take().map(|t| t.get()).unwrap_or_default();
                    if fields.is_empty() {
                        return Poll::Ready(None);
                    }
                    let mut map = http::HeaderMap::new();
                    for h in fields {
                        if let (Ok(name), Ok(value)) = (http::HeaderName::from_bytes(h.name.as_bytes()), http::HeaderValue::from_bytes(h.value.as_bytes())) {
                            map.append(name, value);
                        }
                    }
                    Poll::Ready(Some(Ok(Frame::trailers(map))))
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }

    fn is_end_stream(&self) -> bool {
        matches!(self.inner, Inner::Full(None))
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match &self.inner {
            Inner::Full(bytes) => http_body::SizeHint::with_exact(bytes.as_ref().map_or(0, |b| b.len() as u64)),
            Inner::Channel { .. } => http_body::SizeHint::default(),
        }
    }
}

// ── conversions ───────────────────────────────────────────────────────────────

fn version_to_http(version: &str) -> http::Version {
    match version {
        "HTTP/1.0" => http::Version::HTTP_10,
        "HTTP/2.0" | "HTTP/2" => http::Version::HTTP_2,
        "HTTP/3.0" | "HTTP/3" => http::Version::HTTP_3,
        "HTTP/0.9" => http::Version::HTTP_09,
        _ => http::Version::HTTP_11,
    }
}

fn version_from_http(version: http::Version) -> &'static str {
    let v = &crate::http::VERSION;
    match version {
        http::Version::HTTP_10 => v.http_1_0,
        http::Version::HTTP_2 => v.http_2_0,
        http::Version::HTTP_3 => v.http_3_0,
        _ => v.http_1_1,
    }
}

fn header_map(headers: &[Header]) -> http::HeaderMap {
    let mut map = http::HeaderMap::with_capacity(headers.len());
    for h in headers {
        // Anything `http` refuses is left out rather than failing the request.
        if let (Ok(name), Ok(value)) = (http::HeaderName::from_bytes(h.name.as_bytes()), http::HeaderValue::from_bytes(h.value.as_bytes())) {
            map.append(name, value);
        }
    }
    map
}

fn headers_from(map: &http::HeaderMap) -> Vec<Header> {
    map.iter()
        .filter_map(|(name, value)| {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            Some(Header::new(name.as_str().to_string(), value))
        })
        .collect()
}

/// An rws request as an `http::Request` (body as given).
pub fn to_http_request(request: Request) -> http::Request<Body> {
    let mut builder = http::Request::builder()
        .method(http::Method::from_bytes(request.method.as_bytes()).unwrap_or(http::Method::GET))
        .uri(http::Uri::try_from(request.request_uri.as_str()).unwrap_or_else(|_| http::Uri::from_static("/")))
        .version(version_to_http(&request.http_version));
    if let Some(headers) = builder.headers_mut() {
        *headers = header_map(&request.headers);
    }
    builder.body(Body::from_request_body(request.body)).expect("method, URI and version are valid")
}

/// An `http::Request` as an rws request, its body read into memory.
pub async fn from_http_request<B>(request: http::Request<B>) -> Result<Request, BoxError>
where
    B: http_body::Body<Data = Bytes>,
    B::Error: Into<BoxError>,
{
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await.map_err(Into::into)?.to_bytes();
    Ok(Request {
        method: parts.method.as_str().to_string(),
        request_uri: parts.uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".to_string()),
        http_version: version_from_http(parts.version).to_string(),
        headers: headers_from(&parts.headers),
        body: bytes.to_vec().into(),
    })
}

/// An rws response as an `http::Response`: buffered bodies in one frame,
/// files and pipes streamed (read on the `rws-tower` blocking pool), with
/// `Response::trailers` after them.
pub fn to_http_response(mut response: Response) -> http::Response<Body> {
    let mut headers = header_map(&response.headers);
    let body = if response.stream_pipe.is_some() || response.stream_file.is_some() {
        let _enter = runtime().enter();
        match crate::server::body_stream::spawn_producer(&mut response) {
            Some(mut chunks) => {
                let (tx, rx) = tokio::sync::mpsc::channel(4);
                runtime().spawn(async move {
                    while let Some(chunk) = chunks.recv().await {
                        let chunk = chunk.map(Bytes::from).map_err(std::io::Error::other);
                        let failed = chunk.is_err();
                        if tx.send(chunk).await.is_err() || failed {
                            return;
                        }
                    }
                });
                Body { inner: Inner::Channel { rx, trailers: response.trailers.take() } }
            }
            None => Body::empty(),
        }
    } else {
        let ranges = std::mem::take(&mut response.content_range_list);
        let (content_type, bytes) = match ranges.len() {
            0 => (None, Vec::new()),
            1 => {
                if let Some(value) = crate::response::content_range_header_value(response.status_code, &ranges[0]) {
                    if let Ok(value) = http::HeaderValue::from_str(&value) {
                        headers.insert(http::header::CONTENT_RANGE, value);
                    }
                }
                let range = ranges.into_iter().next().expect("one range");
                (Some(range.content_type), range.body)
            }
            _ => (Some(Range::multipart_content_type(&ranges)), Response::generate_body(ranges)),
        };
        if let Some(content_type) = content_type.and_then(|t| http::HeaderValue::from_str(&t).ok()) {
            headers.insert(http::header::CONTENT_TYPE, content_type);
        }
        match response.trailers.take() {
            // Trailers on a buffered body: a one-chunk channel carries them.
            Some(trailers) if !trailers.get().is_empty() => {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let _ = tx.try_send(Ok(Bytes::from(bytes)));
                Body { inner: Inner::Channel { rx, trailers: Some(trailers) } }
            }
            _ => Body::full(bytes),
        }
    };
    let mut out = http::Response::new(body);
    *out.status_mut() = http::StatusCode::from_u16(response.status_code as u16).unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
    *out.headers_mut() = headers;
    out
}

/// An `http::Response` as an rws response. A body that's gRPC, or of unknown
/// or large size, streams through `stream_pipe` (polled on `rws-tower`),
/// its trailers in `Response::trailers`; small ones are collected.
pub async fn from_http_response<B>(response: http::Response<B>) -> Result<Response, BoxError>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let (parts, body) = response.into_parts();
    let mut response = Response::new();
    response.status_code = parts.status.as_u16() as i16;
    response.reason_phrase = parts.status.canonical_reason().unwrap_or("").to_string();
    let content_type = parts.headers.get(http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_string);
    let grpc = content_type.as_deref().is_some_and(|t| t.starts_with("application/grpc"));
    response.headers = headers_from(&parts.headers)
        .into_iter()
        .filter(|h| !NOT_COPIED.iter().any(|n| h.name.eq_ignore_ascii_case(n)) && !h.name.eq_ignore_ascii_case("content-type"))
        .collect();
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // A gRPC trailers-only answer, a 204, a HEAD: headers only.
    if body.is_end_stream() {
        if grpc {
            response.headers.push(Header::new("content-type".to_string(), content_type));
        }
        return Ok(response);
    }
    let exact = body.size_hint().exact();
    if !grpc && exact.is_some_and(|n| n <= BUFFER_LIMIT) {
        let collected = body.collect().await.map_err(Into::into)?;
        if let Some(trailers) = collected.trailers() {
            response.trailers = Some(Trailers::with(headers_from(trailers)));
        }
        let bytes = collected.to_bytes();
        if !bytes.is_empty() {
            response.content_range_list = vec![Range::get_content_range(bytes.to_vec(), content_type)];
        }
        return Ok(response);
    }

    // Streamed. The content type goes in the headers: a stream has no
    // `ContentRange` to carry it.
    response.headers.push(Header::new("content-type".to_string(), content_type));
    let trailers = Trailers::new();
    response.trailers = Some(trailers.clone());
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    response.stream_pipe = Some(Box::new(crate::server::body_stream::ChannelReader::new(rx)));
    runtime().spawn(async move {
        let mut body = std::pin::pin!(body);
        loop {
            // The body's own error type needn't be `Send`; don't hold it across an await.
            let next: Option<Result<Frame<Bytes>, BoxError>> = body.frame().await.map(|r| r.map_err(Into::into));
            match next {
                Some(Ok(frame)) => {
                    let frame = match frame.into_data() {
                        Ok(data) => {
                            if tx.send(Ok(data.to_vec())).await.is_err() {
                                return; // the client went away
                            }
                            continue;
                        }
                        Err(frame) => frame,
                    };
                    if let Ok(fields) = frame.into_trailers() {
                        // Before the channel closes: the writer reads them at the end.
                        trailers.set(headers_from(&fields));
                    }
                }
                Some(Err(e)) => {
                    let _ = tx.send(Err(std::io::Error::other(e))).await;
                    return;
                }
                None => return,
            }
        }
    });
    Ok(response)
}

/// `poll_ready` then `call`.
async fn call<S, Req>(mut service: S, request: Req) -> Result<S::Response, BoxError>
where
    S: tower_service::Service<Req>,
    S::Error: Into<BoxError>,
{
    std::future::poll_fn(|cx| service.poll_ready(cx)).await.map_err(Into::into)?;
    service.call(request).await.map_err(Into::into)
}

// ── TowerApp ──────────────────────────────────────────────────────────────────

/// A Tower service as an rws [`Application`]: every request goes to it.
/// [`mount`] puts one under a path prefix in front of another app.
///
/// The request body reaches the service as it arrives (a gRPC client or
/// bidirectional stream works); the response streams back with its trailers
/// (`grpc-status`). The service runs on the `rws-tower` runtime.
///
/// ```rust,ignore
/// // A tonic service, e.g. tonic-health's:
/// let (_, health) = tonic_health::server::health_reporter();
/// let app = rust_web_server::tower::mount("/grpc.health.v1.Health/", health, App::new());
/// ```
#[derive(Clone)]
pub struct TowerApp<S> {
    service: S,
}

impl<S> TowerApp<S> {
    pub fn new(service: S) -> TowerApp<S> {
        TowerApp { service }
    }
}

/// Requests whose path starts with `prefix` go to `service`, the rest to
/// `fallback`.
pub fn mount<S, A>(prefix: &str, service: S, fallback: A) -> Mount<S, A> {
    Mount { prefix: prefix.to_string(), service: TowerApp::new(service), fallback }
}

/// What [`mount`] returns.
#[derive(Clone)]
pub struct Mount<S, A> {
    prefix: String,
    service: TowerApp<S>,
    fallback: A,
}

impl<S, A> Mount<S, A> {
    fn routed(&self, request: &Request) -> bool {
        crate::url::URL::request_target_path(&request.request_uri).unwrap_or("").starts_with(&self.prefix)
    }
}

impl<S, B> TowerApp<S>
where
    S: tower_service::Service<http::Request<Body>, Response = http::Response<B>> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn respond(&self, request: Request) -> impl Future<Output = Result<Response, String>> + Send + 'static {
        let service = self.service.clone();
        async move {
            let response = call(service, to_http_request(request)).await.map_err(|e| format!("tower service: {e}"))?;
            from_http_response(response).await.map_err(|e| format!("tower service body: {e}"))
        }
    }
}

impl<S, B> Application for TowerApp<S>
where
    S: tower_service::Service<http::Request<Body>, Response = http::Response<B>> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let work = self.respond(request.clone());
        runtime().spawn(async move {
            let _ = tx.send(work.await);
        });
        rx.recv().map_err(|_| "tower service: dropped".to_string())?
    }

    fn execute_async<'a>(&'a self, request: &'a Request, _connection: &'a ConnectionInfo) -> Option<AsyncResponse<'a>> {
        let work = runtime().spawn(self.respond(request.clone()));
        Some(Box::pin(async move { work.await.map_err(|e| format!("tower service: {e}"))? }))
    }

    fn streams_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

impl<S, B, A> Application for Mount<S, A>
where
    S: tower_service::Service<http::Request<Body>, Response = http::Response<B>> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
    A: Application,
{
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if self.routed(request) {
            self.service.execute(request, connection)
        } else {
            self.fallback.execute(request, connection)
        }
    }

    fn runs_inline(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        !self.routed(request) && self.fallback.runs_inline(request, connection)
    }

    fn execute_async<'a>(&'a self, request: &'a Request, connection: &'a ConnectionInfo) -> Option<AsyncResponse<'a>> {
        if self.routed(request) {
            self.service.execute_async(request, connection)
        } else {
            self.fallback.execute_async(request, connection)
        }
    }

    fn streams_body(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        self.routed(request) || self.fallback.streams_body(request, connection)
    }
}

// ── blocking on a borrowed future ─────────────────────────────────────────────

/// Drive `future` to completion on this thread (it may borrow from the
/// caller, so it can't be spawned), inside the `rws-tower` runtime's context
/// so timers and spawns inside it have a driver.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let _enter = runtime().enter();
    let waker = std::task::Waker::from(std::sync::Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}
