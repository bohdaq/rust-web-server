//! OpenTelemetry-compatible distributed tracing.
//!
//! [`OtelLayer`] is a [`Middleware`] that:
//! 1. Reads the W3C `traceparent` header from incoming requests and continues
//!    an existing trace, or starts a fresh one.
//! 2. Creates an HTTP server span with standard semantic attributes.
//! 3. Stores the active span context in thread-local storage so downstream
//!    middleware (e.g. [`crate::proxy::ReverseProxy`]) can propagate it.
//! 4. Records the completed span to a configurable exporter.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::otel::{OtelLayer, TracingConfig, ExporterConfig};
//!
//! // Dev: print spans to stdout.
//! rust_web_server::otel::setup(TracingConfig {
//!     service_name: "my-service".to_string(),
//!     service_version: env!("CARGO_PKG_VERSION").to_string(),
//!     exporter: ExporterConfig::Stdout,
//!     sample_rate: 1.0,
//!     batch_size: 128,
//! });
//!
//! let app = App::new().wrap(OtelLayer);
//! ```
//!
//! # Production: OTLP HTTP export
//!
//! ```rust,no_run
//! use rust_web_server::otel::{ExporterConfig, TracingConfig};
//!
//! rust_web_server::otel::setup(TracingConfig {
//!     service_name: "my-service".to_string(),
//!     service_version: "1.0.0".to_string(),
//!     exporter: ExporterConfig::Otlp {
//!         endpoint: "http://localhost:4318".to_string(),
//!     },
//!     sample_rate: 0.1,
//!     batch_size: 512,
//! });
//! ```
//!
//! Alternatively, set environment variables before calling [`setup_from_env`]:
//!
//! ```text
//! OTEL_SERVICE_NAME=my-service
//! OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
//! OTEL_TRACES_SAMPLER_ARG=0.1    # sample rate 0.0–1.0 (default 1.0)
//! ```

#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

// ── ID generation ─────────────────────────────────────────────────────────────

static COUNTER: AtomicU64 = AtomicU64::new(1);
static START_SECS: OnceLock<u64> = OnceLock::new();

fn start_secs() -> u64 {
    *START_SECS.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    })
}

/// Generate a new 128-bit trace ID. Not cryptographically random but unique
/// enough for tracing purposes across service restarts.
pub fn new_trace_id() -> [u8; 16] {
    let start = start_secs();
    let seq = COUNTER.fetch_add(2, Ordering::Relaxed);
    let mut id = [0u8; 16];
    id[..8].copy_from_slice(&start.to_be_bytes());
    id[8..].copy_from_slice(&seq.to_be_bytes());
    id
}

/// Generate a new 64-bit span ID.
pub fn new_span_id() -> [u8; 8] {
    let seq = COUNTER.fetch_add(2, Ordering::Relaxed) + 1;
    seq.to_be_bytes()
}

fn hex16(b: &[u8; 16]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn hex8(b: &[u8; 8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

// ── W3C Trace Context ─────────────────────────────────────────────────────────

/// Parsed W3C `traceparent` header value.
///
/// Format: `00-{trace-id}-{parent-id}-{flags}`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: [u8; 16],
    pub parent_span_id: [u8; 8],
    pub sampled: bool,
}

impl TraceContext {
    /// Parse a `traceparent` header value.
    pub fn parse(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.trim().splitn(4, '-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }
        let trace_id = parse_hex16(parts[1])?;
        let parent_span_id = parse_hex8(parts[2])?;
        let flags = u8::from_str_radix(parts[3], 16).ok()?;
        Some(TraceContext { trace_id, parent_span_id, sampled: flags & 0x01 != 0 })
    }

    /// Render as a `traceparent` header value for this context acting as parent.
    pub fn as_header(&self, span_id: &[u8; 8]) -> String {
        format!("00-{}-{}-{:02x}", hex16(&self.trace_id), hex8(span_id), self.sampled as u8)
    }
}

fn parse_hex16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 { return None; }
    let mut out = [0u8; 16];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

fn parse_hex8(s: &str) -> Option<[u8; 8]> {
    if s.len() != 16 { return None; }
    let mut out = [0u8; 8];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

// ── span kind / attributes ───────────────────────────────────────────────────

/// OTLP `SpanKind`. Numbering matches the OTLP spec (`INTERNAL`=1, `SERVER`=2,
/// `CLIENT`=3) so the numeric value can be cast directly with `as i32` when
/// building OTLP JSON. `PRODUCER`/`CONSUMER` are intentionally not exposed —
/// nothing in this crate does message-queue instrumentation yet.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SpanKind {
    /// Internal work with no remote counterpart — DB query, cache lookup,
    /// business logic. The default for [`span`].
    #[default]
    Internal = 1,
    /// The span created by [`OtelLayer`] for an incoming HTTP request.
    Server = 2,
    /// An outbound call to another service. The kind for [`client_span`].
    Client = 3,
}

/// A span attribute value — matches OTLP `AnyValue`'s basic variants.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self { AttributeValue::String(s.to_string()) }
}
impl From<String> for AttributeValue {
    fn from(s: String) -> Self { AttributeValue::String(s) }
}
impl From<i64> for AttributeValue {
    fn from(v: i64) -> Self { AttributeValue::Int(v) }
}
impl From<i32> for AttributeValue {
    fn from(v: i32) -> Self { AttributeValue::Int(v as i64) }
}
impl From<u32> for AttributeValue {
    fn from(v: u32) -> Self { AttributeValue::Int(v as i64) }
}
impl From<f64> for AttributeValue {
    fn from(v: f64) -> Self { AttributeValue::Float(v) }
}
impl From<bool> for AttributeValue {
    fn from(v: bool) -> Self { AttributeValue::Bool(v) }
}

// ── span data ─────────────────────────────────────────────────────────────────

/// A completed span ready for export.
#[derive(Debug, Clone, Default)]
pub struct SpanData {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    /// `"GET /api/users"` — method + path, query stripped.
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
    /// Empty for a non-HTTP span (e.g. a `db.query` child span) — exporters
    /// omit the `http.*` attributes entirely in that case.
    pub http_method: String,
    pub http_target: String,
    pub http_status: i16,
    /// 0=Unset, 1=Ok, 2=Error
    pub status_code: u8,
    pub kind: SpanKind,
    /// Extra key/value attributes beyond the first-class `http.*` fields —
    /// e.g. `db.statement`, `cache.key`, or any custom attribute set via
    /// [`Span::set_attribute`].
    pub attributes: Vec<(String, AttributeValue)>,
}

impl SpanData {
    fn duration_ms(&self) -> f64 {
        (self.end_ns.saturating_sub(self.start_ns)) as f64 / 1_000_000.0
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn strip_query(uri: &str) -> &str {
    match uri.find('?') {
        Some(i) => &uri[..i],
        None => uri,
    }
}

// ── thread-local span stack ─────────────────────────────────────────────────

/// Compact span context stored on the thread-local stack for downstream
/// propagation and parent/child nesting.
#[derive(Copy, Clone)]
struct ActiveSpanCtx {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    sampled: bool,
}

thread_local! {
    /// One entry per currently-open [`Span`] on this thread, innermost last.
    /// A single-slot cell can't represent nesting (span A active, starts
    /// span B, B is now "current", B ends, A is "current" again) — this
    /// stack is what makes multiple nested spans per request possible.
    static ACTIVE_STACK: RefCell<Vec<ActiveSpanCtx>> = const { RefCell::new(Vec::new()) };
}

/// Return the W3C `traceparent` value for the innermost span currently being
/// processed on this thread (the deepest active [`Span`], not necessarily the
/// request root). Returns `None` when no span is active.
///
/// Use this to propagate trace context into outbound calls made from within
/// a handler or a child span.
pub fn current_traceparent() -> Option<String> {
    ACTIVE_STACK.with(|stack| {
        stack.borrow().last().map(|s| {
            format!(
                "00-{}-{}-{:02x}",
                hex16(&s.trace_id),
                hex8(&s.span_id),
                s.sampled as u8,
            )
        })
    })
}

// ── Span ─────────────────────────────────────────────────────────────────────

/// A single open span. Create one with [`span`] or [`client_span`] (or
/// [`Span::new`] for full control over [`SpanKind`]); it becomes the
/// "current" span on this thread until it's dropped (or [`Span::end`] is
/// called explicitly, which is equivalent).
///
/// Nesting: creating a `Span` while another is already active makes the new
/// one its child — the child inherits the parent's `trace_id` and sampling
/// decision, and `parent_span_id()` is the parent's `span_id()`. Dropping the
/// child makes the parent "current" again.
///
/// Not `Send`: a `Span` must be dropped on the thread that created it, since
/// dropping pops a thread-local stack — moving one to another thread and
/// dropping it there would corrupt that thread's stack.
///
/// # Example
///
/// ```rust
/// use rust_web_server::otel;
///
/// let span = otel::span("db.query");
/// span.set_attribute("db.statement", "SELECT 1");
/// // ... do the work ...
/// span.end(); // or just let it drop at the end of scope
/// ```
pub struct Span {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    parent_span_id: Option<[u8; 8]>,
    sampled: bool,
    kind: SpanKind,
    name: String,
    start_ns: u64,
    attributes: RefCell<Vec<(String, AttributeValue)>>,
    status_code: Cell<u8>,
    http_method: RefCell<Option<String>>,
    http_target: RefCell<Option<String>>,
    http_status: Cell<Option<i16>>,
    // Makes `Span` `!Send` (a `Rc` is never `Send`) without requiring
    // unstable negative impls. See the "Not `Send`" note above.
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

fn new_span(name: &str, kind: SpanKind, incoming: Option<TraceContext>) -> Span {
    let (trace_id, parent_span_id, sampled) = ACTIVE_STACK.with(|stack| {
        let stack = stack.borrow();
        if let Some(top) = stack.last() {
            // Nested child: inherit the trace and the parent's sampling
            // decision — resampling independently per child would produce
            // broken/partial traces whenever a child's own coin-flip
            // disagreed with its parent's.
            (top.trace_id, Some(top.span_id), top.sampled)
        } else {
            // Root-ish: no active parent on this thread, so make a fresh
            // sampling decision, exactly like `OtelLayer` always has.
            let sampled = tracer().map(|t| t.should_sample()).unwrap_or(false);
            match incoming {
                Some(ctx) => (ctx.trace_id, Some(ctx.parent_span_id), sampled),
                None => (new_trace_id(), None, sampled),
            }
        }
    });
    let span_id = new_span_id();
    ACTIVE_STACK.with(|stack| stack.borrow_mut().push(ActiveSpanCtx { trace_id, span_id, sampled }));

    Span {
        trace_id,
        span_id,
        parent_span_id,
        sampled,
        kind,
        name: name.to_string(),
        start_ns: now_ns(),
        attributes: RefCell::new(Vec::new()),
        status_code: Cell::new(0),
        http_method: RefCell::new(None),
        http_target: RefCell::new(None),
        http_status: Cell::new(None),
        _not_send: std::marker::PhantomData,
    }
}

impl Span {
    /// Start a new span with an explicit [`SpanKind`]. Prefer [`span`] or
    /// [`client_span`] for the common cases.
    pub fn new(name: &str, kind: SpanKind) -> Span {
        new_span(name, kind, None)
    }

    /// Used only by [`OtelLayer`] to start the request's root span, honoring
    /// an incoming W3C `traceparent` header if present.
    pub(crate) fn start_root(name: &str, kind: SpanKind, incoming: Option<TraceContext>) -> Span {
        new_span(name, kind, incoming)
    }

    /// Attach a key/value attribute. Repeated keys are appended, not
    /// deduplicated — matching this module's existing "loosely OTLP" style.
    pub fn set_attribute(&self, key: &str, value: impl Into<AttributeValue>) {
        self.attributes.borrow_mut().push((key.to_string(), value.into()));
    }

    /// Mark this span as failed (OTLP `Status.code = 2`, Error).
    pub fn set_error(&self) {
        self.status_code.set(2);
    }

    /// Mark this span as failed and attach an `error.message` attribute.
    pub fn record_error(&self, message: &str) {
        self.set_error();
        self.set_attribute("error.message", message);
    }

    pub fn trace_id(&self) -> [u8; 16] {
        self.trace_id
    }

    pub fn span_id(&self) -> [u8; 8] {
        self.span_id
    }

    pub fn parent_span_id(&self) -> Option<[u8; 8]> {
        self.parent_span_id
    }

    /// End the span now. Equivalent to letting it drop at the end of scope —
    /// both run the exact same recording logic — but useful when you want
    /// the span's duration to stop before other work continues in the same
    /// scope.
    pub fn end(self) {}

    pub(crate) fn set_http(&self, method: &str, target: &str) {
        *self.http_method.borrow_mut() = Some(method.to_string());
        *self.http_target.borrow_mut() = Some(target.to_string());
    }

    /// Also marks the span as an error when `status >= 500`.
    pub(crate) fn set_http_status(&self, status: i16) {
        self.http_status.set(Some(status));
        if status >= 500 {
            self.set_error();
        }
    }

    fn finish(&mut self, end_ns: u64) -> SpanData {
        SpanData {
            trace_id: self.trace_id,
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            name: std::mem::take(&mut self.name),
            start_ns: self.start_ns,
            end_ns,
            http_method: self.http_method.get_mut().take().unwrap_or_default(),
            http_target: self.http_target.get_mut().take().unwrap_or_default(),
            http_status: self.http_status.get().unwrap_or(0),
            status_code: self.status_code.get(),
            kind: self.kind,
            attributes: std::mem::take(self.attributes.get_mut()),
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        ACTIVE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            // Pop this span and (defensively) anything still above it, in
            // case a descendant was somehow leaked without being dropped.
            if let Some(pos) = stack.iter().rposition(|c| c.span_id == self.span_id) {
                stack.truncate(pos);
            }
        });

        if !self.sampled {
            return;
        }
        let Some(t) = tracer() else { return };
        let end_ns = now_ns();
        t.record(self.finish(end_ns));
    }
}

/// Start a new [`SpanKind::Internal`] child span nested under the currently
/// active span (or a fresh trace if none is active). Use for internal work
/// like a database query or cache lookup.
///
/// ```rust
/// use rust_web_server::otel;
///
/// let span = otel::span("db.query");
/// span.set_attribute("db.statement", "SELECT 1");
/// ```
pub fn span(name: &str) -> Span {
    Span::new(name, SpanKind::Internal)
}

/// Start a new [`SpanKind::Client`] child span for an outbound call to
/// another service (an HTTP request, a gRPC call, ...).
pub fn client_span(name: &str) -> Span {
    Span::new(name, SpanKind::Client)
}

// ── exporter ──────────────────────────────────────────────────────────────────

/// Destination for completed spans.
pub trait Exporter: Send + Sync {
    fn export(&self, spans: &[SpanData]);
    fn shutdown(&self) {}
}

/// Renders one [`AttributeValue`] as an OTLP `AnyValue` JSON object.
/// `Int` is string-encoded per the OTLP JSON mapping for `int64`.
fn attr_value_json(v: &AttributeValue) -> String {
    match v {
        AttributeValue::String(s) => format!("{{\"stringValue\":\"{s}\"}}"),
        AttributeValue::Int(i) => format!("{{\"intValue\":\"{i}\"}}"),
        AttributeValue::Float(f) => format!("{{\"doubleValue\":{f}}}"),
        AttributeValue::Bool(b) => format!("{{\"boolValue\":{b}}}"),
    }
}

/// Renders one `(key, value)` pair as an OTLP `KeyValue` JSON object.
fn attr_json(key: &str, value: &AttributeValue) -> String {
    format!("{{\"key\":\"{key}\",\"value\":{}}}", attr_value_json(value))
}

/// Print one JSON line per span to stdout. Useful for development and for
/// piping into `jq` or a log aggregator.
pub struct StdoutExporter;

impl StdoutExporter {
    fn format_span(span: &SpanData) -> String {
        let http_attrs = if span.http_method.is_empty() {
            String::new()
        } else {
            format!(
                ",\"httpMethod\":\"{}\",\"httpTarget\":\"{}\",\"httpStatus\":{}",
                span.http_method, span.http_target, span.http_status,
            )
        };
        let extra_attrs: String = span.attributes.iter()
            .map(|(k, v)| format!(",\"{k}\":{}", attr_value_json(v)))
            .collect();
        format!(
            "{{\"traceId\":\"{}\",\"spanId\":\"{}\",\"parentSpanId\":{},\
             \"name\":\"{}\",\"kind\":{},\"startNs\":{},\"durationMs\":{:.3}{http_attrs}{extra_attrs}}}",
            hex16(&span.trace_id),
            hex8(&span.span_id),
            span.parent_span_id
                .as_ref()
                .map(|p| format!("\"{}\"", hex8(p)))
                .unwrap_or_else(|| "null".to_string()),
            span.name,
            span.kind as i32,
            span.start_ns,
            span.duration_ms(),
        )
    }
}

impl Exporter for StdoutExporter {
    fn export(&self, spans: &[SpanData]) {
        for span in spans {
            println!("{}", Self::format_span(span));
        }
    }
}

/// Send spans to an OTLP-compatible collector over HTTP (JSON encoding).
///
/// Compatible with Jaeger ≥ 1.35, Grafana Tempo, and OpenTelemetry Collector.
/// Point at `http://localhost:4318` (the default OTLP HTTP port).
pub struct OtlpHttpExporter {
    host: String,
    port: u16,
    timeout: Duration,
    service_name: String,
    service_version: String,
}

impl OtlpHttpExporter {
    pub fn new(endpoint: &str, service_name: &str, service_version: &str) -> Self {
        // Parse "http://host:port" or "host:port"
        let stripped = endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let (host, port) = if let Some(i) = stripped.rfind(':') {
            let p = stripped[i + 1..].parse().unwrap_or(4318);
            (stripped[..i].to_string(), p)
        } else {
            (stripped.to_string(), 4318)
        };
        OtlpHttpExporter {
            host,
            port,
            timeout: Duration::from_secs(5),
            service_name: service_name.to_string(),
            service_version: service_version.to_string(),
        }
    }

    pub fn build_body(&self, spans: &[SpanData]) -> String {
        let span_jsons: Vec<String> = spans.iter().map(|s| {
            let parent = s.parent_span_id
                .as_ref()
                .map(|p| format!(",\"parentSpanId\":\"{}\"", hex8(p)))
                .unwrap_or_default();
            let status_msg = if s.status_code == 2 { "Error" } else { "Unset" };

            let mut attrs: Vec<String> = Vec::new();
            if !s.http_method.is_empty() {
                attrs.push(format!("{{\"key\":\"http.method\",\"value\":{{\"stringValue\":\"{}\"}} }}", s.http_method));
                attrs.push(format!("{{\"key\":\"http.target\",\"value\":{{\"stringValue\":\"{}\"}} }}", s.http_target));
                attrs.push(format!("{{\"key\":\"http.status_code\",\"value\":{{\"intValue\":\"{}\"}} }}", s.http_status));
            }
            attrs.extend(s.attributes.iter().map(|(k, v)| attr_json(k, v)));

            format!(
                "{{\"traceId\":\"{trace}\",\"spanId\":\"{span}\"{parent},\
                 \"name\":\"{name}\",\"kind\":{kind},\
                 \"startTimeUnixNano\":\"{start}\",\"endTimeUnixNano\":\"{end}\",\
                 \"attributes\":[{attrs}],\
                 \"status\":{{\"code\":{scode},\"message\":\"{smsg}\"}} }}",
                trace = hex16(&s.trace_id),
                span  = hex8(&s.span_id),
                name  = s.name,
                kind  = s.kind as i32,
                start = s.start_ns,
                end   = s.end_ns,
                attrs = attrs.join(","),
                scode = s.status_code,
                smsg  = status_msg,
            )
        }).collect();

        format!(
            "{{\"resourceSpans\":[{{\"resource\":{{\"attributes\":[\
               {{\"key\":\"service.name\",\"value\":{{\"stringValue\":\"{svc}\"}} }},\
               {{\"key\":\"service.version\",\"value\":{{\"stringValue\":\"{ver}\"}} }}\
             ]}},\"scopeSpans\":[{{\"scope\":{{\"name\":\"rws\"}},\"spans\":[{spans}]}}]}}]}}",
            svc   = self.service_name,
            ver   = self.service_version,
            spans = span_jsons.join(","),
        )
    }

    fn post(&self, body: &str) {
        use std::net::ToSocketAddrs;
        let addr = format!("{}:{}", self.host, self.port);
        let Some(socket_addr) = addr.to_socket_addrs().ok().and_then(|mut i| i.next()) else {
            return;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, self.timeout) else {
            return;
        };
        let _ = stream.set_write_timeout(Some(self.timeout));
        let _ = stream.set_read_timeout(Some(self.timeout));
        let request = format!(
            "POST /v1/traces HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {len}\r\n\
             Connection: close\r\n\r\n\
             {body}",
            host = self.host,
            port = self.port,
            len  = body.len(),
            body = body,
        );
        if stream.write_all(request.as_bytes()).is_ok() {
            let mut _buf = [0u8; 256];
            let _ = stream.read(&mut _buf); // drain response
        }
    }
}

impl Exporter for OtlpHttpExporter {
    fn export(&self, spans: &[SpanData]) {
        if spans.is_empty() { return; }
        let body = self.build_body(spans);
        self.post(&body);
    }
}

// ── global tracer ─────────────────────────────────────────────────────────────

struct GlobalTracer {
    exporter: Box<dyn Exporter>,
    batch: Mutex<Vec<SpanData>>,
    batch_size: usize,
    sample_rate: f64,
    shutdown_flag: AtomicBool,
}

impl GlobalTracer {
    fn should_sample(&self) -> bool {
        if self.sample_rate >= 1.0 { return true; }
        if self.sample_rate <= 0.0 { return false; }
        // Use the counter as a pseudo-random source — cheap and uniform enough.
        let n = COUNTER.load(Ordering::Relaxed);
        (n % 10000) < (self.sample_rate * 10000.0) as u64
    }

    fn record(&self, span: SpanData) {
        let mut guard = self.batch.lock().unwrap();
        guard.push(span);
        if guard.len() >= self.batch_size {
            let spans = std::mem::take(&mut *guard);
            drop(guard);
            self.exporter.export(&spans);
        }
    }

    fn flush(&self) {
        let spans = std::mem::take(&mut *self.batch.lock().unwrap());
        if !spans.is_empty() {
            self.exporter.export(&spans);
        }
    }
}

static TRACER: OnceLock<GlobalTracer> = OnceLock::new();

fn tracer() -> Option<&'static GlobalTracer> {
    TRACER.get()
}

// ── public API ────────────────────────────────────────────────────────────────

/// Which backend to export spans to.
#[derive(Clone, Debug)]
pub enum ExporterConfig {
    /// Print JSON-encoded spans to stdout. Suitable for development.
    Stdout,
    /// POST OTLP JSON to `{endpoint}/v1/traces`.
    ///
    /// Compatible with Jaeger ≥ 1.35, Grafana Tempo, OpenTelemetry Collector.
    /// Typical endpoint: `"http://localhost:4318"`.
    Otlp { endpoint: String },
    /// No-op — spans are discarded. Useful in tests to silence output.
    Discard,
}

/// Configuration for the tracing subsystem.
#[derive(Clone, Debug)]
pub struct TracingConfig {
    /// Value of the `service.name` resource attribute (e.g. `"checkout-service"`).
    pub service_name: String,
    /// Value of the `service.version` resource attribute.
    pub service_version: String,
    /// Where to send completed spans.
    pub exporter: ExporterConfig,
    /// Fraction of requests to sample. `1.0` = 100%, `0.1` = 10%.
    pub sample_rate: f64,
    /// Maximum number of spans to accumulate before flushing to the exporter.
    pub batch_size: usize,
}

impl Default for TracingConfig {
    fn default() -> Self {
        TracingConfig {
            service_name: "rws".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            exporter: ExporterConfig::Stdout,
            sample_rate: 1.0,
            batch_size: 128,
        }
    }
}

/// Initialize tracing with an explicit config. Call once at startup before
/// the server starts accepting requests.
///
/// Calling this more than once is a no-op (the first call wins).
pub fn setup(config: TracingConfig) {
    let exporter: Box<dyn Exporter> = match &config.exporter {
        ExporterConfig::Stdout => Box::new(StdoutExporter),
        ExporterConfig::Otlp { endpoint } => Box::new(OtlpHttpExporter::new(
            endpoint,
            &config.service_name,
            &config.service_version,
        )),
        ExporterConfig::Discard => Box::new(DiscardExporter),
    };
    setup_with_exporter(config, exporter);
}

/// Like [`setup`], but takes the exporter directly instead of building one
/// from [`TracingConfig::exporter`]. Lets you wire in any [`Exporter`] —
/// including [`CapturingExporter`] — through the same code path production
/// code uses, rather than only [`ExporterConfig`]'s three built-in choices.
///
/// Calling this (or [`setup`]) more than once is a no-op (the first call wins).
pub fn setup_with_exporter(config: TracingConfig, exporter: Box<dyn Exporter>) {
    let _ = TRACER.set(GlobalTracer {
        exporter,
        batch: Mutex::new(Vec::new()),
        batch_size: config.batch_size.max(1),
        sample_rate: config.sample_rate.clamp(0.0, 1.0),
        shutdown_flag: AtomicBool::new(false),
    });
}

/// Initialize tracing from standard OpenTelemetry environment variables:
///
/// - `OTEL_SERVICE_NAME` — service name (default `"rws"`)
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` — OTLP endpoint URL (default: stdout)
/// - `OTEL_TRACES_SAMPLER_ARG` — sample rate `0.0`–`1.0` (default `1.0`)
pub fn setup_from_env() {
    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| "rws".to_string());
    let sample_rate: f64 = std::env::var("OTEL_TRACES_SAMPLER_ARG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let exporter = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok() {
        Some(ep) if !ep.is_empty() => ExporterConfig::Otlp { endpoint: ep },
        _ => ExporterConfig::Stdout,
    };
    setup(TracingConfig {
        service_name,
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        exporter,
        sample_rate,
        batch_size: 128,
    });
}

/// Flush all buffered spans to the exporter. Call before the process exits
/// to ensure no spans are lost.
pub fn shutdown() {
    if let Some(t) = tracer() {
        t.shutdown_flag.store(true, Ordering::Relaxed);
        t.flush();
        t.exporter.shutdown();
    }
}

/// Flush buffered spans without shutting down. Useful in tests.
pub fn flush() {
    if let Some(t) = tracer() {
        t.flush();
    }
}

// ── OtelLayer middleware ──────────────────────────────────────────────────────

/// Middleware that creates an HTTP server span for each request.
///
/// - Reads W3C `traceparent` from the request to continue an existing trace.
/// - Creates a new trace when no `traceparent` is present.
/// - Stores the active span in thread-local storage so downstream middleware
///   (e.g. [`crate::proxy::ReverseProxy`]) can propagate it.
/// - Records the span on the way out with `http.method`, `http.target`, and
///   `http.status_code` attributes.
///
/// Requires [`setup`] or [`setup_from_env`] to be called at startup. Without
/// initialisation the layer is a no-op passthrough.
pub struct OtelLayer;

impl Middleware for OtelLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if tracer().is_none() {
            return next.execute(request, connection);
        }

        // Extract an existing trace context from the request, if present.
        let incoming = request.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case("traceparent"))
            .and_then(|h| TraceContext::parse(&h.value));

        let path = strip_query(&request.request_uri).to_string();
        let root = Span::start_root(&format!("{} {}", request.method, path), SpanKind::Server, incoming);
        root.set_http(&request.method, &request.request_uri);

        let result = next.execute(request, connection);

        let status = match &result {
            Ok(r) => r.status_code,
            Err(_) => 500,
        };
        root.set_http_status(status);

        result
        // `root` drops here: pops the thread-local stack and records the
        // span (if sampled) — the exact same path a child `Span` follows.
    }
}

// ── internal no-op exporter (used for tests / Discard config) ─────────────────

struct DiscardExporter;
impl Exporter for DiscardExporter {
    fn export(&self, _: &[SpanData]) {}
}

// ── collect spans in tests ────────────────────────────────────────────────────

/// Captures spans in memory instead of exporting them. Use in unit tests via
/// [`setup_with_exporter`] to prove that a span was actually recorded, not
/// just that its getters return the right values.
///
/// ```rust,no_run
/// use rust_web_server::otel::{self, CapturingExporter, TracingConfig, ExporterConfig};
/// use std::sync::Arc;
///
/// let captured = Arc::new(CapturingExporter::new());
/// otel::setup_with_exporter(
///     TracingConfig { exporter: ExporterConfig::Discard, ..Default::default() },
///     Box::new(captured.clone()),
/// );
///
/// otel::span("db.query").end();
/// otel::flush();
/// assert_eq!(1, captured.take().len());
/// ```
pub struct CapturingExporter {
    pub spans: Mutex<Vec<SpanData>>,
}

impl CapturingExporter {
    pub fn new() -> Self {
        CapturingExporter { spans: Mutex::new(Vec::new()) }
    }

    pub fn take(&self) -> Vec<SpanData> {
        std::mem::take(&mut *self.spans.lock().unwrap())
    }
}

impl Default for CapturingExporter {
    fn default() -> Self { Self::new() }
}

impl Exporter for CapturingExporter {
    fn export(&self, spans: &[SpanData]) {
        self.spans.lock().unwrap().extend_from_slice(spans);
    }
}

/// Lets a shared, externally-held `Arc<CapturingExporter>` be handed to
/// [`setup_with_exporter`] (which takes ownership of a `Box<dyn Exporter>`)
/// while the caller keeps its own handle to call [`CapturingExporter::take`]
/// afterward.
impl Exporter for std::sync::Arc<CapturingExporter> {
    fn export(&self, spans: &[SpanData]) {
        self.spans.lock().unwrap().extend_from_slice(spans);
    }
}
