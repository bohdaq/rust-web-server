# Gaps Compared to nginx, Traefik, Caddy, HAProxy, and Envoy

This document tracks what is missing for `rws` to be competitive with production-grade web servers and reverse proxies.

---

## Critical gaps (core server functionality)

### Reverse proxy

The biggest gap. `rws` can only serve its own content — it cannot forward requests to upstream backends. nginx's `proxy_pass`, Traefik's services, and Caddy's `reverse_proxy` directive are their primary use cases. Without this, `rws` cannot sit in front of application servers.

### Load balancing

No round-robin, least-connections, IP-hash, or weighted distribution across multiple upstreams. Follows directly from the absence of a reverse proxy.

### Virtual hosting / SNI routing

One server instance = one site. nginx `server_name` blocks, Traefik's `Host` rules, and Caddy's site blocks let a single process serve multiple domains on one port. `rws` has no concept of virtual hosts.

### Automatic TLS (ACME)

Caddy auto-provisions and renews Let's Encrypt certificates with zero config. `rws` requires manually providing cert files and has no ACME client, renewal scheduling, or OCSP stapling.

---

## Important gaps

### Rate limiting

No per-IP or per-route request rate caps. nginx `limit_req`, Traefik's `RateLimit` middleware, and HAProxy's `stick-table` are standard production features.

### Request / response rewriting

No URL rewriting, header injection/removal, or body transformation. nginx `rewrite`, `proxy_set_header`, and `sub_filter` are heavily used in production deployments.

### Authentication middleware

No basic auth, no JWT validation, no IP allowlist/denylist, no forward-auth integration (Traefik's `ForwardAuth` delegates auth decisions to an external service).

### Response caching

No HTTP cache layer. nginx `proxy_cache`, Varnish, and Caddy's cache module let the server store and replay upstream responses. Without this `rws` cannot act as a CDN edge or cache accelerator.

### WebSocket support

No HTTP Upgrade path for WebSocket connections, either as a server or a proxy.

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
| TCP proxy (L4) | ✅ | ✅ | ✅ | ❌ |
| UDP proxy | ✅ | ✅ | ❌ | ❌ |
| WebSocket | ✅ | ✅ | ✅ | ❌ |
| gRPC proxy | ✅ | ✅ | ✅ | ❌ |
| Server-Sent Events | ✅ | ✅ | ✅ | ❌ |
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

| Priority | Gap |
|---|---|
| 1 | Reverse proxy + load balancing |
| 2 | Virtual hosting / SNI routing |
| 3 | Automatic TLS (ACME / Let's Encrypt) |
| 4 | Rate limiting |
| 5 | Request / response rewriting |
| 6 | Authentication middleware |
| 7 | Response caching |
| 8 | WebSocket support |
| 9 | Hot config reload |
| 10 | Per-route metrics + distributed tracing |
