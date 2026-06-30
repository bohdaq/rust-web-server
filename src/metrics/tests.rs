use std::sync::atomic::Ordering;

use crate::application::Application;
use crate::http::VERSION;
use crate::metrics::{
    ACTIVE_CONNECTIONS, ERRORS_TOTAL, REQUESTS_TOTAL,
    MetricsLayer, connection_close, connection_open,
    prometheus_text, record_error, record_request, record_route,
};
use crate::middleware::Middleware;
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

// ── helpers ───────────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 9999 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 8192,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn ok_response() -> Response {
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: 200,
        reason_phrase: "OK".to_string(),
        headers: vec![],
        content_range_list: vec![],
        stream_file: None,
    }
}

struct FixedApp(Response);

impl Application for FixedApp {
    fn execute(&self, _req: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        Ok(self.0.clone())
    }
}

struct ErrorApp;

impl Application for ErrorApp {
    fn execute(&self, _req: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        Err("boom".to_string())
    }
}

// ── server-wide counter behaviour ─────────────────────────────────────────────
//
// Static atomics persist across tests.  We assert relative increments (after >=
// before + N) so that concurrent tests don't create false failures.

#[test]
fn record_request_increments_requests_total() {
    let before = REQUESTS_TOTAL.load(Ordering::SeqCst);
    record_request();
    let after = REQUESTS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 1);
}

#[test]
fn record_request_called_multiple_times_accumulates() {
    let before = REQUESTS_TOTAL.load(Ordering::SeqCst);
    record_request();
    record_request();
    record_request();
    let after = REQUESTS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 3);
}

#[test]
fn record_error_increments_errors_total() {
    let before = ERRORS_TOTAL.load(Ordering::SeqCst);
    record_error();
    let after = ERRORS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 1);
}

#[test]
fn connection_open_increments_active_connections() {
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_open();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    assert!(after >= before + 1);
    connection_close();
}

#[test]
fn connection_close_decrements_active_connections() {
    connection_open();
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_close();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    assert!(after <= before - 1);
}

#[test]
fn open_and_close_net_zero() {
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_open();
    connection_open();
    connection_close();
    connection_close();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    assert_eq!(after - before, 0);
}

// ── prometheus_text format ────────────────────────────────────────────────────

#[test]
fn prometheus_text_contains_required_metric_names() {
    let text = prometheus_text();
    assert!(text.contains("rws_requests_total"),    "missing rws_requests_total");
    assert!(text.contains("rws_errors_total"),       "missing rws_errors_total");
    assert!(text.contains("rws_active_connections"), "missing rws_active_connections");
}

#[test]
fn prometheus_text_contains_help_and_type_lines() {
    let text = prometheus_text();
    let help_count = text.lines().filter(|l| l.starts_with("# HELP")).count();
    let type_count = text.lines().filter(|l| l.starts_with("# TYPE")).count();
    // At least 3 server-wide + possibly route metrics headers.
    assert!(help_count >= 3, "expected at least 3 # HELP lines, got {}", help_count);
    assert!(type_count >= 3, "expected at least 3 # TYPE lines, got {}", type_count);
}

#[test]
fn prometheus_text_type_annotations_are_correct() {
    let text = prometheus_text();
    assert!(text.contains("# TYPE rws_requests_total counter"));
    assert!(text.contains("# TYPE rws_errors_total counter"));
    assert!(text.contains("# TYPE rws_active_connections gauge"));
}

#[test]
fn prometheus_text_metric_values_are_parseable_numbers() {
    record_request();
    let text = prometheus_text();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Lines look like: name{labels} value  OR  name value
        let value_part = line.rsplit(' ').next().expect("no value on metric line");
        value_part.parse::<f64>().unwrap_or_else(|_| {
            panic!("metric value is not a number on line: '{}'", line)
        });
    }
}

#[test]
fn prometheus_text_requests_reflect_recorded_requests() {
    let before_val: u64 = prometheus_text()
        .lines()
        .find(|l| l.starts_with("rws_requests_total "))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .expect("rws_requests_total line not found");

    record_request();
    record_request();

    let after_val: u64 = prometheus_text()
        .lines()
        .find(|l| l.starts_with("rws_requests_total "))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .expect("rws_requests_total line not found");

    assert!(after_val >= before_val + 2);
}

// ── per-route record_route ────────────────────────────────────────────────────

#[test]
fn record_route_appears_in_prometheus_text() {
    record_route("GET", "/__metrics_test_appear__", 200, 0.001);
    let text = prometheus_text();
    assert!(
        text.contains("/__metrics_test_appear__"),
        "route path not found in prometheus output"
    );
}

#[test]
fn record_route_counts_accumulate_for_same_key() {
    let path = "/__metrics_test_count__";
    record_route("GET", path, 200, 0.01);
    record_route("GET", path, 200, 0.01);
    record_route("GET", path, 200, 0.01);

    let text = prometheus_text();
    // Find the counter line for this path with status 200.
    let count: u64 = text.lines()
        .find(|l| l.contains(path) && l.contains("status=\"200\"") && l.contains("rws_route_requests_total"))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
        .expect("counter line not found");
    assert!(count >= 3, "expected at least 3, got {}", count);
}

#[test]
fn record_route_separates_by_status() {
    let path = "/__metrics_test_status__";
    record_route("GET", path, 200, 0.01);
    record_route("GET", path, 404, 0.01);

    let text = prometheus_text();
    assert!(text.contains(&format!("path=\"{}\"", path)));

    let has_200 = text.lines().any(|l| l.contains(path) && l.contains("status=\"200\""));
    let has_404 = text.lines().any(|l| l.contains(path) && l.contains("status=\"404\""));
    assert!(has_200, "missing 200 status line");
    assert!(has_404, "missing 404 status line");
}

#[test]
fn record_route_separates_by_method() {
    let path = "/__metrics_test_method__";
    record_route("GET",  path, 200, 0.01);
    record_route("POST", path, 201, 0.02);

    let text = prometheus_text();
    let has_get  = text.lines().any(|l| l.contains(path) && l.contains("method=\"GET\""));
    let has_post = text.lines().any(|l| l.contains(path) && l.contains("method=\"POST\""));
    assert!(has_get,  "missing GET method line");
    assert!(has_post, "missing POST method line");
}

#[test]
fn histogram_includes_bucket_sum_and_count_lines() {
    let path = "/__metrics_test_histogram__";
    record_route("GET", path, 200, 0.003); // falls in ≤0.005 bucket
    record_route("GET", path, 200, 0.008); // falls in ≤0.01 bucket

    let text = prometheus_text();
    let bucket_line = text.lines()
        .find(|l| l.contains(path) && l.contains("_bucket{") && l.contains("le=\"+Inf\""))
        .expect("no +Inf bucket line found");
    let inf_count: u64 = bucket_line.rsplit(' ').next()
        .and_then(|v| v.parse().ok())
        .expect("could not parse +Inf count");
    assert!(inf_count >= 2);

    assert!(text.lines().any(|l| l.contains(path) && l.contains("rws_route_duration_seconds_sum")));
    assert!(text.lines().any(|l| l.contains(path) && l.contains("rws_route_duration_seconds_count")));
}

#[test]
fn histogram_buckets_are_cumulative() {
    let path = "/__metrics_test_cumulative__";
    // Both observations are ≤ 0.05 s, so all buckets ≥ 0.05 must have count ≥ 2.
    record_route("GET", path, 200, 0.001);
    record_route("GET", path, 200, 0.04);

    let text = prometheus_text();

    // Bucket ≤ 0.005: only the 0.001 observation fits.
    let bucket_005: u64 = text.lines()
        .find(|l| l.contains(path) && l.contains("le=\"0.005\""))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // Bucket ≤ 0.05: both observations fit.
    let bucket_05: u64 = text.lines()
        .find(|l| l.contains(path) && l.contains("le=\"0.05\""))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    assert!(bucket_005 >= 1, "≤0.005 bucket should have at least 1");
    assert!(bucket_05  >= 2, "≤0.05 bucket should have at least 2");
    assert!(bucket_05 >= bucket_005, "buckets must be non-decreasing");
}

// ── MetricsLayer middleware ───────────────────────────────────────────────────

#[test]
fn metrics_layer_records_route_on_success() {
    let path = "/__metrics_test_layer_ok__";
    let app = FixedApp(ok_response());
    let layer = MetricsLayer;
    layer.handle(&get(path), &conn(), &app).unwrap();

    let text = prometheus_text();
    assert!(text.contains(path), "route path not found after MetricsLayer call");
}

#[test]
fn metrics_layer_records_500_on_handler_error() {
    let path = "/__metrics_test_layer_err__";
    let layer = MetricsLayer;
    let _ = layer.handle(&get(path), &conn(), &ErrorApp);

    let text = prometheus_text();
    let has_500 = text.lines()
        .any(|l| l.contains(path) && l.contains("status=\"500\""));
    assert!(has_500, "expected status=500 after handler error");
}

#[test]
fn metrics_layer_strips_query_string_from_path() {
    let layer = MetricsLayer;
    let app = FixedApp(ok_response());
    let req = get("/__metrics_test_query__?page=2&limit=10");
    layer.handle(&req, &conn(), &app).unwrap();

    let text = prometheus_text();
    // Path stored must not include the query string.
    assert!(
        text.contains("/__metrics_test_query__"),
        "stripped path not found"
    );
    assert!(
        !text.contains("page=2"),
        "query string must not appear in the metric path label"
    );
}

#[test]
fn metrics_layer_returns_handler_response_unchanged() {
    let mut expected = ok_response();
    expected.status_code = 201;
    expected.reason_phrase = "Created".to_string();

    let layer = MetricsLayer;
    let app = FixedApp(expected.clone());
    let result = layer.handle(&get("/__metrics_test_passthrough__"), &conn(), &app).unwrap();

    assert_eq!(201, result.status_code);
    assert_eq!("Created", result.reason_phrase);
}

#[test]
fn metrics_layer_propagates_handler_error() {
    let layer = MetricsLayer;
    let result = layer.handle(&get("/__metrics_test_err_prop__"), &conn(), &ErrorApp);
    assert!(result.is_err());
}

#[test]
fn route_prometheus_text_contains_correct_help_and_type_lines() {
    // Trigger at least one route observation so the section is emitted.
    record_route("GET", "/__metrics_test_headers__", 200, 0.01);
    let text = prometheus_text();
    assert!(text.contains("# HELP rws_route_requests_total"));
    assert!(text.contains("# TYPE rws_route_requests_total counter"));
    assert!(text.contains("# HELP rws_route_duration_seconds"));
    assert!(text.contains("# TYPE rws_route_duration_seconds histogram"));
}
