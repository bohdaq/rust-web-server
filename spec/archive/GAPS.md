# Gaps Compared to nginx, Traefik, Caddy, HAProxy, and Envoy

This document tracks what is missing for `rws` to be competitive with production-grade web servers and reverse proxies.

---

## Critical gaps (core server functionality)

### Reverse proxy ✅ Done (v17.20.0)

`src/proxy/ReverseProxy` forwards requests to HTTP backends with round-robin load balancing.  `path_prefix` routing lets the proxy coexist with local handlers.  Returns `502 Bad Gateway` when all backends fail.

Remaining: TLS upstreams, weighted distribution, health-check probes, circuit breaker.

### Virtual hosting / SNI routing ✅ Done (v17.29.0)

`SniCertResolver` (`src/tls/mod.rs`) selects the correct TLS certificate at handshake time based on the SNI hostname sent in the client `ClientHello`. `create_tls_acceptor_from_vhosts()` and `create_quinn_server_config_from_vhosts()` replace the previous single-cert path for HTTP/2 and HTTP/3 respectively. Add one `[[virtual_host]]` block per domain in `rws.config.toml` (or set `RWS_CONFIG_VIRTUAL_HOST_{N}_DOMAIN/CERT_FILE/KEY_FILE` env vars). The negotiated hostname is exposed as `ConnectionInfo::sni_hostname: Option<String>` in every handler. `Router::with_host(name)` restricts a router's routes to one virtual host; the `Host` request header is used as fallback for plain-HTTP connections. SIGHUP hot-reloads all virtual host certs alongside the default cert.

Remaining: wildcard domain matching (e.g. `*.example.com`), automatic per-vhost ACME provisioning, weighted traffic splitting across virtual hosts.

### Automatic TLS (ACME) ✅ Done — v17.25.0

`AcmeManager` (feature flag `acme`) auto-provisions and renews Let's Encrypt certificates. Set `RWS_CONFIG_ACME_DOMAINS` and `RWS_CONFIG_ACME_EMAIL` at startup; a background task renews before expiry and triggers a TLS hot-reload via SIGHUP. OCSP stapling is not yet implemented.

---

## Important gaps

### Rate limiting ✅ Done

`RateLimiter` and `RateLimitLayer` provide per-IP sliding-window rate limiting configurable via env vars.

Remaining: per-route rate caps, least-connections upstream selection.

### Request / response rewriting ✅ Done (v17.30.0)

`RewriteLayer` (`src/rewrite/mod.rs`) is a composable `Middleware` that transforms requests before they reach handlers and responses before they leave the server. Builder API: `.request_header_set(name, value)`, `.request_header_remove(name)`, `.request_uri_set(uri)`, `.request_uri_strip_prefix(prefix)`, `.request_uri_add_prefix(prefix)`, `.response_header_set(name, value)`, `.response_header_remove(name)`, `.response_status(code, reason)`, `.response_body_replace(from, to)`. The incoming request is cloned; the original is never mutated.

`X-Forwarded-For` and `Via` are injected automatically by `ReverseProxy`.

Remaining: regex URI rewriting, conditional rewrites (match on header/status), per-route rewrite rule tables.

### Authentication middleware ✅ Done

`BasicAuthLayer` (HTTP Basic) and `JwtLayer` (HS256 JWT) ship in the `auth` feature.  `IpFilter::allow` / `IpFilter::deny` handle IP allowlist/denylist.

Remaining: forward-auth delegation to an external service (Traefik's `ForwardAuth`).

### Response caching ✅ Done (v17.22.0)

`CacheLayer` middleware (`src/cache/`) stores successful `GET` responses in memory and serves subsequent identical requests without calling the handler. Supports TTL, vary-by-header for content negotiation, capacity-bounded eviction (oldest-first), and `Cache-Control: no-store/private` opt-out. `Age` header injected on hits.

Remaining: shared cache across processes (Redis, Memcached), `stale-while-revalidate`, CDN-tier caching (Surrogate-Key, purge API).

### Hot config reload ✅ Done (v17.21.0)

Send `SIGHUP` (or `POST /admin/config/reload`) to re-apply CORS rules, rate-limit thresholds, log format, and request allocation size without restarting. `RateLimiter` limits update live via `AtomicU32`/`AtomicU64`. `config_reload::current()` exposes a typed `ConfigSnapshot` anywhere in the handler stack. TLS cert rotation is also handled by SIGHUP — the acceptor is rebuilt in-place with all virtual host certs (v17.29.0).

Remaining: port/thread-count changes (require restart).

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
| HTTP/2 reverse proxy | ✅ | ✅ | ✅ | ✅ (`H2ReverseProxy`, `http2` feature) |
| TCP proxy (L4) | ✅ | ✅ | ✅ | ✅ (`TcpProxy`) |
| UDP proxy | ✅ | ✅ | ❌ | ✅ (`UdpProxy`, request-reply) |
| WebSocket (server) | ✅ | ✅ | ✅ | ✅ |
| WebSocket proxy | ✅ | ✅ | ✅ | ✅ (`WsProxy`, standalone listener) |
| gRPC proxy | ✅ | ✅ | ✅ | ✅ (`GrpcProxy`, `http2` feature; trailers pending) |
| Server-Sent Events | ✅ | ✅ | ✅ | ✅ |
| mTLS (client certificates) | ✅ | ✅ | ✅ | ✅ (`RWS_CONFIG_TLS_CLIENT_CA_FILE`) |

---

## Kubernetes / cloud-native gaps

### Ingress controller ✅ Done (v17.32.0)

`KubernetesIngressWatcher` (`src/ingress/mod.rs`) polls `/apis/networking.k8s.io/v1/ingresses` on the K8s API server, parses Ingress rules, and maintains a live route table in an `Arc<RwLock<Vec<IngressRule>>>`. `IngressRouter` implements `Application` and routes incoming requests to `{service}.{namespace}.svc.cluster.local:{port}` via HTTP/1.1. Configure via `RWS_K8S_API_SERVER`, `RWS_K8S_TOKEN`, `RWS_K8S_NAMESPACE` or call `KubernetesIngressWatcher::from_service_account()`. Background thread polls at a configurable interval.

Remaining: TLS to `kubernetes.default.svc` (requires rustls client config); watch API (`?watch=true`) instead of polling; support for `pathType: Exact`; Ingress class filtering.

### Service discovery ✅ Done (v17.32.0)

`BackendPool` (`src/service_discovery/mod.rs`) maintains a live list of backends updated by a background thread. Four discovery sources: `Static` (fixed list), `EnvPrefix` (scan `{PREFIX}_0`, `{PREFIX}_1`, … env vars), `File` (one `host:port` per line, polled), `Dns` (A-record lookup via `ToSocketAddrs`). All clones share the same `Arc<RwLock<Vec<String>>>`. Call `.start()` once at startup to launch the background poller.

Remaining: Consul HTTP API, etcd watch, Docker label discovery, SRV record support, weighted DNS.

### Traffic splitting / canary routing ✅ Done (v17.32.0)

`CanaryLayer` (`src/canary/mod.rs`) is a `Middleware` that distributes requests across backends proportionally to their weights. Backends are expanded into a rotation vec (`weight` copies each) and selected via an `AtomicUsize` counter — deterministic, lock-free, zero-dep. `WeightedBackend::new(url, weight)` with `weight = 0` removes a backend from rotation without removing its config entry.

Remaining: live weight updates without restart; smooth weighted round-robin for finer-grained distribution; integration with `BackendPool` for dynamic backend lists.

### Circuit breaker / retry ✅ Done (v17.32.0)

`CircuitBreaker` (`src/circuit_breaker/mod.rs`) is a per-backend state machine: **Closed** counts failures → **Open** blocks requests until recovery window elapses → **HalfOpen** tests with one request → back to **Closed** on success, or **Open** on failure. `global()` returns a process-wide `OnceLock<Mutex<CircuitBreaker>>` singleton (threshold=5, recovery=30s). `RetryLayer` middleware retries on configurable status codes (default: 502, 503, 504) up to `max_retries` times.

Remaining: integration with `ReverseProxy` for automatic per-backend open/close; half-open concurrent request limit; metrics exposure (`rws_circuit_breaker_state{backend}`).

---

## Implementation priority

| Priority | Gap | Status |
|---|---|---|
| 1 | Reverse proxy + load balancing | ✅ Done (v17.20.0) |
| 2 | Virtual hosting / SNI routing | ✅ Done (v17.29.0) |
| 3 | Automatic TLS (ACME / Let's Encrypt) | ✅ Done (v17.25.0) |
| 4 | Rate limiting | ✅ Done |
| 5 | Request / response rewriting | ✅ Done (v17.30.0) |
| 6 | Authentication middleware | ✅ Done (Basic, JWT, IP filter) |
| 7 | Response caching | ✅ Done (v17.22.0) |
| 8 | WebSocket support | ✅ Done (v17.8.0) |
| 9 | Hot config reload | ✅ Done (v17.21.0) |
| 10 | Per-route metrics | ✅ Done (v17.23.0) |
| 11 | Distributed tracing (OpenTelemetry) | ✅ Done (v17.24.0) |
