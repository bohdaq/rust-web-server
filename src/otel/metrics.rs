//! OTLP metrics export: `http.server.request.duration` pushed to an
//! OpenTelemetry collector over OTLP/HTTP (JSON), every `interval`, with
//! cumulative temporality and the semantic convention's explicit buckets.
//!
//! The data is the same store `/metrics` serves as
//! `http_server_request_duration_seconds` ([`crate::metrics::http`]), so the
//! two never disagree. Start it with [`start`] or [`start_from_env`] (the
//! `rws` binary does when `OTEL_EXPORTER_OTLP_ENDPOINT` or
//! `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` is set); [`super::shutdown`] sends a
//! last export.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::metrics::http;

/// Where and how often to push metrics.
#[derive(Clone, Debug)]
pub struct MetricsExportConfig {
    /// The full URL metrics are POSTed to, e.g. `http://collector:4318/v1/metrics`.
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
    config: MetricsExportConfig,
    start_ns: u64,
    stop: AtomicBool,
    wake: (Mutex<()>, Condvar),
}

static EXPORTER: OnceLock<Exporter> = OnceLock::new();

fn now_ns() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

/// Start pushing metrics on a background thread. A second call is a no-op.
pub fn start(config: MetricsExportConfig) {
    let interval = config.interval.max(Duration::from_millis(100));
    let exporter = Exporter { config, start_ns: now_ns(), stop: AtomicBool::new(false), wake: (Mutex::new(()), Condvar::new()) };
    if EXPORTER.set(exporter).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("rws-otlp-metrics".to_string()).spawn(move || {
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

/// [`start`] configured by the standard OpenTelemetry variables, if an
/// endpoint is set and `OTEL_METRICS_EXPORTER` isn't `none`:
///
/// - `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` — full URL, used as is; else
///   `OTEL_EXPORTER_OTLP_ENDPOINT` + `/v1/metrics`
/// - `OTEL_METRIC_EXPORT_INTERVAL` — milliseconds (default 60000)
/// - `OTEL_SERVICE_NAME` (default `rws`), `OTEL_EXPORTER_OTLP_HEADERS`
///   (`key=value,…`; `OTEL_EXPORTER_OTLP_METRICS_HEADERS` wins)
///
/// Returns whether it started.
pub fn start_from_env() -> bool {
    let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    if var("OTEL_METRICS_EXPORTER").is_some_and(|v| v.eq_ignore_ascii_case("none")) {
        return false;
    }
    let Some(endpoint) = var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT")
        .or_else(|| var("OTEL_EXPORTER_OTLP_ENDPOINT").map(|base| format!("{}/v1/metrics", base.trim_end_matches('/'))))
    else {
        return false;
    };
    let interval = var("OTEL_METRIC_EXPORT_INTERVAL").and_then(|v| v.parse().ok()).unwrap_or(60_000);
    let headers = var("OTEL_EXPORTER_OTLP_METRICS_HEADERS").or_else(|| var("OTEL_EXPORTER_OTLP_HEADERS")).map(|h| super::parse_headers(&h)).unwrap_or_default();
    start(MetricsExportConfig {
        endpoint,
        interval: Duration::from_millis(interval),
        service_name: var("OTEL_SERVICE_NAME").unwrap_or_else(|| "rws".to_string()),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        headers,
    });
    true
}

/// Stop the background thread after one last export (called by [`super::shutdown`]).
pub(crate) fn shutdown() {
    if let Some(exporter) = EXPORTER.get() {
        if !exporter.stop.swap(true, Ordering::Relaxed) {
            exporter.wake.1.notify_all();
            export(exporter);
        }
    }
}

fn export(exporter: &Exporter) {
    let body = body(&exporter.config.service_name, &exporter.config.service_version, exporter.start_ns, now_ns());
    let client = crate::http_client::Client::new().timeout_ms(10_000);
    let mut request = client.post(&exporter.config.endpoint).body_json(&body);
    for (name, value) in &exporter.config.headers {
        request = request.header(name, value);
    }
    match request.send() {
        Ok(response) if response.is_success() => {}
        Ok(response) => eprintln!("OTLP metrics export to {}: HTTP {}", exporter.config.endpoint, response.status()),
        Err(e) => eprintln!("OTLP metrics export to {}: {}", exporter.config.endpoint, e),
    }
}

/// The OTLP/HTTP JSON `ExportMetricsServiceRequest` for the current data.
pub fn body(service_name: &str, service_version: &str, start_ns: u64, now_ns: u64) -> String {
    let points: Vec<String> = http::snapshot()
        .into_iter()
        .map(|(series, histogram)| {
            let attributes = [
                string_attr("http.request.method", series.method()),
                int_attr("http.response.status_code", series.status as i64),
                string_attr("network.protocol.name", "http"),
                string_attr("network.protocol.version", series.protocol_version()),
                string_attr("url.scheme", series.scheme()),
            ]
            .join(",");
            let counts: Vec<String> = histogram.counts.iter().map(|c| format!("\"{c}\"")).collect();
            let bounds: Vec<String> = http::BOUNDS.iter().map(|b| b.to_string()).collect();
            format!(
                "{{\"attributes\":[{attributes}],\"startTimeUnixNano\":\"{start_ns}\",\"timeUnixNano\":\"{now_ns}\",\
                 \"count\":\"{}\",\"sum\":{},\"bucketCounts\":[{}],\"explicitBounds\":[{}]}}",
                histogram.count,
                histogram.sum,
                counts.join(","),
                bounds.join(","),
            )
        })
        .collect();
    let metrics = if points.is_empty() {
        String::new()
    } else {
        format!(
            "{{\"name\":\"http.server.request.duration\",\"description\":\"Duration of HTTP server requests.\",\"unit\":\"s\",\
             \"histogram\":{{\"aggregationTemporality\":2,\"dataPoints\":[{}]}}}}",
            points.join(",")
        )
    };
    format!(
        "{{\"resourceMetrics\":[{{\"resource\":{{\"attributes\":[{},{}]}},\
         \"scopeMetrics\":[{{\"scope\":{{\"name\":\"rust-web-server\",\"version\":\"{}\"}},\"metrics\":[{metrics}]}}]}}]}}",
        string_attr("service.name", service_name),
        string_attr("service.version", service_version),
        env!("CARGO_PKG_VERSION"),
    )
}

fn string_attr(key: &str, value: &str) -> String {
    format!("{{\"key\":\"{key}\",\"value\":{{\"stringValue\":\"{}\"}}}}", super::json_escape(value))
}

fn int_attr(key: &str, value: i64) -> String {
    format!("{{\"key\":\"{key}\",\"value\":{{\"intValue\":\"{value}\"}}}}")
}

#[cfg(test)]
mod tests;
