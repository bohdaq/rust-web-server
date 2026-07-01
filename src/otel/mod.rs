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

use std::cell::Cell;
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

// ── thread-local active span ──────────────────────────────────────────────────

/// Compact span context stored in thread-local for downstream propagation.
#[derive(Copy, Clone)]
struct ActiveSpan {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    sampled: bool,
}

thread_local! {
    static ACTIVE: Cell<Option<ActiveSpan>> = Cell::new(None);
}

/// Return the W3C `traceparent` value for the span currently being processed
/// on this thread. Returns `None` when no `OtelLayer` span is active.
///
/// Used by [`crate::proxy::ReverseProxy`] to propagate trace context to
/// upstream services.
pub fn current_traceparent() -> Option<String> {
    ACTIVE.with(|cell| {
        cell.get().map(|s| {
            format!(
                "00-{}-{}-{:02x}",
                hex16(&s.trace_id),
                hex8(&s.span_id),
                s.sampled as u8,
            )
        })
    })
}

// ── span data ─────────────────────────────────────────────────────────────────

/// A completed span ready for export.
#[derive(Debug, Clone)]
pub struct SpanData {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    /// `"GET /api/users"` — method + path, query stripped.
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
    pub http_method: String,
    pub http_target: String,
    pub http_status: i16,
    /// 0=Unset, 1=Ok, 2=Error
    pub status_code: u8,
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

// ── exporter ──────────────────────────────────────────────────────────────────

/// Destination for completed spans.
pub trait Exporter: Send + Sync {
    fn export(&self, spans: &[SpanData]);
    fn shutdown(&self) {}
}

/// Print one JSON line per span to stdout. Useful for development and for
/// piping into `jq` or a log aggregator.
pub struct StdoutExporter;

impl Exporter for StdoutExporter {
    fn export(&self, spans: &[SpanData]) {
        for span in spans {
            println!(
                "{{\"traceId\":\"{}\",\"spanId\":\"{}\",\"parentSpanId\":{},\
                 \"name\":\"{}\",\"startNs\":{},\"durationMs\":{:.3},\
                 \"httpMethod\":\"{}\",\"httpTarget\":\"{}\",\"httpStatus\":{}}}",
                hex16(&span.trace_id),
                hex8(&span.span_id),
                span.parent_span_id
                    .as_ref()
                    .map(|p| format!("\"{}\"", hex8(p)))
                    .unwrap_or_else(|| "null".to_string()),
                span.name,
                span.start_ns,
                span.duration_ms(),
                span.http_method,
                span.http_target,
                span.http_status,
            );
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
            format!(
                "{{\"traceId\":\"{trace}\",\"spanId\":\"{span}\"{parent},\
                 \"name\":\"{name}\",\"kind\":2,\
                 \"startTimeUnixNano\":\"{start}\",\"endTimeUnixNano\":\"{end}\",\
                 \"attributes\":[\
                   {{\"key\":\"http.method\",\"value\":{{\"stringValue\":\"{method}\"}} }},\
                   {{\"key\":\"http.target\",\"value\":{{\"stringValue\":\"{target}\"}} }},\
                   {{\"key\":\"http.status_code\",\"value\":{{\"intValue\":{status}}} }}\
                 ],\
                 \"status\":{{\"code\":{scode},\"message\":\"{smsg}\"}} }}",
                trace  = hex16(&s.trace_id),
                span   = hex8(&s.span_id),
                name   = s.name,
                start  = s.start_ns,
                end    = s.end_ns,
                method = s.http_method,
                target = s.http_target,
                status = s.http_status,
                scode  = s.status_code,
                smsg   = status_msg,
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
        let Some(t) = tracer() else {
            return next.execute(request, connection);
        };

        let sampled = t.should_sample();

        // Extract or create trace context.
        let incoming = request.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case("traceparent"))
            .and_then(|h| TraceContext::parse(&h.value));

        let trace_id = incoming.map(|c| c.trace_id).unwrap_or_else(new_trace_id);
        let parent_span_id = incoming.map(|c| c.parent_span_id);
        let span_id = new_span_id();

        // Publish active span for downstream propagation.
        ACTIVE.with(|cell| {
            cell.set(Some(ActiveSpan { trace_id, span_id, sampled }));
        });

        let start_ns = now_ns();
        let result = next.execute(request, connection);
        let end_ns = now_ns();

        // Clear active span.
        ACTIVE.with(|cell| cell.set(None));

        if sampled {
            let status = match &result {
                Ok(r) => r.status_code,
                Err(_) => 500,
            };
            let path = strip_query(&request.request_uri).to_string();
            t.record(SpanData {
                trace_id,
                span_id,
                parent_span_id,
                name: format!("{} {}", request.method, path),
                start_ns,
                end_ns,
                http_method: request.method.clone(),
                http_target: request.request_uri.clone(),
                http_status: status,
                status_code: if status >= 500 { 2 } else { 0 },
            });
        }

        result
    }
}

// ── internal no-op exporter (used for tests / Discard config) ─────────────────

struct DiscardExporter;
impl Exporter for DiscardExporter {
    fn export(&self, _: &[SpanData]) {}
}

// ── collect spans in tests ────────────────────────────────────────────────────

/// Captures spans in memory instead of exporting them. Use in unit tests.
///
/// ```rust
/// use rust_web_server::otel::{CapturingExporter, ExporterConfig, TracingConfig, setup};
/// use std::sync::{Arc, Mutex};
///
/// let captured: Arc<Mutex<Vec<_>>> = Default::default();
/// // (CapturingExporter is constructed internally by the test helpers)
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
