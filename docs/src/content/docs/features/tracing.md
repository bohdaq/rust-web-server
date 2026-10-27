---
title: Distributed Tracing
description: OpenTelemetry-compatible HTTP server spans and nested child spans via OtelLayer and otel::span, with W3C traceparent propagation.
---

`rust-web-server` includes an OpenTelemetry-compatible distributed tracing implementation in `src/otel/mod.rs`. `OtelLayer` middleware creates an HTTP server root span for every request, reads and propagates W3C `traceparent` headers, and exports spans to stdout or an OTLP-compatible collector (Jaeger, Grafana Tempo, OpenTelemetry Collector). Application code can nest child spans under that root (or under each other) with `otel::span`/`otel::client_span` — see [Child spans](#child-spans) below.

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

The OTLP exporter sends spans to `POST {endpoint}/v1/traces` (an `endpoint` already ending in `/v1/traces` is used as is; `https://` works) using JSON encoding, from a background thread: a full batch or one that has waited 5 seconds goes out without holding up a request, and a batch the collector can't take in time is dropped rather than queued without bound. `OtlpHttpExporter::new(..).with_headers(..)` adds headers such as an API key, through `setup_with_exporter`. It is compatible with Jaeger >= 1.35, Grafana Tempo, and the OpenTelemetry Collector. The default OTLP HTTP port is `4318`.

### Environment variable setup

`setup_from_env()` reads standard OpenTelemetry environment variables. Use this when the exporter endpoint is injected by the platform (Kubernetes, Docker Compose, etc.):

```bash
export OTEL_SERVICE_NAME=my-service
export OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4318
export OTEL_TRACES_SAMPLER_ARG=0.1   # 0.0–1.0, default 1.0
export OTEL_EXPORTER_OTLP_HEADERS="api-key=secret"   # optional, key=value,…
# OTEL_EXPORTER_OTLP_TRACES_ENDPOINT / _TRACES_HEADERS override the shared ones
```

```rust
rust_web_server::otel::setup_from_env();
let app = App::new().wrap(OtelLayer);
```

When no endpoint is set, the exporter falls back to stdout.

The `rws` binary does this by itself: with `OTEL_EXPORTER_OTLP_ENDPOINT` (or `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`) set, and `OTEL_TRACES_EXPORTER` not `none`, it wraps everything — static files, `/mcp`, config-file proxy routes — in `OtelLayer` and exports to it; without one it adds no layer. Proxy routes then go through the thread-pool forwarder rather than the [non-blocking one](/proxy/config-driven/#performance). The same endpoint turns on [OTLP metrics](/deployment/observability/#otlp-metrics-export).

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

Trace and span IDs are random (64 bits from a per-thread generator seeded by the OS), so services in one trace never mint the same ID.

The active span context is stored in a **thread-local stack** for the duration of the request — not just a single slot — so nested [child spans](#child-spans) work correctly: starting a child span pushes it, and it's popped when the child ends, restoring its parent as "current."

## Automatic client spans and propagation

Inside a request traced by `OtelLayer`, rws starts a `Client` span for the outbound calls it makes and sends the trace on — no code needed:

| Outbound call | Span | Headers sent |
|---|---|---|
| `http_client::Client` (`send`) | `GET`/`POST`/… with `http.request.method`, `url.full` (no query or credentials), `server.address`, `server.port`, `http.response.status_code`; an error for no response or a 4xx/5xx | `traceparent` naming the client span, `baggage`; a header the caller set is kept |
| `http_client::AsyncClient` | the same, as a [detached span](#async-code) | the same |
| `ReverseProxy`, config-file proxy routes, canary | the same, per forward (retries are separate spans) | `traceparent` and `baggage` replace the client's; `tracestate` passes through |
| `DbPool` / `DbTransaction` statements (`model-*` features) | `SELECT`/`INSERT`/… with `db.system.name` and `db.operation.name` — not the query text, which can carry literals; an error when the statement fails | — |

Outside a traced request none of this happens: no span is started (outbound calls never begin a trace of their own) and the proxies forward the client's trace headers untouched. The OTLP exporters' own requests run on their threads, outside any request, so they aren't traced. The HTTP/2 upstream (`H2ReverseProxy`, `GrpcProxy`, `grpc` routes) isn't covered yet.

## Baggage

[W3C Baggage](https://www.w3.org/TR/baggage/) — key/value pairs such as a tenant or a feature flag that travel with a request across services — comes in on the `baggage` header, is readable and extendable in a handler, and goes out on every outbound call above:

```rust
use rust_web_server::otel;

// Inside a handler, under OtelLayer:
let tenant = otel::current_baggage().get("tenant").map(str::to_string);
otel::set_baggage("checkout.step", "payment"); // sent on from here on
```

`otel::Baggage` parses and formats the header (`Baggage::parse`, `.to_header()`, `.get`, `.set`, `.remove`, `.iter`). Values are percent-decoded; malformed members are skipped, and the specification's limits apply (64 members, 8192 bytes; the rest is dropped). Baggage isn't added to spans as attributes — it may hold what you wouldn't want in a tracing backend.

## Async code

A `Span` lives on a thread-local stack, but an `async` task can resume on another thread. Two tools cover that:

- **`otel::detached_span(name, kind)`** — a `Send` span whose parent is the span current where it starts; it records when dropped, wherever that is. `span.enter(|| …)` makes it current for a synchronous block (to propagate headers from it, say). rws uses it for database statements and `AsyncClient`. Returns `None` outside a traced request.
- **`otel::Context::current()` / `context.attach()`** — capture the current span and baggage, and make them current on another thread until the guard drops. rws does this for `AsyncAppWithState` handlers, which run on a thread of their own, so their DB queries and outbound calls nest under the request's span.

## `otel::current_traceparent()`

Retrieve the `traceparent` value for the *innermost* span currently active on the calling thread — the request's root span, or whichever child span is currently open, whichever is deeper. Use this to propagate trace context into outbound HTTP requests made inside a handler:

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

Returns `None` when no span is active on the current thread.

## Span attributes

Each span records standard HTTP semantic attributes:

| Attribute | Example |
|-----------|---------|
| `http.method` | `"GET"` |
| `http.target` | `"/api/users?page=2"` |
| `http.status_code` | `200` |
| span `name` | `"GET /api/users"` (query stripped) |

Spans with a 5xx status code receive `status.code = 2` (Error). All others receive `0` (Unset).

## Child spans

`OtelLayer` only creates one span per request — the root. To see *where* time was spent inside that request (a slow DB query vs. a slow upstream call vs. everything else), start a **child span** with `otel::span` or `otel::client_span` wherever that work happens:

```rust
use rust_web_server::otel;

fn get_user(id: u64) -> Result<String, String> {
    let span = otel::span("db.query"); // SpanKind::Internal
    span.set_attribute("db.statement", "SELECT * FROM users WHERE id = ?");
    span.set_attribute("db.user_id", id as i64);

    let result = run_query(id);
    if result.is_err() {
        span.record_error("query failed");
    }
    result
    // `span` is recorded here, when it drops (or call `span.end()` explicitly
    // for the exact same effect, sooner).
}
```

A span created while another is active becomes its **child**: it shares the enclosing span's `trace_id`, and its `parent_span_id` is the enclosing span's `span_id` — this is what makes a proper trace waterfall show up in Jaeger/Tempo instead of a flat list of unrelated spans. Spans can nest arbitrarily deep. A child also **inherits its parent's sampling decision** — if the root wasn't sampled, none of its children are either, so traces are never partially recorded.

| Function | `SpanKind` | Use for |
|---|---|---|
| `otel::span(name)` | `Internal` | Work with no remote counterpart — a DB query, a cache lookup, business logic |
| `otel::client_span(name)` | `Client` | An outbound call to another service that rws doesn't [instrument itself](#automatic-client-spans-and-propagation) — pair with `otel::current_traceparent()` to propagate context into it |
| `Span::new(name, kind)` | any `SpanKind` | Full control, if neither helper fits |

`Span` mutation methods (all take `&self` — no `mut` binding needed):

| Method | Effect |
|---|---|
| `span.set_attribute(key, value)` | Attach a `String`/`i64`/`f64`/`bool` attribute (anything implementing `Into<AttributeValue>`) |
| `span.set_error()` | Mark the span `status.code = 2` (Error) |
| `span.record_error(message)` | `set_error()` plus an `error.message` attribute |
| `span.trace_id()` / `.span_id()` / `.parent_span_id()` | Read the span's own IDs |
| `span.end()` | End the span now — identical to letting it drop at the end of scope |

A `Span` must be dropped on the thread that created it (it's not `Send`) — moving one across threads and dropping it elsewhere would corrupt that other thread's span stack.

## Shutdown

Flush all buffered spans before the process exits:

```rust
rust_web_server::otel::shutdown();
```

Calling `shutdown()` waits for batches already handed to the export thread, flushes the partial one, invokes the exporter's own shutdown hook, and sends a last [metrics export](/deployment/observability/#otlp-metrics-export). Without this call, the last batch of spans may not be delivered. The `rws` binary calls it on SIGTERM/SIGINT.

## Testing

Use `ExporterConfig::Discard` in tests to silence output without changing application code paths:

```rust
rust_web_server::otel::setup(TracingConfig {
    exporter: ExporterConfig::Discard,
    ..TracingConfig::default()
});
```

`flush()` is also available for non-destructive flushing in test assertions.

To assert on the actual spans a test produced, use `setup_with_exporter` with `CapturingExporter` instead of `setup`/`ExporterConfig` — it accepts any `Box<dyn Exporter>` directly:

```rust,no_run
use rust_web_server::otel::{self, CapturingExporter, TracingConfig, ExporterConfig};
use std::sync::Arc;

let captured = Arc::new(CapturingExporter::new());
otel::setup_with_exporter(
    TracingConfig { exporter: ExporterConfig::Discard, ..Default::default() },
    Box::new(captured.clone()),
);

otel::span("db.query").end();
otel::flush();
assert_eq!(1, captured.take().len());
```

Note that `setup`/`setup_with_exporter` initialize a process-wide singleton (first call wins) — in a test binary where other tests also call `setup`, only one of them actually takes effect for the whole process, so this pattern works best in an isolated test binary or when you control which `setup*` call runs first.
