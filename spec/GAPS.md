# Gaps Compared to nginx, Traefik, Caddy, HAProxy, and Envoy

This document tracks what is missing for `rws` to be competitive with production-grade web servers and reverse proxies.

---

## Critical gaps (core server functionality)

### Reverse proxy ✅ Done (v17.20.0)

`src/proxy/ReverseProxy` forwards requests to HTTP backends with round-robin load balancing.  `path_prefix` routing lets the proxy coexist with local handlers.  Returns `502 Bad Gateway` when all backends fail.

Remaining: TLS upstreams, weighted distribution, health-check probes, circuit breaker.

### Virtual hosting / SNI routing

One server instance = one site. nginx `server_name` blocks, Traefik's `Host` rules, and Caddy's site blocks let a single process serve multiple domains on one port. `rws` has no concept of virtual hosts.

### Automatic TLS (ACME) ✅ Done — v17.25.0

`AcmeManager` (feature flag `acme`) auto-provisions and renews Let's Encrypt certificates. Set `RWS_CONFIG_ACME_DOMAINS` and `RWS_CONFIG_ACME_EMAIL` at startup; a background task renews before expiry and triggers a TLS hot-reload via SIGHUP. OCSP stapling is not yet implemented.

---

## Important gaps

### Rate limiting ✅ Done

`RateLimiter` and `RateLimitLayer` provide per-IP sliding-window rate limiting configurable via env vars.

Remaining: per-route rate caps, least-connections upstream selection.

### Request / response rewriting

No URL rewriting or body transformation. nginx `rewrite`, `proxy_set_header`, and `sub_filter` are heavily used in production deployments.

`X-Forwarded-For` and `Via` are injected automatically by `ReverseProxy`.

### Authentication middleware ✅ Done

`BasicAuthLayer` (HTTP Basic) and `JwtLayer` (HS256 JWT) ship in the `auth` feature.  `IpFilter::allow` / `IpFilter::deny` handle IP allowlist/denylist.

Remaining: forward-auth delegation to an external service (Traefik's `ForwardAuth`).

### Response caching ✅ Done (v17.22.0)

`CacheLayer` middleware (`src/cache/`) stores successful `GET` responses in memory and serves subsequent identical requests without calling the handler. Supports TTL, vary-by-header for content negotiation, capacity-bounded eviction (oldest-first), and `Cache-Control: no-store/private` opt-out. `Age` header injected on hits.

Remaining: shared cache across processes (Redis, Memcached), `stale-while-revalidate`, CDN-tier caching (Surrogate-Key, purge API).

### Hot config reload ✅ Done (v17.21.0)

Send `SIGHUP` (or `POST /admin/config/reload`) to re-apply CORS rules, rate-limit thresholds, log format, and request allocation size without restarting. `RateLimiter` limits update live via `AtomicU32`/`AtomicU64`. `config_reload::current()` exposes a typed `ConfigSnapshot` anywhere in the handler stack.

Remaining: TLS cert rotation (requires rebuilding the acceptor), port/thread-count changes (require restart).

---

## Observability gaps

### Per-route metrics ✅ Done (v17.23.0)

`MetricsLayer` middleware (`src/metrics/`) records `rws_route_requests_total{method,path,status}` counters and `rws_route_duration_seconds{method,path}` histograms (11 standard Prometheus buckets) per endpoint. `GET /metrics` now emits per-route data when `MetricsLayer` is in the stack. Query strings are stripped before keying to avoid cardinality explosion.

Remaining: OpenTelemetry export, per-route error-rate alerting, cardinality limits.

### Distributed tracing ✅ Done (v17.24.0)

`OtelLayer` middleware adds W3C Trace Context propagation and OTLP HTTP export
(Jaeger ≥ 1.35, Grafana Tempo, OpenTelemetry Collector). Zero new Cargo
dependencies.

Remaining: B3 propagation, multi-span (child spans within handlers), baggage
propagation, automatic instrumentation of DB/HTTP calls.

### Access log rotation

No built-in log rotation or external log shipping (syslog, journald). Relies on the OS or a sidecar container.

---

## Protocol gaps

| Protocol | nginx | Traefik | Caddy | rws |
|---|---|---|---|---|
| HTTP/1.1 reverse proxy | ✅ | ✅ | ✅ | ✅ |
| HTTP/2 reverse proxy | ✅ | ✅ | ✅ | ❌ |
| TCP proxy (L4) | ✅ | ✅ | ✅ | ❌ |
| UDP proxy | ✅ | ✅ | ❌ | ❌ |
| WebSocket (server) | ✅ | ✅ | ✅ | ✅ |
| WebSocket proxy | ✅ | ✅ | ✅ | ❌ |
| gRPC proxy | ✅ | ✅ | ✅ | ❌ |
| Server-Sent Events | ✅ | ✅ | ✅ | ✅ |
| mTLS (client certificates) | ✅ | ✅ | ✅ | ❌ |

---

## Kubernetes / cloud-native gaps

### Ingress controller

Traefik and nginx both run as Kubernetes Ingress controllers, reading `Ingress` / `IngressRoute` objects from the API server and dynamically routing traffic. `rws` has no service discovery or dynamic configuration model.

### Service discovery

No integration with Consul, etcd, Docker labels, or the Kubernetes API.

### Traffic splitting / canary routing

No weighted routing between versions (e.g. 10% to v2, 90% to v1).

### Circuit breaker / retry

No upstream health-based circuit breaking or automatic retries on upstream 5xx responses.

---

## Implementation priority

| Priority | Gap | Status |
|---|---|---|
| 1 | Reverse proxy + load balancing | ✅ Done (v17.20.0) |
| 2 | Virtual hosting / SNI routing | Pending |
| 3 | Automatic TLS (ACME / Let's Encrypt) | ✅ Done (v17.25.0) |
| 4 | Rate limiting | ✅ Done |
| 5 | Request / response rewriting | Partial (proxy injects `X-Forwarded-For`, `Via`) |
| 6 | Authentication middleware | ✅ Done (Basic, JWT, IP filter) |
| 7 | Response caching | ✅ Done (v17.22.0) |
| 8 | WebSocket support | ✅ Done (v17.8.0) |
| 9 | Hot config reload | ✅ Done (v17.21.0) |
| 10 | Per-route metrics | ✅ Done (v17.23.0) |
| 11 | Distributed tracing (OpenTelemetry) | ✅ Done (v17.24.0) |
