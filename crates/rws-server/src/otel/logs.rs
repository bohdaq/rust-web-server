//! OTLP logs export (18.33.0, #102): every access-log record pushed to an
//! OpenTelemetry collector over OTLP/HTTP (JSON, `/v1/logs`), with the
//! request's trace and span ids when it had a server span.
//!
//! Off until [`start`] (the `rws` binary calls [`start_from_env`]: on with
//! `OTEL_LOGS_EXPORTER=otlp`). Then [`crate::log::Log::log_access`] formats
//! each record into a buffer of its thread's own — no shared lock, no I/O on
//! the request path — and the thread `rws-otlp-logs` collects every buffer
//! and POSTs one batch every `OTEL_BLRP_SCHEDULE_DELAY` (1 s by default).
//! A thread holding more than [`MAX_PENDING`] bytes drops records (counted
//! by [`dropped`]) rather than grow without bound while a collector is down.
//! [`super::shutdown`] sends what is left.
//!
//! A record's body is the access-log line (`RWS_CONFIG_LOG_FORMAT`; JSON
//! when that is `off`), with the access log's privacy settings applied; its
//! attributes are `http.request.method`, `url.path`,
//! `http.response.status_code`, `client.address` and
//! `network.protocol.version`. Severity is INFO, WARN for 4xx and ERROR for
//! 5xx.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::request::Request;
use crate::response::Response;
use crate::server_config::LogPrivacy;

/// A thread's unsent records past this many bytes are dropped.
pub const MAX_PENDING: usize = 1 << 20;
/// A POST carries at most about this many bytes of records.
pub const MAX_BATCH: usize = 4 << 20;

/// The response header `OtelLayer` hands the request's trace context to the
/// access log in, while export is on; the server's connection loops remove
/// it before the response is written ([`take_trace`]).
#[doc(hidden)]
pub const TRACE_HEADER: &str = "x-rws-log-trace";

/// The trace a logged request belonged to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceIds {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub sampled: bool,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Whether access-log records are exported (after [`start`]).
pub fn enabled() -> bool {
    #[cfg(test)]
    if TEST_ENABLED.with(|e| e.get()) {
        return true;
    }
    ENABLED.load(Ordering::Relaxed)
}

/// Records dropped because a thread's buffer was full.
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

fn hex(bytes: &[u8], out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Put `ids` on `response` for the access log (only while export is on).
#[doc(hidden)]
pub fn put_trace(response: &mut Response, ids: TraceIds) {
    if !enabled() {
        return;
    }
    let mut value = String::with_capacity(52);
    hex(&ids.trace_id, &mut value);
    value.push('-');
    hex(&ids.span_id, &mut value);
    value.push_str(if ids.sampled { "-01" } else { "-00" });
    response.headers.push(crate::header::Header::new(TRACE_HEADER, value));
}

/// Remove the trace context [`put_trace`] left on `response` and return
/// it. One atomic load while export is off.
pub fn take_trace(response: &mut Response) -> Option<TraceIds> {
    if !enabled() {
        return None;
    }
    let at = response.headers.iter().position(|h| h.name.eq_ignore_ascii_case(TRACE_HEADER))?;
    let header = response.headers.remove(at);
    parse_trace(&header.value)
}

fn parse_trace(value: &str) -> Option<TraceIds> {
    let mut parts = value.split('-');
    let trace_id = unhex::<16>(parts.next()?)?;
    let span_id = unhex::<8>(parts.next()?)?;
    let sampled = parts.next()? == "01";
    Some(TraceIds { trace_id, span_id, sampled })
}

// ── per-thread buffers ───────────────────────────────────────────────────────

type Shard = Arc<Mutex<Vec<u8>>>;

fn registry() -> &'static Mutex<Vec<Shard>> {
    static REGISTRY: OnceLock<Mutex<Vec<Shard>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Records of threads that have exited, not sent yet.
fn orphans() -> &'static Mutex<Vec<u8>> {
    static ORPHANS: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    &ORPHANS
}

struct ShardHandle(Shard);

impl Drop for ShardHandle {
    fn drop(&mut self) {
        let pending = std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()));
        append(&mut orphans().lock().unwrap_or_else(|e| e.into_inner()), &pending);
        registry().lock().unwrap_or_else(|e| e.into_inner()).retain(|s| !Arc::ptr_eq(s, &self.0));
    }
}

thread_local! {
    static SHARD: ShardHandle = {
        let shard: Shard = Arc::new(Mutex::new(Vec::with_capacity(4096)));
        registry().lock().unwrap_or_else(|e| e.into_inner()).push(Arc::clone(&shard));
        ShardHandle(shard)
    };
    static SCRATCH: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Append comma-separated `records` to `buf`, unless that takes it past
/// [`MAX_PENDING`].
fn append(buf: &mut Vec<u8>, records: &[u8]) {
    if records.is_empty() {
        return;
    }
    if buf.len() + records.len() > MAX_PENDING {
        DROPPED.fetch_add(1 + memchr::memmem::find_iter(records, b"},{\"timeUnixNano\"").count() as u64, Ordering::Relaxed);
        return;
    }
    if !buf.is_empty() {
        buf.push(b',');
    }
    buf.extend_from_slice(records);
}

/// Record one request: `write_line` writes the access-log line that
/// becomes the body. Does nothing until [`start`].
pub fn record(
    request: &Request,
    response: &Response,
    client: std::net::IpAddr,
    privacy: &LogPrivacy,
    trace: Option<TraceIds>,
    write_line: impl FnOnce(&mut Vec<u8>),
) {
    if !enabled() {
        return;
    }
    let mut write_line = Some(write_line);
    let mut build = |out: &mut Vec<u8>, line: &mut Vec<u8>| {
        line.clear();
        if let Some(write_line) = write_line.take() {
            write_line(line);
        }
        write_record(out, request, response, client, privacy, trace, line, super::now_ns());
    };
    let done = SHARD.try_with(|handle| {
        SCRATCH.with(|scratch| {
            let mut record = Vec::with_capacity(512);
            build(&mut record, &mut scratch.borrow_mut());
            let mut buf = handle.0.lock().unwrap_or_else(|e| e.into_inner());
            append(&mut buf, &record);
        })
    });
    if done.is_err() {
        // The thread is exiting: straight to the orphans.
        let (mut record, mut line) = (Vec::new(), Vec::new());
        build(&mut record, &mut line);
        append(&mut orphans().lock().unwrap_or_else(|e| e.into_inner()), &record);
    }
}

/// One OTLP JSON `LogRecord`.
#[allow(clippy::too_many_arguments)]
fn write_record(
    out: &mut Vec<u8>,
    request: &Request,
    response: &Response,
    client: std::net::IpAddr,
    privacy: &LogPrivacy,
    trace: Option<TraceIds>,
    line: &[u8],
    now_ns: u64,
) {
    use std::io::Write;
    let (number, text) = match response.status_code {
        500.. => (17, "ERROR"),
        400.. => (13, "WARN"),
        _ => (9, "INFO"),
    };
    let _ = write!(
        out,
        "{{\"timeUnixNano\":\"{now_ns}\",\"observedTimeUnixNano\":\"{now_ns}\",\"severityNumber\":{number},\"severityText\":\"{text}\",\"body\":{{\"stringValue\":\""
    );
    escape_into(out, line);
    out.extend_from_slice(b"\"},\"attributes\":[");
    string_attr(out, "http.request.method", &request.method);
    out.push(b',');
    let path = request.request_uri.split(['?', '#']).next().unwrap_or("");
    string_attr(out, "url.path", path);
    let _ = write!(out, ",{{\"key\":\"http.response.status_code\",\"value\":{{\"intValue\":\"{}\"}}}},", response.status_code);
    string_attr(out, "client.address", &privacy.ip_string(client));
    out.push(b',');
    let version = request.http_version.strip_prefix("HTTP/").unwrap_or(&request.http_version);
    string_attr(out, "network.protocol.version", version);
    out.push(b']');
    if let Some(ids) = trace {
        let mut trace_hex = String::with_capacity(32);
        hex(&ids.trace_id, &mut trace_hex);
        let mut span_hex = String::with_capacity(16);
        hex(&ids.span_id, &mut span_hex);
        let _ = write!(out, ",\"traceId\":\"{trace_hex}\",\"spanId\":\"{span_hex}\",\"flags\":{}", ids.sampled as u8);
    }
    out.push(b'}');
}

fn string_attr(out: &mut Vec<u8>, key: &str, value: &str) {
    out.extend_from_slice(b"{\"key\":\"");
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(b"\",\"value\":{\"stringValue\":\"");
    escape_into(out, value.as_bytes());
    out.extend_from_slice(b"\"}}");
}

/// `bytes` as the inside of a JSON string: every control character
/// escaped, invalid UTF-8 replaced.
fn escape_into(out: &mut Vec<u8>, bytes: &[u8]) {
    use std::io::Write;
    for c in String::from_utf8_lossy(bytes).chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\r' => out.extend_from_slice(b"\\r"),
            '\t' => out.extend_from_slice(b"\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => {
                let mut utf8 = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
            }
        }
    }
}

/// Every buffered record, taken: batches of at most about [`MAX_BATCH`]
/// bytes, each a comma-separated list of `LogRecord`s.
#[doc(hidden)]
pub fn drain() -> Vec<Vec<u8>> {
    let shards: Vec<Shard> = registry().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut pieces = vec![std::mem::take(&mut *orphans().lock().unwrap_or_else(|e| e.into_inner()))];
    for shard in shards {
        pieces.push(std::mem::take(&mut *shard.lock().unwrap_or_else(|e| e.into_inner())));
    }
    let mut batches: Vec<Vec<u8>> = Vec::new();
    for piece in pieces.into_iter().filter(|p| !p.is_empty()) {
        match batches.last_mut() {
            Some(batch) if batch.len() + piece.len() < MAX_BATCH => {
                batch.push(b',');
                batch.extend_from_slice(&piece);
            }
            _ => batches.push(piece),
        }
    }
    batches
}

/// The OTLP/HTTP JSON `ExportLogsServiceRequest` for `records` (a
/// comma-separated list of `LogRecord`s).
pub fn body(records: &[u8], service_name: &str, service_version: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() + 256);
    out.extend_from_slice(b"{\"resourceLogs\":[{\"resource\":{\"attributes\":[");
    string_attr(&mut out, "service.name", service_name);
    out.push(b',');
    string_attr(&mut out, "service.version", service_version);
    out.extend_from_slice(b"]},\"scopeLogs\":[{\"scope\":{\"name\":\"rust-web-server\",\"version\":\"");
    out.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    out.extend_from_slice(b"\"},\"logRecords\":[");
    out.extend_from_slice(records);
    out.extend_from_slice(b"]}]}]}");
    out
}

// ── the exporter ─────────────────────────────────────────────────────────────

/// Where and how often to push logs.
#[derive(Clone, Debug)]
pub struct LogsExportConfig {
    /// The full URL logs are POSTed to, e.g. `http://collector:4318/v1/logs`.
    pub endpoint: String,
    /// Time between exports.
    pub interval: Duration,
    /// `service.name` resource attribute.
    pub service_name: String,
    /// `service.version` resource attribute.
    pub service_version: String,
    /// Extra request headers (`OTEL_EXPORTER_OTLP_HEADERS`), e.g. an API key.
    pub headers: Vec<(String, String)>,
}

struct Exporter {
    config: LogsExportConfig,
    stop: AtomicBool,
    wake: (Mutex<()>, Condvar),
    /// Held while a batch is sent, so [`shutdown`] waits for one in flight.
    sending: Mutex<()>,
}

static EXPORTER: OnceLock<Exporter> = OnceLock::new();

/// Start exporting on a background thread. A second call is a no-op.
pub fn start(config: LogsExportConfig) {
    let interval = config.interval.max(Duration::from_millis(100));
    let exporter = Exporter { config, stop: AtomicBool::new(false), wake: (Mutex::new(()), Condvar::new()), sending: Mutex::new(()) };
    if EXPORTER.set(exporter).is_err() {
        return;
    }
    ENABLED.store(true, Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("rws-otlp-logs".to_string()).spawn(move || {
        let exporter = EXPORTER.get().expect("set above");
        loop {
            let guard = exporter.wake.0.lock().unwrap_or_else(|e| e.into_inner());
            let _ = exporter.wake.1.wait_timeout(guard, interval);
            if exporter.stop.load(Ordering::Relaxed) {
                return;
            }
            export(exporter);
        }
    });
}

/// [`start`] configured by the standard OpenTelemetry variables, when
/// `OTEL_LOGS_EXPORTER` names `otlp`:
///
/// - `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` — full URL, used as is; else
///   `OTEL_EXPORTER_OTLP_ENDPOINT` + `/v1/logs`; else
///   `http://localhost:4318/v1/logs`
/// - `OTEL_BLRP_SCHEDULE_DELAY` — milliseconds between exports (default 1000)
/// - `OTEL_SERVICE_NAME` (default `rws`), `OTEL_EXPORTER_OTLP_HEADERS`
///   (`key=value,…`; `OTEL_EXPORTER_OTLP_LOGS_HEADERS` wins)
///
/// Returns whether it started.
pub fn start_from_env() -> bool {
    let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let Some(config) = config_from(var) else {
        return false;
    };
    start(config);
    true
}

/// The configuration [`start_from_env`] would use, from `var`.
#[doc(hidden)]
pub fn config_from(var: impl Fn(&str) -> Option<String>) -> Option<LogsExportConfig> {
    if !var("OTEL_LOGS_EXPORTER").is_some_and(|v| v.split(',').any(|e| e.trim().eq_ignore_ascii_case("otlp"))) {
        return None;
    }
    let endpoint = var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .or_else(|| var("OTEL_EXPORTER_OTLP_ENDPOINT").map(|base| format!("{}/v1/logs", base.trim_end_matches('/'))))
        .unwrap_or_else(|| "http://localhost:4318/v1/logs".to_string());
    let interval = var("OTEL_BLRP_SCHEDULE_DELAY").and_then(|v| v.parse().ok()).unwrap_or(1000);
    let headers = var("OTEL_EXPORTER_OTLP_LOGS_HEADERS").or_else(|| var("OTEL_EXPORTER_OTLP_HEADERS")).map(|h| super::parse_headers(&h)).unwrap_or_default();
    Some(LogsExportConfig {
        endpoint,
        interval: Duration::from_millis(interval),
        service_name: var("OTEL_SERVICE_NAME").unwrap_or_else(|| "rws".to_string()),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        headers,
    })
}

/// Stop the background thread after sending what is buffered (called by
/// [`super::shutdown`]).
#[doc(hidden)]
pub fn shutdown() {
    if let Some(exporter) = EXPORTER.get() {
        if !exporter.stop.swap(true, Ordering::Relaxed) {
            exporter.wake.1.notify_all();
            export(exporter);
        }
    }
}

fn export(exporter: &Exporter) {
    let _sending = exporter.sending.lock().unwrap_or_else(|e| e.into_inner());
    for records in drain() {
        let body = body(&records, &exporter.config.service_name, &exporter.config.service_version);
        let client = crate::http_client::Client::new().timeout_ms(10_000);
        let mut request = client.post(&exporter.config.endpoint).header("Content-Type", "application/json").body(body);
        for (name, value) in &exporter.config.headers {
            request = request.header(name, value);
        }
        match request.send() {
            Ok(response) if response.is_success() => {}
            Ok(response) => eprintln!("OTLP logs export to {}: HTTP {}", exporter.config.endpoint, response.status()),
            Err(e) => eprintln!("OTLP logs export to {}: {}", exporter.config.endpoint, e),
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Recording on for this test thread only (no exporter): the process-wide
    /// switch would change other tests' responses (`OtelLayer`'s header).
    static TEST_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn enable_on_this_thread(on: bool) {
    TEST_ENABLED.with(|e| e.set(on));
}

#[cfg(test)]
mod tests;
