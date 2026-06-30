# Gaps Compared to nginx, Traefik, Caddy, HAProxy, and Envoy

This document tracks what is missing for `rws` to be competitive with production-grade web servers and reverse proxies.

---

## Critical gaps (core server functionality)

### Reverse proxy ✅ Done (v17.20.0)

`src/proxy/ReverseProxy` forwards requests to HTTP backends with round-robin load balancing.  `path_prefix` routing lets the proxy coexist with local handlers.  Returns `502 Bad Gateway` when all backends fail.

Remaining: TLS upstreams, weighted distribution, health-check probes, circuit breaker.

### Virtual hosting / SNI routing

One server instance = one site. nginx `server_name` blocks, Traefik's `Host` rules, and Caddy's site blocks let a single process serve multiple domains on one port. `rws` has no concept of virtual hosts.

### Automatic TLS (ACME)

Caddy auto-provisions and renews Let's Encrypt certificates with zero config. `rws` requires manually providing cert files and has no ACME client, renewal scheduling, or OCSP stapling.

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

### Response caching

No HTTP cache layer. nginx `proxy_cache`, Varnish, and Caddy's cache module let the server store and replay upstream responses. Without this `rws` cannot act as a CDN edge or cache accelerator.

### Hot config reload

nginx `nginx -s reload`, Traefik's dynamic provider model, and Caddy's admin API allow config changes without dropping connections. `rws` requires a full process restart.

---

## Observability gaps

### Per-route metrics

Only global counters exist (`rws_requests_total`, `rws_errors_total`, `rws_active_connections`). nginx's `$request_time`, Traefik's per-router metrics, and Envoy's per-cluster stats give per-endpoint latency, error rates, and throughput.

### Distributed tracing

No OpenTelemetry, Jaeger, or Zipkin integration. Envoy's built-in tracing is a major differentiator for service mesh deployments.

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
| 3 | Automatic TLS (ACME / Let's Encrypt) | Pending |
| 4 | Rate limiting | ✅ Done |
| 5 | Request / response rewriting | Partial (proxy injects `X-Forwarded-For`, `Via`) |
| 6 | Authentication middleware | ✅ Done (Basic, JWT, IP filter) |
| 7 | Response caching | Pending |
| 8 | WebSocket support | ✅ Done (v17.8.0) |
| 9 | Hot config reload | Pending |
| 10 | Per-route metrics + distributed tracing | Pending |
