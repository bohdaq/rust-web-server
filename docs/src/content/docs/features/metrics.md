---
title: Metrics
description: Built-in Prometheus metrics — server-wide counters always active, per-route histograms via MetricsLayer middleware.
---

`rust-web-server` exposes two tiers of Prometheus-compatible metrics. Server-wide counters are always active at the server-core level with no configuration. Per-route counters and latency histograms are opt-in via `MetricsLayer` middleware. All metrics are served at `GET /metrics` in the standard Prometheus text format.

## Server-wide counters

These atomics are updated by `Server::process()` and the connection dispatch loop automatically. No middleware registration is required.

| Symbol | Type | Description |
|--------|------|-------------|
| `REQUESTS_TOTAL` | counter | Total HTTP requests handled across all connections and protocols |
| `ERRORS_TOTAL` | counter | Requests where `app.execute()` returned `Err` |
| `ACTIVE_CONNECTIONS` | gauge | Number of currently open TCP/QUIC connections |
| `SERVER_READY` | `AtomicBool` | `true` after server setup; `false` during shutdown; drives `/readyz` |

### Helper functions

```rust
use rust_web_server::metrics;

metrics::record_request();   // increments REQUESTS_TOTAL
metrics::record_error();     // increments ERRORS_TOTAL
metrics::connection_open();  // increments ACTIVE_CONNECTIONS
metrics::connection_close(); // decrements ACTIVE_CONNECTIONS
```

These functions are called by the server core, but you can also call them from custom instrumentation code.

## `GET /metrics` endpoint

The built-in endpoint returns all metrics in Prometheus text format. No setup is needed — it is registered automatically by `App::new()`.

```
# HELP rws_requests_total Total HTTP requests handled
# TYPE rws_requests_total counter
rws_requests_total 12483

# HELP rws_errors_total HTTP requests that returned an application error
# TYPE rws_errors_total counter
rws_errors_total 3

# HELP rws_active_connections Currently open connections
# TYPE rws_active_connections gauge
rws_active_connections 7
```

When `MetricsLayer` is active, per-route metrics are appended to the same response body.

A scraper whose `Accept` header asks for `application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily` (Prometheus does when `scrape_native_histograms` is on) gets the same metrics in the Prometheus protobuf format instead — the only format that carries [native histograms](#native-histograms).

## HTTP server duration (OpenTelemetry)

Every request on every protocol — HTTP/1.1, HTTP/2 (TLS and h2c), HTTP/3 — is recorded by the server core in `http_server_request_duration_seconds`, the OpenTelemetry [`http.server.request.duration`](https://opentelemetry.io/docs/specs/semconv/http/http-metrics/#metric-httpserverrequestduration) metric under its Prometheus name. No layer or setting is needed.

| Label | Values |
|---|---|
| `http_request_method` | `GET`, `POST`, … (the nine standard methods); anything else `_OTHER` |
| `http_response_status_code` | the status sent |
| `network_protocol_version` | `1.0`, `1.1`, `2`, `3` |
| `url_scheme` | `http` or `https` |

There is no path label, so the number of series stays bounded however many URLs are requested ([`MetricsLayer`](#per-route-metrics-with-metricslayer) has per-path series). Buckets are the convention's advised boundaries: 5 ms, 10 ms, 25 ms, 50 ms, 75 ms, 100 ms, 250 ms, 500 ms, 750 ms, 1 s, 2.5 s, 5 s, 7.5 s, 10 s. The duration runs from when the request is handed to the application until its response is ready to write — the transfer of a streamed body (a large file, a proxied stream, SSE) isn't included.

```promql
# p99 per protocol
histogram_quantile(0.99, sum by (le, network_protocol_version) (rate(http_server_request_duration_seconds_bucket[5m])))

# 5xx ratio
sum(rate(http_server_request_duration_seconds_count{http_response_status_code=~"5.."}[5m]))
  / sum(rate(http_server_request_duration_seconds_count[5m]))
```

### Native histograms

`http_server_request_duration_seconds` also has exponential buckets (schema 3: eight per power of two, each about 9% wider than the last), served as a [native histogram](https://prometheus.io/docs/specs/native_histograms/) when Prometheus scrapes in the protobuf format. Prometheus 3.9 and later need `scrape_native_histograms` set on the job:

```yaml
scrape_configs:
  - job_name: rws
    scrape_native_histograms: true
    static_configs:
      - targets: ["localhost:7878"]
```

Prometheus then stores the native form only (unless `always_scrape_classic_histograms` is set), and queries drop `_bucket` and `le`:

```promql
histogram_quantile(0.99, sum(rate(http_server_request_duration_seconds[5m])))
histogram_count(rate(http_server_request_duration_seconds{http_response_status_code="200"}[5m]))
```

Without `scrape_native_histograms` nothing changes: Prometheus asks for the text format and gets the classic buckets.

## Per-route metrics with `MetricsLayer`

Wrap your application once at startup to enable per-route request counts and latency histograms:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::metrics::MetricsLayer;

let app = App::new().wrap(MetricsLayer);
```

`MetricsLayer` records two metric families per `(method, path)` pair after each request completes.

### `rws_route_requests_total`

A counter labelled by method, path, and HTTP status code.

```
# HELP rws_route_requests_total Total requests handled per route
# TYPE rws_route_requests_total counter
rws_route_requests_total{method="GET",path="/api/users",status="200"} 9401
rws_route_requests_total{method="GET",path="/api/users",status="404"} 12
rws_route_requests_total{method="POST",path="/api/users",status="201"} 530
```

Query strings are stripped from the path label automatically (`/users?page=2` → `/users`).

### `rws_route_duration_seconds`

A cumulative histogram labelled by method and path, with 11 predefined buckets matching the default Prometheus/Go client boundaries:

`0.005`, `0.01`, `0.025`, `0.05`, `0.1`, `0.25`, `0.5`, `1.0`, `2.5`, `5.0`, `10.0` (seconds)

```
# HELP rws_route_duration_seconds Request duration in seconds per route
# TYPE rws_route_duration_seconds histogram
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.005"} 8200
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.01"} 9100
...
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="+Inf"} 9413
rws_route_duration_seconds_sum{method="GET",path="/api/users"} 47.231000000
rws_route_duration_seconds_count{method="GET",path="/api/users"} 9413
```

### Behaviour notes

- A handler that returns `Err` is recorded with status `500`.
- The per-route section is absent from `/metrics` until at least one route has been observed.
- `metrics::record_route(method, path, status, elapsed_secs)` records a request directly, for custom instrumentation without `MetricsLayer`:

```rust
use rust_web_server::metrics;

metrics::record_route("GET", "/jobs/run", 200, 0.042);
```

## Circuit breaker state

`rws_circuit_breaker_state{backend}` is included automatically — no `MetricsLayer` or other opt-in needed — for every backend known to [`circuit_breaker::global()`](/proxy/circuit-breaker/):

```
# HELP rws_circuit_breaker_state Circuit breaker state per backend (0=closed, 1=half_open, 2=open)
# TYPE rws_circuit_breaker_state gauge
rws_circuit_breaker_state{backend="api-1:8080"} 0
rws_circuit_breaker_state{backend="api-2:8080"} 2
```

The gauge value is `0` (Closed/healthy), `1` (HalfOpen/probing), or `2` (Open/unhealthy). Wiring `ReverseProxy::with_circuit_breaker(Arc::new(circuit_breaker::global()))` gets you this metric "for free" alongside the automatic proxy integration, since both read from the same breaker instance. `RedisCircuitBreaker` state doesn't appear here — the minimal hand-rolled RESP client has no `SCAN`/`KEYS` support to enumerate its keys.

## Upstream metrics

Every `[[upstream]]` of a [config-file proxy](/proxy/config-driven/) is measured per backend, with no opt-in:

| Metric | Type | Labels | Description |
|---|---|---|---|
| `rws_upstream_requests_total` | Counter | `upstream`, `backend`, `code` | Requests sent to the backend by response status; `code="error"` when none came back |
| `rws_upstream_request_duration_seconds` | Histogram | `upstream`, `backend` | Time to the backend's response head |
| `rws_upstream_retries_total` | Counter | `upstream` | Requests sent again to another backend (`[upstream.retry]`) |
| `rws_upstream_backend_up` | Gauge | `upstream`, `backend` | `1` while in rotation (passing health checks, breaker not open), else `0` |
| `rws_upstream_circuit_breaker_state` | Gauge | `upstream`, `backend` | `0`=closed, `1`=half_open, `2`=open; only for upstreams with `[upstream.circuit_breaker]` |

```promql
# Backend error ratio
sum by (upstream, backend) (rate(rws_upstream_requests_total{code=~"5..|error"}[5m]))
  / sum by (upstream, backend) (rate(rws_upstream_requests_total[5m]))

# Backends out of rotation
rws_upstream_backend_up == 0
```

## `SERVER_READY` and `/readyz`

The built-in `GET /readyz` controller returns `200 OK` when `SERVER_READY` is `true` and `503 Service Unavailable` when it is `false`. The flag is set to `true` after `Server::setup()` completes and back to `false` when a shutdown signal is received, so Kubernetes stops routing traffic before the pod exits.

```rust
use rust_web_server::metrics::SERVER_READY;
use std::sync::atomic::Ordering;

// Check readiness programmatically.
let ready = SERVER_READY.load(Ordering::Relaxed);
```

## Scraping with Prometheus

```yaml
# prometheus.yml
scrape_configs:
  - job_name: rws
    static_configs:
      - targets: ["localhost:7878"]
    metrics_path: /metrics
```
