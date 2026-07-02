---
title: Distributed Tracing
description: OpenTelemetry-compatible HTTP server spans via OtelLayer middleware, with W3C traceparent propagation.
---

`rust-web-server` includes an OpenTelemetry-compatible distributed tracing implementation in `src/otel/mod.rs`. `OtelLayer` middleware creates an HTTP server span for every request, reads and propagates W3C `traceparent` headers, and exports spans to stdout or an OTLP-compatible collector (Jaeger, Grafana Tempo, OpenTelemetry Collector).

## Setup

Call `otel::setup()` once at startup before the server starts accepting requests. Calling it more than once is a no-op — the first call wins.

### Development: print spans to stdout

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::otel::{OtelLayer, TracingConfig, ExporterConfig};

rust_web_server::otel::setup(TracingConfig {
    service_name: "my-service".to_string(),
    service_version: env!("CARGO_PKG_VERSION").to_string(),
    exporter: ExporterConfig::Stdout,
    sample_rate: 1.0,
    batch_size: 128,
});

let app = App::new().wrap(OtelLayer);
```

Each completed span is printed as a single JSON line to stdout, suitable for piping into `jq` or a log aggregator.

### Production: OTLP HTTP export

```rust
use rust_web_server::otel::{ExporterConfig, TracingConfig};

rust_web_server::otel::setup(TracingConfig {
    service_name: "my-service".to_string(),
    service_version: "1.0.0".to_string(),
    exporter: ExporterConfig::Otlp {
        endpoint: "http://jaeger:4318".to_string(),
    },
    sample_rate: 0.1,   // sample 10 % of requests
    batch_size: 512,
});
```

The OTLP exporter sends spans to `POST {endpoint}/v1/traces` using JSON encoding. It is compatible with Jaeger >= 1.35, Grafana Tempo, and the OpenTelemetry Collector. The default OTLP HTTP port is `4318`.

### Environment variable setup

`setup_from_env()` reads standard OpenTelemetry environment variables. Use this when the exporter endpoint is injected by the platform (Kubernetes, Docker Compose, etc.):

```bash
export OTEL_SERVICE_NAME=my-service
export OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4318
export OTEL_TRACES_SAMPLER_ARG=0.1   # 0.0–1.0, default 1.0
```

```rust
rust_web_server::otel::setup_from_env();
let app = App::new().wrap(OtelLayer);
```

When `OTEL_EXPORTER_OTLP_ENDPOINT` is absent or empty, the exporter falls back to stdout.

## `TracingConfig` fields

| Field | Type | Description |
|-------|------|-------------|
| `service_name` | `String` | `service.name` resource attribute |
| `service_version` | `String` | `service.version` resource attribute |
| `exporter` | `ExporterConfig` | `Stdout`, `Otlp { endpoint }`, or `Discard` |
| `sample_rate` | `f64` | Fraction of requests to trace (`0.0`–`1.0`) |
| `batch_size` | `usize` | Spans accumulated before a flush to the exporter |

## W3C `traceparent` propagation

`OtelLayer` reads the `traceparent` header from each incoming request using the W3C Trace Context format (`00-{trace-id}-{parent-id}-{flags}`):

- If present, the existing trace ID is continued and the incoming span ID becomes the parent.
- If absent, a new trace ID is generated.

The active span context is stored in **thread-local storage** for the duration of the request so downstream middleware (such as `ReverseProxy`) can inject the header into upstream calls automatically.

## `otel::current_traceparent()`

Retrieve the `traceparent` value for the span currently being processed on the calling thread. Use this to propagate trace context into outbound HTTP requests made inside a handler:

```rust
use rust_web_server::otel;
use rust_web_server::http_client::Client;

// Inside a handler, while OtelLayer is active:
let mut builder = Client::new().get("http://other-service/api/data");
if let Some(tp) = otel::current_traceparent() {
    builder = builder.header("traceparent", &tp);
}
let response = builder.send()?;
```

Returns `None` when no `OtelLayer` span is active on the current thread.

## Span attributes

Each span records standard HTTP semantic attributes:

| Attribute | Example |
|-----------|---------|
| `http.method` | `"GET"` |
| `http.target` | `"/api/users?page=2"` |
| `http.status_code` | `200` |
| span `name` | `"GET /api/users"` (query stripped) |

Spans with a 5xx status code receive `status.code = 2` (Error). All others receive `0` (Unset).

## Shutdown

Flush all buffered spans before the process exits:

```rust
rust_web_server::otel::shutdown();
```

Calling `shutdown()` flushes the internal batch and invokes the exporter's own shutdown hook. Without this call, the last batch of spans may not be delivered.

## Testing

Use `ExporterConfig::Discard` in tests to silence output without changing application code paths:

```rust
rust_web_server::otel::setup(TracingConfig {
    exporter: ExporterConfig::Discard,
    ..TracingConfig::default()
});
```

`flush()` is also available for non-destructive flushing in test assertions.
