---
title: Observability
description: Metrics, structured logs, distributed tracing, and health endpoints for rust-web-server.
---

## Metrics

### Built-in Prometheus endpoint

`GET /metrics` is always available and returns metrics in the Prometheus text exposition format. No configuration is required.

Server-wide counters updated by the request loop:

| Metric | Type | Description |
|---|---|---|
| `rws_requests_total` | Counter | Total HTTP requests received |
| `rws_errors_total` | Counter | Total requests that produced a 5xx response |
| `rws_active_connections` | Gauge | Number of currently open connections |

### Per-route metrics with MetricsLayer

Add `MetricsLayer` to any application to record per-route counters and latency histograms:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::metrics::MetricsLayer;

let app = App::new().wrap(MetricsLayer::new());
```

`MetricsLayer` adds:

| Metric | Type | Labels | Description |
|---|---|---|---|
| `rws_route_requests_total` | Counter | `method`, `path`, `status` | Requests per route and status code |
| `rws_route_duration_seconds` | Histogram | `method`, `path` | Response time distribution |

### Circuit breaker state

Present automatically — no `MetricsLayer` or other opt-in needed — for every backend known to [`circuit_breaker::global()`](/proxy/circuit-breaker/):

| Metric | Type | Labels | Description |
|---|---|---|---|
| `rws_circuit_breaker_state` | Gauge | `backend` | `0`=closed, `1`=half_open, `2`=open |

Wire `ReverseProxy::with_circuit_breaker(Arc::new(circuit_breaker::global()))` to get this metric for free alongside [automatic breaker integration](/proxy/circuit-breaker/#automatic-reverseproxy-wiring).

### Grafana / PromQL queries

```promql
# Request rate (requests per second over 1 m)
rate(rws_requests_total[1m])

# Error rate
rate(rws_errors_total[1m])

# p99 latency per route
histogram_quantile(0.99, rate(rws_route_duration_seconds_bucket[5m]))

# Top routes by request count
topk(10, sum by (path) (rate(rws_route_requests_total[5m])))

# Backends currently tripped open
rws_circuit_breaker_state == 2
```

## Logs

### Combined Log Format (default)

By default the server writes one line per request to stdout in Apache Combined Log Format (CLF):

```
127.0.0.1 - - [02/Jul/2026:12:00:00 +0000] "GET /healthz HTTP/1.1" 200 2
```

This format is compatible with GoAccess, AWStats, and most log-analysis tools.

### JSON structured logs

Set `RWS_CONFIG_LOG_FORMAT=json` (via environment variable, `rws.config.toml`, or CLI flag) to switch to newline-delimited JSON:

```json
{"time":"2026-07-02T12:00:00Z","method":"GET","path":"/healthz","status":200,"bytes":2,"client":"127.0.0.1"}
```

JSON logs are well-suited for ingestion by Loki, Fluentd, Datadog, and other structured-log collectors.

### Log aggregation in Kubernetes

The server writes all logs to stdout. Kubernetes log aggregation pipelines pick them up automatically:

```sh
# View logs in real time
kubectl logs -f deployment/my-rws-app

# Ship to Loki via Promtail — no application config required; just label the pod
```

:::caution[Coming Soon]
Access log rotation. Currently, redirect stdout to a log file and use an external tool such as `logrotate` if you need rotation outside of Kubernetes.
:::

## Tracing

### OtelLayer

`OtelLayer` implements `Middleware` and propagates W3C `traceparent` headers. Add it to your application and configure an OTLP exporter:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::otel::{self, ExporterConfig, OtelLayer};

otel::setup(ExporterConfig::Otlp {
    endpoint: "http://tempo:4318".to_string(),
});

let app = App::new().wrap(OtelLayer::new());
```

The layer reads the incoming `traceparent` header, creates a child span for each request, and exports completed spans to the configured OTLP endpoint.

### Supported backends

- **Grafana Tempo** — point `endpoint` at the OTLP HTTP port (default `4318`)
- **Jaeger** — enable the OTLP receiver and point at port `4318`
- **OpenTelemetry Collector** — use as a fan-out proxy to multiple backends

### Distributed tracing across services

Include the `traceparent` header in outbound requests using the synchronous HTTP client:

```rust
use rust_web_server::http_client::Client;

let response = Client::new()
    .get("http://downstream-service/api/data")
    .header("traceparent", incoming_traceparent)
    .send()?;
```

:::caution[Coming Soon]
Child spans and baggage propagation within a single request (multi-span tracing). Currently one span is created per inbound request.
:::

## Health endpoints

Two health endpoints are built into every server instance without any configuration:

### GET /healthz — liveness

Returns `200 OK` as long as the binary is running. Use this as the Kubernetes liveness probe:

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 7878
  initialDelaySeconds: 5
  periodSeconds: 10
```

### GET /readyz — readiness

Returns `200 OK` during normal operation. Returns `503 Service Unavailable` during graceful shutdown (after `SIGTERM` is received, while in-flight requests are being drained). Use as the Kubernetes readiness probe:

```yaml
readinessProbe:
  httpGet:
    path: /readyz
    port: 7878
  initialDelaySeconds: 3
  periodSeconds: 5
```

The readiness gate ensures the load balancer stops routing new traffic to a pod as soon as it begins draining, giving in-flight requests time to complete before the pod terminates.
