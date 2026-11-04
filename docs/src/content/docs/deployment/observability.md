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
{"time":"2026-07-02T12:00:00Z","remote_addr":"203.0.113.9","method":"GET","path":"/docs/","protocol":"HTTP/2.0","status":200,"bytes":14384,"host":"example.com","user_agent":"Mozilla/5.0 …","referer":"https://www.google.com/"}
```

`host`, `user_agent` and `referer` come last, each only when the request
sent the header (since 18.15.0). JSON logs are well-suited for ingestion by Loki, Fluentd, Datadog, and other structured-log collectors.

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
access_log_max_size = "100M"    # rotate past 100 MiB (the default since 18.15.0)
access_log_keep     = 5         # access.log.1 … access.log.5
```

- Lines are appended; the file and its directory must be writable by rws.
- **Rotation by rws** (the default): past `access_log_max_size` (100 MiB
  unless set) the file is renamed `access.log.1` (older ones shift up, the
  oldest past `access_log_keep` is deleted) and a new one is started — at
  most 600 MiB with the defaults. Before 18.15.0 the default was `0`,
  never rotating.
- **Rotation by logrotate:** set `access_log_max_size` to `0` and have
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

### Disk safety

A busy server writes a lot: at 10,000 requests a second an access log
grows by about 170 GB a day. Everything rws writes is bounded:

| What | Bound |
|---|---|
| Access log in a file | rotated at `access_log_max_size` (100 MiB) × `access_log_keep` (5) |
| Disk cache | `[route.middleware.cache] max_size` (256 MiB), oldest out |
| Any of them, whatever the settings | `min_free_disk` (below) |

**`min_free_disk`** (`RWS_CONFIG_MIN_FREE_DISK`, since 18.15.0): before
writing the access-log file or a disk-cache entry, rws checks the free
space of that filesystem (at most every 5 s). Under the floor it stops
writing there — the lines are dropped and counted, the cache serves what it
has — logs one warning, and resumes when space is back. A full disk then
costs log lines, never the server, a database or a mail spool on the same
disk.

```toml
min_free_disk = "2G"     # or "5%"; default: the larger of 1% and 1 GiB; "0" turns it off
```

The space counted is what an unprivileged process may use, so the root
reserve most filesystems keep is already set aside. `/metrics` has
`rws_disk_free_bytes{dir}`, `rws_disk_low{dir}`,
`rws_access_log_dropped_lines_total` and `rws_cache_skipped_writes_total`;
the admin API's `/status` lists each directory, and the admin UI's Overview
warns while one is low.

**Logging to stdout** leaves the bound to what collects it:

- **journald** keeps at most `SystemMaxUse` (default 10% of the
  filesystem, up to 4 GiB) and drops lines past `RateLimitBurst` per
  `RateLimitIntervalSec`. For a busy server set them in
  `/etc/systemd/journald.conf.d/rws.conf`:

  ```ini
  [Journal]
  SystemMaxUse=2G
  RateLimitIntervalSec=0
  ```

  (`RateLimitIntervalSec=0` stops journald dropping rws's lines; the size
  cap still holds.)
- **Docker**: the default `json-file` driver never rotates — set
  `--log-opt max-size=100m --log-opt max-file=5` (or the same in
  `daemon.json`).
- **Kubernetes** rotates container logs at 10 MiB × 5 by default
  (`containerLogMaxSize`, `containerLogMaxFiles`).

If you don't read every line, `log_format = "off"` writes none; metrics and
the admin API keep working.

### Log aggregation in Kubernetes

The server writes all logs to stdout. Kubernetes log aggregation pipelines pick them up automatically:

```sh
# View logs in real time
kubectl logs -f deployment/my-rws-app

# Ship to Loki via Promtail — no application config required; just label the pod
```

### Anonymised addresses and redaction

Since 18.33.0 the access log can leave out what it doesn't need to keep
about people. All of it is off by default, and then every line is byte for
byte what it was; the settings are hot-reloadable.

```toml
access_log_anonymize_ip     = "truncate"            # or "hash"; default "off"
# access_log_ip_hash_secret = "…"                   # the key for "hash"
log_redact_query            = ["token", "api_key"]  # query parameter values → REDACTED
log_redact_headers          = ["x-api-key"]         # header values → REDACTED
```

- **`access_log_anonymize_ip`**: `truncate` writes an IPv4 address as its
  /24 and an IPv6 address as its /48 (`203.0.113.0`, `2001:db8:1::`), as
  Google Analytics and nginx `map` recipes do. `hash` writes 16 hex digits
  of HMAC-SHA256(`access_log_ip_hash_secret`, address) instead: one client
  keeps one pseudonym, so you can still count and follow a client, but the
  address can't be recovered without the key (and an IPv4 address space is
  too small to hash without one — with no secret `hash` truncates). It
  applies to the client address in JSON and combined lines, including the
  one taken from a trusted proxy's `X-Forwarded-For`, to
  [OTLP log records](#otlp-logs) and to the `recent_requests` list.
  [Traffic insights](/deployment/traffic/) keep no addresses either way; the
  [IP blocklist](/features/ip-filter/) and rate limits still see real ones.
- **`log_redact_query`**: the values of these query parameters (any case)
  are written as `REDACTED` in the logged path and `Referer`:
  `/login?user=ann&token=REDACTED`.
- **Headers.** An access-log line carries three request headers: `Host`,
  `User-Agent` and `Referer` (JSON only). `Authorization`,
  `Proxy-Authorization`, `Cookie` and `Set-Cookie` are never written —
  not in the access log, not in `recent_requests`, and shown as `REDACTED`
  in `Log::request_response`'s debugging dump. `log_redact_headers` adds
  names whose values are written as `REDACTED` wherever a header is logged
  (`user-agent`, `referer`, or your own in the dump).

### Syslog

`access_log_syslog` (since 18.33.0) sends every access-log line to a syslog
server too, besides stdout or the [file](#writing-to-a-file):

```toml
log_format                 = "json"                 # what SIEMs parse best
access_log_syslog          = "tcp://siem.internal:6514"   # or udp://host:514, unix:///dev/log
access_log_syslog_facility = "local0"               # default; any facility name or 0–23
```

Each line is one RFC 5424 message, severity `info`, APP-NAME `rws`, MSGID
`access`, the host name from `HOSTNAME` (else `/etc/hostname`):

```
<134>1 2026-11-04T12:00:00Z web-1 rws 4242 access - {"time":"2026-11-04T12:00:00Z","remote_addr":"203.0.113.0",…}
```

Over TCP messages are framed by octet counting (RFC 6587: `LEN SP MSG`),
which rsyslog, syslog-ng, Splunk and Graylog accept; over UDP and Unix
sockets each is one datagram. Nothing is sent on the request path: the
background thread that flushes the access log every 100 ms hands each
batch to the thread `rws-syslog`. While the server is unreachable up to
8 MiB of lines wait, then lines are dropped; a TCP connection is re-made
on the next batch. Read at startup; plain TCP only (put a local relay in
front for TLS).

### OTLP logs

With `OTEL_LOGS_EXPORTER=otlp` (since 18.33.0) every access-log record also
goes to an OpenTelemetry collector as an OTLP/HTTP JSON log record
(`/v1/logs`), next to [metrics](#otlp-metrics-export) and
[traces](#tracing):

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4318   # logs go to /v1/logs
export OTEL_LOGS_EXPORTER=otlp
rws
```

| Variable | Meaning |
|---|---|
| `OTEL_LOGS_EXPORTER` | `otlp` turns log export on (off by default) |
| `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` | Full logs URL, used as is; else the base URL + `/v1/logs`, else `http://localhost:4318/v1/logs` |
| `OTEL_BLRP_SCHEDULE_DELAY` | Milliseconds between exports (default `1000`) |
| `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_EXPORTER_OTLP_LOGS_HEADERS` | Request headers, `key=value,…` |

A record's body is the access-log line (in `log_format`; JSON when that is
`off`, so you can ship only OTLP), with the settings above applied. Its
attributes are `http.request.method`, `url.path`,
`http.response.status_code`, `client.address` and
`network.protocol.version`; its severity INFO, WARN for 4xx and ERROR for
5xx. When the request was traced (the binary traces when an OTLP endpoint
is set), the record carries the server span's `traceId` and `spanId`, so a
backend links each log line to its trace. Records are formatted into a
buffer per thread and sent by the thread `rws-otlp-logs`; a thread with more
than 1 MiB unsent drops records while the collector is down. The rest goes
out on SIGTERM. From Rust: `otel::logs::start(LogsExportConfig { .. })` or
`otel::logs::start_from_env()`.


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
