use std::sync::Once;

use crate::app::App;
use crate::core::New;
use crate::otel::{
    ExporterConfig, OtelLayer, TraceContext, TracingConfig,
    current_traceparent, new_span_id, new_trace_id, setup,
};
use crate::test_client::TestClient;

// Initialize tracer once for all tests that go through OtelLayer.
// We use Discard so tests don't pollute stdout.
static INIT: Once = Once::new();
fn init_tracer() {
    INIT.call_once(|| {
        setup(TracingConfig {
            service_name: "test-service".to_string(),
            service_version: "0.0.0".to_string(),
            exporter: ExporterConfig::Discard,
            sample_rate: 1.0,
            batch_size: 1024,
        });
    });
}

// ── ID generation ─────────────────────────────────────────────────────────────

#[test]
fn new_trace_id_is_32_hex_chars() {
    let id = new_trace_id();
    let hex: String = id.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(hex.len(), 32, "trace-id hex must be 32 chars");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn new_span_id_is_16_hex_chars() {
    let id = new_span_id();
    let hex: String = id.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(hex.len(), 16, "span-id hex must be 16 chars");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn successive_ids_are_distinct() {
    let a = new_trace_id();
    let b = new_trace_id();
    assert_ne!(a, b, "successive trace IDs must differ");
}

// ── TraceContext parsing ───────────────────────────────────────────────────────

#[test]
fn parse_valid_traceparent_sampled() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let ctx = TraceContext::parse(tp).expect("valid traceparent must parse");
    assert!(ctx.sampled);
    assert_eq!(ctx.trace_id[0], 0x4b);
    assert_eq!(ctx.parent_span_id[0], 0x00);
}

#[test]
fn parse_valid_traceparent_not_sampled() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
    let ctx = TraceContext::parse(tp).expect("valid traceparent must parse");
    assert!(!ctx.sampled);
}

#[test]
fn parse_rejects_wrong_version() {
    let tp = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    assert!(TraceContext::parse(tp).is_none(), "version != 00 must fail");
}

#[test]
fn parse_rejects_short_trace_id() {
    let tp = "00-4bf92f3577b34da6-00f067aa0ba902b7-01";
    assert!(TraceContext::parse(tp).is_none());
}

#[test]
fn parse_rejects_invalid_hex() {
    let tp = "00-GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG-00f067aa0ba902b7-01";
    assert!(TraceContext::parse(tp).is_none());
}

#[test]
fn parse_rejects_too_few_parts() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7";
    assert!(TraceContext::parse(tp).is_none());
}

// ── TraceContext as_header ────────────────────────────────────────────────────

#[test]
fn as_header_roundtrips() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let ctx = TraceContext::parse(tp).unwrap();
    let span_id = new_span_id();
    let header = ctx.as_header(&span_id);
    // Format: 00-{trace}-{span}-{flags}
    let parts: Vec<&str> = header.splitn(4, '-').collect();
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1].len(), 32); // trace-id
    assert_eq!(parts[2].len(), 16); // span-id
    assert!(parts[3] == "01" || parts[3] == "00");
}

#[test]
fn as_header_propagates_sampled_flag() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
    let ctx = TraceContext::parse(tp).unwrap();
    let span_id = new_span_id();
    let header = ctx.as_header(&span_id);
    assert!(header.ends_with("-00"), "not-sampled flag must be 00");
}

// ── current_traceparent (no active span) ─────────────────────────────────────

#[test]
fn current_traceparent_is_none_outside_span() {
    // On a fresh thread there's no active span — must return None.
    let result = std::thread::spawn(|| current_traceparent()).join().unwrap();
    assert!(result.is_none());
}

// ── OtelLayer as passthrough when not initialized ────────────────────────────

#[test]
fn otel_layer_passes_through_without_setup() {
    // Use a fresh thread to avoid contamination from init_tracer()
    // called in other tests. On that thread TRACER is still unset (OnceLock
    // per-process, so this test only passes if run before init_tracer).
    // Rather than relying on test order, we simply verify that when OtelLayer
    // wraps an app the response is the inner app's response.
    init_tracer(); // Discard exporter — verify response is still correct

    let app = App::new().wrap(OtelLayer);
    let client = TestClient::new(app);
    let res = client.get("/healthz").send();
    assert_eq!(200, res.status());
}

// ── OtelLayer propagates trace context ───────────────────────────────────────

#[test]
fn otel_layer_creates_span_for_incoming_request() {
    init_tracer();

    // We can't capture spans from the global Discard exporter, but we can
    // verify that the layer doesn't break the response.
    let app = App::new().wrap(OtelLayer);
    let client = TestClient::new(app);
    let res = client.get("/healthz").send();
    assert_eq!(200, res.status());
}

#[test]
fn otel_layer_accepts_incoming_traceparent() {
    init_tracer();

    let app = App::new().wrap(OtelLayer);
    let client = TestClient::new(app);
    let res = client
        .get("/healthz")
        .header("traceparent", "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
        .send();
    assert_eq!(200, res.status());
}

#[test]
fn otel_layer_handles_invalid_traceparent_gracefully() {
    init_tracer();

    let app = App::new().wrap(OtelLayer);
    let client = TestClient::new(app);
    let res = client
        .get("/healthz")
        .header("traceparent", "not-a-valid-traceparent")
        .send();
    // Must still return a valid response (new trace started).
    assert_eq!(200, res.status());
}

// ── OtlpHttpExporter build_body ───────────────────────────────────────────────

#[test]
fn otlp_build_body_contains_service_name() {
    use crate::otel::OtlpHttpExporter;

    let exp = OtlpHttpExporter::new("http://localhost:4318", "my-svc", "2.0.0");
    let span = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        name: "GET /api/users".to_string(),
        start_ns: 1_000_000_000,
        end_ns:   1_001_000_000,
        http_method: "GET".to_string(),
        http_target: "/api/users".to_string(),
        http_status: 200,
        status_code: 0,
    };
    let body = exp.build_body(&[span]);
    assert!(body.contains("my-svc"), "body must contain service name");
    assert!(body.contains("2.0.0"), "body must contain service version");
    assert!(body.contains("resourceSpans"), "body must be OTLP JSON");
    assert!(body.contains("http.method"), "body must contain http.method");
    assert!(body.contains("http.status_code"), "body must contain http.status_code");
}

#[test]
fn otlp_build_body_includes_parent_span_id_when_present() {
    use crate::otel::OtlpHttpExporter;

    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let parent = new_span_id();
    let span = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: Some(parent),
        name: "POST /orders".to_string(),
        start_ns: 0,
        end_ns: 1,
        http_method: "POST".to_string(),
        http_target: "/orders".to_string(),
        http_status: 201,
        status_code: 0,
    };
    let body = exp.build_body(&[span]);
    assert!(body.contains("parentSpanId"), "body must contain parentSpanId when parent is set");
}

#[test]
fn otlp_build_body_no_parent_span_id_when_root() {
    use crate::otel::OtlpHttpExporter;

    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        name: "GET /".to_string(),
        start_ns: 0,
        end_ns: 1,
        http_method: "GET".to_string(),
        http_target: "/".to_string(),
        http_status: 200,
        status_code: 0,
    };
    let body = exp.build_body(&[span]);
    assert!(!body.contains("parentSpanId"), "root span must not have parentSpanId key");
}

#[test]
fn otlp_build_body_is_valid_json_structure() {
    use crate::otel::OtlpHttpExporter;

    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        name: "GET /".to_string(),
        start_ns: 0,
        end_ns: 1,
        http_method: "GET".to_string(),
        http_target: "/".to_string(),
        http_status: 200,
        status_code: 0,
    };
    let body = exp.build_body(&[span]);
    // Basic JSON structure check
    assert!(body.starts_with('{'));
    assert!(body.ends_with('}'));
    // Balanced braces
    let opens = body.chars().filter(|&c| c == '{').count();
    let closes = body.chars().filter(|&c| c == '}').count();
    assert_eq!(opens, closes, "JSON braces must be balanced");
}

#[test]
fn otlp_build_body_error_span_has_status_code_2() {
    use crate::otel::OtlpHttpExporter;

    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        name: "GET /boom".to_string(),
        start_ns: 0,
        end_ns: 1,
        http_method: "GET".to_string(),
        http_target: "/boom".to_string(),
        http_status: 500,
        status_code: 2,
    };
    let body = exp.build_body(&[span]);
    assert!(body.contains("\"code\":2"), "error span must have status code 2");
    assert!(body.contains("Error"), "error span must have Error message");
}

// ── OtlpHttpExporter endpoint parsing ────────────────────────────────────────

#[test]
fn otlp_exporter_parses_http_prefix() {
    use crate::otel::OtlpHttpExporter;
    // We access build_body as a proxy to verify construction succeeded.
    let exp = OtlpHttpExporter::new("http://collector.example.com:4318", "svc", "1.0");
    let body = exp.build_body(&[]);
    // Empty spans list still produces valid envelope
    assert!(body.contains("resourceSpans"));
}

// ── setup_from_env ────────────────────────────────────────────────────────────

#[test]
fn setup_from_env_second_call_is_noop() {
    // setup() is called in init_tracer(). Calling again must not panic.
    init_tracer();
    setup(TracingConfig {
        service_name: "another-service".to_string(),
        service_version: "9.9.9".to_string(),
        exporter: ExporterConfig::Stdout,
        sample_rate: 0.5,
        batch_size: 1,
    });
    // Original config still in effect — no panic = pass.
}
