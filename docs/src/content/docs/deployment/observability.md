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
| `rws_errors_total` | Counter | Connections whose handling ended in an error (I/O or application error, or a panicked handler) — not a count of 5xx responses |
| `rws_active_connections` | Gauge | Number of currently open connections |
| `http_server_request_duration_seconds` | Histogram | OpenTelemetry `http.server.request.duration` by method, status, scheme and protocol version — a [native histogram](/features/metrics/#native-histograms) for a protobuf scrape |

### Per-route metrics with MetricsLayer

Add `MetricsLayer` to any application to record per-route counters and latency histograms:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::metrics::MetricsLayer;

let app = App::new().wrap(MetricsLayer);
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

### TLS

`rws_tls_handshakes_total{tls_protocol_version,tls_cipher,tls_group}`, `rws_tls_handshake_failures_total` and `rws_tls_certificate_expiry_timestamp_seconds{name}` — see [TLS metrics](/features/metrics/#tls). Alert on `rws_tls_certificate_expiry_timestamp_seconds - time() < 14 * 86400`.

### Upstreams

Config-file proxies export `rws_upstream_requests_total`, `rws_upstream_request_duration_seconds`, `rws_upstream_retries_total`, `rws_upstream_backend_up` and `rws_upstream_circuit_breaker_state` per `upstream` and `backend` — see [Upstream metrics](/features/metrics/#upstream-metrics).

### OTLP metrics export

`http.server.request.duration` can also be pushed to an OpenTelemetry Collector (or any OTLP/HTTP endpoint: Grafana Cloud, Honeycomb, Datadog's OTLP intake) — the same data `/metrics` serves, so the two never disagree. The `rws` binary starts the exporter when an endpoint is set:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4318   # metrics go to /v1/metrics, spans to /v1/traces
export OTEL_SERVICE_NAME=edge-1                                   # default rws
export OTEL_METRIC_EXPORT_INTERVAL=15000                          # ms, default 60000
export OTEL_EXPORTER_OTLP_HEADERS="api-key=secret"               # optional, key=value,…
rws
```

| Variable | Meaning |
|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector base URL; `/v1/metrics` and `/v1/traces` are appended |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | Full metrics URL, used as is (wins over the base) |
| `OTEL_METRIC_EXPORT_INTERVAL` | Milliseconds between exports (default `60000`) |
| `OTEL_METRICS_EXPORTER` | `none` turns metrics export off |
| `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_EXPORTER_OTLP_METRICS_HEADERS` | Request headers, `key=value` pairs separated by commas (values percent-decoded) |
| `OTEL_SERVICE_NAME` | `service.name` resource attribute |

Each export is an OTLP/HTTP JSON `ExportMetricsServiceRequest` with cumulative temporality and the convention's explicit buckets, attributes `http.request.method`, `http.response.status_code`, `url.scheme`, `network.protocol.name` and `network.protocol.version`. A last export is sent on SIGTERM/SIGINT. From Rust: `otel::metrics::start(MetricsExportConfig { .. })` or `otel::metrics::start_from_env()`, and `otel::shutdown()` before exit.

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

### JSON structured logs (default)

By default the server writes one newline-delimited JSON object per request to stdout:

```json
{"time":"2026-07-02T12:00:00Z","remote_addr":"127.0.0.1","method":"GET","path":"/healthz","protocol":"HTTP/1.1","status":200,"bytes":2}
```

JSON logs are well-suited for ingestion by Loki, Fluentd, Datadog, and other structured-log collectors.

### Combined Log Format

Set `RWS_CONFIG_LOG_FORMAT=combined` (via environment variable or `rws.config.toml`) for Apache Combined Log Format (CLF):

```
127.0.0.1 - - [02/Jul/2026:12:00:00 +0000] "GET /healthz HTTP/1.1" 200 2
```

This format is compatible with GoAccess, AWStats, and most log-analysis tools.

To produce the same line yourself — in your own middleware or a custom server loop — call `Log::combined`:

```rust
use rust_web_server::log::Log;

// request: &Request, response: &Response, peer_addr: &SocketAddr
let line = Log::combined(&request, &response, &peer_addr);
println!("{}", line);
// 192.168.1.1 - - [29/Jun/2026:14:23:05 +0000] "GET /index.html HTTP/1.1" 200 1234
```

`RWS_CONFIG_LOG_FORMAT=off` writes no access log at all.

### Buffered writes

Access-log lines are buffered per worker thread and written to stdout by a
background thread every 100 ms — at once when a thread's buffer passes
32 KiB, when a thread exits, and when the process exits normally (including
a graceful shutdown on `SIGTERM`/`SIGINT`). Writing each line with its own
`write` under the process-wide stdout lock cost about a sixth of a small
request's CPU. A line can therefore appear up to 100 ms after its request;
lines from one thread keep their order, and every line carries its own
timestamp. Code that ends the process some other way can call
`Log::flush_access_log()` first.

### Writing to a file

Set `access_log` (`RWS_CONFIG_ACCESS_LOG`, since 18.1.0) to write the access
log to a file instead of stdout:

```toml
log_format          = "combined"
access_log          = "/var/log/rws/access.log"
access_log_max_size = "100M"    # optional: rotate past 100 MiB
access_log_keep     = 5         # access.log.1 … access.log.5
```

- Lines are appended; the file and its directory must be writable by rws.
- **Rotation by rws:** with `access_log_max_size`, the file is renamed
  `access.log.1` (older ones shift up, the oldest past `access_log_keep` is
  deleted) and a new one is started when the next lines would pass the
  size.
- **Rotation by logrotate:** leave `access_log_max_size` at `0` and have
  logrotate send `SIGUSR1` after moving the file; rws reopens the path
  before its next write:

  ```
  /var/log/rws/access.log {
      daily
      rotate 14
      compress
      delaycompress
      missingok
      notifempty
      postrotate
          systemctl kill -s USR1 rws.service
      endscript
  }
  ```

  Without an access-log file `SIGUSR1` keeps its default action (it ends
  the process), so don't send it to an rws logging to stdout.

### Log aggregation in Kubernetes

The server writes all logs to stdout. Kubernetes log aggregation pipelines pick them up automatically:

```sh
# View logs in real time
kubectl logs -f deployment/my-rws-app

# Ship to Loki via Promtail — no application config required; just label the pod
```


## Tracing

### OtelLayer

`OtelLayer` implements `Middleware` and propagates W3C `traceparent` headers. Add it to your application and configure an OTLP exporter:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::otel::{self, ExporterConfig, OtelLayer, TracingConfig};

otel::setup(TracingConfig {
    service_name: "my-service".to_string(),
    exporter: ExporterConfig::Otlp {
        endpoint: "http://tempo:4318".to_string(),
    },
    ..TracingConfig::default()
});

let app = App::new().wrap(OtelLayer);
```

The layer reads the incoming `traceparent` header, creates a span for each request (a child of the caller's span when one was sent), and exports completed spans to the configured OTLP endpoint. Start nested spans inside a request with `otel::span(name)` / `otel::client_span(name)` — see [Distributed Tracing](/features/tracing/#child-spans).

The `rws` binary adds `OtelLayer` and exports spans by itself when `OTEL_EXPORTER_OTLP_ENDPOINT` is set — see [Environment variable setup](/features/tracing/#environment-variable-setup).

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

Inside a traced request rws does this itself: the HTTP client, the proxies and database statements get client spans, and `traceparent` plus [`baggage`](/features/tracing/#baggage) are sent on — see [Automatic client spans and propagation](/features/tracing/#automatic-client-spans-and-propagation).

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
