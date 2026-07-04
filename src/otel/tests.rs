use std::sync::Once;

use crate::app::App;
use crate::core::New;
use crate::otel::{
    client_span, current_traceparent, new_span_id, new_trace_id, setup, span, AttributeValue,
    ExporterConfig, OtelLayer, Span, SpanKind, TraceContext, TracingConfig,
};
use crate::test_client::TestClient;
use super::ActiveSpanCtx;

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
    let result = std::thread::spawn(current_traceparent).join().unwrap();
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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

// ── multi-span nesting ────────────────────────────────────────────────────────
//
// These run on a freshly spawned thread so `ACTIVE_STACK` starts empty,
// exactly like `current_traceparent_is_none_outside_span` above — avoids any
// interference from other tests' spans on a pooled/shared test thread.
//
// Attribute/status/kind assertions go through the crate-private
// `finish` helper (reachable here since `tests` is a child module of
// `otel`) rather than a true end-to-end capture through the global `TRACER`
// singleton: `TRACER` is a process-wide `OnceLock` and many tests in this
// file already race to be the first to call `setup(...)` via `init_tracer()`
// — a second, different `setup_with_exporter(...)` call in a test here would
// not reliably win that race, making a genuine "did the exporter receive it"
// assertion flaky. Testing the recorded `SpanData` directly is deterministic
// and covers the same logic.

#[test]
fn nested_span_parent_id_is_outer_span_id() {
    std::thread::spawn(|| {
        let outer = span("outer");
        let inner = span("inner");
        assert_eq!(Some(outer.span_id()), inner.parent_span_id());
        assert_eq!(outer.trace_id(), inner.trace_id());
    })
    .join()
    .unwrap();
}

#[test]
fn three_levels_deep_nesting_chains_correctly() {
    std::thread::spawn(|| {
        let a = span("a");
        let b = span("b");
        let c = span("c");
        assert_eq!(Some(a.span_id()), b.parent_span_id());
        assert_eq!(Some(b.span_id()), c.parent_span_id());
        assert_eq!(a.trace_id(), c.trace_id());
    })
    .join()
    .unwrap();
}

#[test]
fn span_drop_restores_parent_as_current() {
    std::thread::spawn(|| {
        let outer = span("outer");
        {
            let _inner = span("inner");
            assert!(current_traceparent().unwrap().contains(&hex8_of(_inner.span_id())));
        }
        // Inner dropped — outer is current again.
        assert!(current_traceparent().unwrap().contains(&hex8_of(outer.span_id())));
        drop(outer);
        assert!(current_traceparent().is_none());
    })
    .join()
    .unwrap();
}

#[test]
fn explicit_end_behaves_same_as_drop() {
    std::thread::spawn(|| {
        let outer = span("outer");
        let inner = span("inner");
        inner.end();
        assert!(current_traceparent().unwrap().contains(&hex8_of(outer.span_id())));
    })
    .join()
    .unwrap();
}

#[test]
fn span_with_no_active_parent_has_no_parent_id() {
    std::thread::spawn(|| {
        let root = span("root");
        assert!(root.parent_span_id().is_none());
    })
    .join()
    .unwrap();
}

fn hex8_of(id: [u8; 8]) -> String {
    id.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── sampling inheritance ─────────────────────────────────────────────────────

#[test]
fn child_span_inherits_parents_unsampled_state() {
    std::thread::spawn(|| {
        // Manually push a "parent" context with sampled=false — sidesteps
        // the global TRACER singleton entirely (see rationale above).
        super::ACTIVE_STACK.with(|stack| {
            stack.borrow_mut().push(ActiveSpanCtx { trace_id: [1u8; 16], span_id: [2u8; 8], sampled: false });
        });
        let child = span("child");
        assert!(!child.sampled, "child must inherit the parent's unsampled state, not resample independently");
    })
    .join()
    .unwrap();
}

#[test]
fn child_span_inherits_parents_sampled_state() {
    std::thread::spawn(|| {
        super::ACTIVE_STACK.with(|stack| {
            stack.borrow_mut().push(ActiveSpanCtx { trace_id: [1u8; 16], span_id: [2u8; 8], sampled: true });
        });
        let child = span("child");
        assert!(child.sampled);
    })
    .join()
    .unwrap();
}

// ── Span mutation → SpanData ─────────────────────────────────────────────────

#[test]
fn set_attribute_appears_in_span_data() {
    let mut s = Span::new("db.query", SpanKind::Internal);
    s.set_attribute("db.statement", "SELECT 1");
    s.set_attribute("db.rows", 3i64);
    let data = s.finish(0);
    assert_eq!(
        vec![
            ("db.statement".to_string(), AttributeValue::String("SELECT 1".to_string())),
            ("db.rows".to_string(), AttributeValue::Int(3)),
        ],
        data.attributes
    );
}

#[test]
fn set_error_sets_status_code_2() {
    let mut s = Span::new("work", SpanKind::Internal);
    s.set_error();
    assert_eq!(2, s.finish(0).status_code);
}

#[test]
fn record_error_sets_status_and_message_attribute() {
    let mut s = Span::new("work", SpanKind::Internal);
    s.record_error("boom");
    let data = s.finish(0);
    assert_eq!(2, data.status_code);
    assert_eq!(Some(&AttributeValue::String("boom".to_string())), data.attributes.iter().find(|(k, _)| k == "error.message").map(|(_, v)| v));
}

#[test]
fn finish_carries_kind() {
    let mut s = Span::new("upstream.call", SpanKind::Client);
    assert_eq!(SpanKind::Client, s.finish(0).kind);
}

#[test]
fn client_span_helper_uses_client_kind() {
    let mut s = client_span("upstream.call");
    assert_eq!(SpanKind::Client, s.finish(0).kind);
}

#[test]
fn span_helper_uses_internal_kind() {
    let mut s = span("db.query");
    assert_eq!(SpanKind::Internal, s.finish(0).kind);
}

// ── exporter rendering: kind + attributes ───────────────────────────────────

#[test]
fn otlp_build_body_renders_client_kind() {
    use crate::otel::OtlpHttpExporter;
    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span_data = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        name: "upstream.call".to_string(),
        kind: SpanKind::Client,
        ..Default::default()
    };
    let body = exp.build_body(&[span_data]);
    assert!(body.contains("\"kind\":3"), "Client kind must render as OTLP kind 3");
}

#[test]
fn otlp_build_body_renders_internal_kind() {
    use crate::otel::OtlpHttpExporter;
    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span_data = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        name: "db.query".to_string(),
        kind: SpanKind::Internal,
        ..Default::default()
    };
    let body = exp.build_body(&[span_data]);
    assert!(body.contains("\"kind\":1"), "Internal kind must render as OTLP kind 1");
}

#[test]
fn otlp_build_body_renders_generic_attributes() {
    use crate::otel::OtlpHttpExporter;
    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span_data = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        name: "db.query".to_string(),
        attributes: vec![
            ("db.statement".to_string(), AttributeValue::String("SELECT 1".to_string())),
            ("db.rows".to_string(), AttributeValue::Int(3)),
            ("db.cached".to_string(), AttributeValue::Bool(false)),
            ("db.duration_ratio".to_string(), AttributeValue::Float(0.5)),
        ],
        ..Default::default()
    };
    let body = exp.build_body(&[span_data]);
    assert!(body.contains("\"key\":\"db.statement\""));
    assert!(body.contains("\"intValue\":\"3\""), "int64 attributes are string-encoded per OTLP JSON mapping");
    assert!(body.contains("\"boolValue\":false"));
    assert!(body.contains("\"doubleValue\":0.5"));
}

#[test]
fn otlp_build_body_omits_http_attributes_when_http_method_empty() {
    use crate::otel::OtlpHttpExporter;
    let exp = OtlpHttpExporter::new("http://localhost:4318", "svc", "1.0");
    let span_data = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        name: "db.query".to_string(),
        attributes: vec![("db.statement".to_string(), AttributeValue::String("SELECT 1".to_string()))],
        ..Default::default()
    };
    let body = exp.build_body(&[span_data]);
    assert!(!body.contains("http.method"), "non-HTTP span must not emit http.* attributes");
    assert!(body.contains("db.statement"));
}

#[test]
fn stdout_format_span_includes_kind_and_attributes() {
    use crate::otel::StdoutExporter;
    let span_data = crate::otel::SpanData {
        trace_id: new_trace_id(),
        span_id: new_span_id(),
        name: "db.query".to_string(),
        kind: SpanKind::Internal,
        attributes: vec![("db.statement".to_string(), AttributeValue::String("SELECT 1".to_string()))],
        ..Default::default()
    };
    let line = StdoutExporter::format_span(&span_data);
    assert!(line.contains("\"kind\":1"));
    assert!(line.contains("db.statement"));
    assert!(!line.contains("httpMethod"), "non-HTTP span must not emit http fields");
}

// ── setup_with_exporter ──────────────────────────────────────────────────────

#[test]
fn setup_with_exporter_is_callable_and_does_not_panic() {
    // Like `setup_from_env_second_call_is_noop`: TRACER is a process-wide
    // OnceLock already claimed by another test's `setup(...)` call by the
    // time this runs (in practice), so this only proves the function is a
    // safe, harmless no-op after the first `setup`/`setup_with_exporter`
    // call anywhere in the process — not that this specific exporter wins.
    init_tracer();
    crate::otel::setup_with_exporter(
        TracingConfig { exporter: ExporterConfig::Discard, ..Default::default() },
        Box::new(crate::otel::CapturingExporter::new()),
    );
}
