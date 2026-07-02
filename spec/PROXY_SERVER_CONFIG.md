# rws as a Proxy Server — Config-Driven Design

This document specifies the configuration schema for evolving `rws` from a static file server into a config-driven proxy and web server platform comparable to nginx, Caddy, Traefik, and HAProxy. No Rust code should be required to configure routing, proxying, TLS, or middleware.

---

## Design principles

1. **Config-driven** — every capability expressible in `rws.config.toml` (or env vars). The binary reads config at startup and wires the middleware stack automatically.
2. **Backward compatible** — existing `rws.config.toml` fields keep working unchanged.
3. **TOML-native** — use `[[route]]` array-of-tables for location blocks, `[[upstream]]` for backend pools. No custom DSL.
4. **Layered middleware per route** — each route can declare its own middleware stack (auth, rate limit, rewrite, cache) without affecting others.
5. **Hot reload** — SIGHUP or `POST /admin/config/reload` re-applies the full config without restart, including route table changes.

---

## Full annotated `rws.config.toml`

```toml
# ── Server ────────────────────────────────────────────────────────────────────

ip           = "0.0.0.0"
port         = 443
thread_count = 512
request-allocation-size-in-bytes = 65536

# Optional: redirect plain HTTP on this port to HTTPS.
http_redirect_port = 80

# ── TLS ───────────────────────────────────────────────────────────────────────

tls_cert_file      = "/etc/rws/certs/default.crt"
tls_key_file       = "/etc/rws/certs/default.key"

# mTLS: require clients to present a certificate signed by this CA.
# tls_client_ca_file = "/etc/rws/certs/ca.pem"

# ── Virtual hosts (SNI-based cert selection) ──────────────────────────────────

[[virtual_host]]
domain    = "api.example.com"
cert_file = "/etc/rws/certs/api.crt"
key_file  = "/etc/rws/certs/api.key"

[[virtual_host]]
domain    = "www.example.com"
cert_file = "/etc/rws/certs/www.crt"
key_file  = "/etc/rws/certs/www.key"

# ── Automatic TLS via ACME (Let's Encrypt) ────────────────────────────────────
# Requires `--features acme`. Provisions and renews certs automatically.
# [acme]
# domains = ["api.example.com", "www.example.com"]
# email   = "ops@example.com"

# ── Upstream pools ────────────────────────────────────────────────────────────
# Named groups of backends used by proxy routes.
# strategy: "round_robin" (default) | "least_conn" | "ip_hash" | "random"

[[upstream]]
name     = "api-backends"
strategy = "round_robin"
backends = ["10.0.0.10:8080", "10.0.0.11:8080", "10.0.0.12:8080"]

  [upstream.health_check]
  path             = "/healthz"      # HTTP GET path to probe
  interval_secs    = 10
  timeout_ms       = 2000
  healthy_threshold   = 2            # consecutive successes to mark up
  unhealthy_threshold = 3            # consecutive failures to mark down

[[upstream]]
name     = "grpc-service"
strategy = "round_robin"
backends = ["h2://grpc1:50051", "h2://grpc2:50051"]

[[upstream]]
name     = "static-origin"
strategy = "round_robin"
backends = ["https://storage.example.com"]

# ── Routes ────────────────────────────────────────────────────────────────────
# Evaluated top-to-bottom; first match wins.
# Each route has a [match] block and an [action] block.
# Optional [middleware] block declares per-route middleware layers.

# ── Route 1: API reverse proxy ────────────────────────────────────────────────
[[route]]
name = "api-proxy"

  [route.match]
  host   = "api.example.com"        # SNI / Host header; omit to match any host
  path   = "/v1/*"                  # prefix match; use exact = "/v1/ping" for exact

  [route.action]
  type     = "proxy"
  upstream = "api-backends"         # references [[upstream]] name above

    [route.action.proxy]
    connect_timeout_ms = 1000
    read_timeout_ms    = 10000
    strip_path_prefix  = "/v1"      # strip before forwarding
    add_path_prefix    = "/api/v1"  # prepend after strip
    upstream_scheme    = "http"     # "http" | "https" | "h2"

  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  auth       = { type = "jwt", secret_env = "JWT_SECRET" }

    [[route.middleware.rewrite.request]]
    type  = "header_set"
    name  = "X-Forwarded-Host"
    value = "api.example.com"

    [[route.middleware.rewrite.response]]
    type   = "header_set"
    name   = "Cache-Control"
    value  = "no-store"

# ── Route 2: gRPC proxy ───────────────────────────────────────────────────────
[[route]]
name = "grpc-proxy"

  [route.match]
  host         = "api.example.com"
  content_type = "application/grpc*"   # match on Content-Type prefix

  [route.action]
  type     = "grpc"
  upstream = "grpc-service"

    [route.action.proxy]
    connect_timeout_ms = 1000
    read_timeout_ms    = 30000

# ── Route 3: static files ─────────────────────────────────────────────────────
[[route]]
name = "static-site"

  [route.match]
  host = "www.example.com"
  path = "/*"

  [route.action]
  type = "static"
  root = "/var/www/html"             # serve files from this directory
  index = ["index.html", "index.htm"]
  directory_listing = false

  [route.middleware]
  cache = { ttl_secs = 3600, vary_by = ["Accept-Encoding"] }

    [[route.middleware.rewrite.response]]
    type  = "header_set"
    name  = "Cache-Control"
    value = "public, max-age=3600"

# ── Route 4: redirect ─────────────────────────────────────────────────────────
[[route]]
name = "www-redirect"

  [route.match]
  host = "example.com"
  path = "/*"

  [route.action]
  type     = "redirect"
  location = "https://www.example.com$path"
  status   = 301

# ── Route 5: basic auth protected area ───────────────────────────────────────
[[route]]
name = "admin-area"

  [route.match]
  host = "www.example.com"
  path = "/admin/*"

  [route.action]
  type = "static"
  root = "/var/www/admin"

  [route.middleware]
  auth = { type = "basic", users_file = "/etc/rws/htpasswd" }

# ── Route 6: MCP endpoint ─────────────────────────────────────────────────────
[[route]]
name = "mcp"

  [route.match]
  path   = "/mcp"
  method = "POST"

  [route.action]
  type = "mcp"                       # built-in MCP server

  [route.middleware]
  auth = { type = "bearer", token_env = "MCP_TOKEN" }

# ── Route 7: catch-all 404 ────────────────────────────────────────────────────
[[route]]
name = "not-found"

  [route.match]
  path = "/*"

  [route.action]
  type   = "respond"
  status = 404
  body   = "Not Found"

# ── TCP proxy (L4, runs a separate listener) ──────────────────────────────────
[[tcp_proxy]]
name              = "postgres"
listen            = "0.0.0.0:5432"
backends          = ["db1:5432", "db2:5432"]
strategy          = "round_robin"
connect_timeout_ms = 500

[[tcp_proxy]]
name     = "redis"
listen   = "0.0.0.0:6379"
backends = ["cache1:6379"]

# ── UDP proxy ─────────────────────────────────────────────────────────────────
[[udp_proxy]]
name             = "dns"
listen           = "0.0.0.0:53"
backends         = ["8.8.8.8:53", "1.1.1.1:53"]
strategy         = "round_robin"
reply_timeout_ms = 2000
buffer_size      = 8192

# ── WebSocket proxy ───────────────────────────────────────────────────────────
# WsProxy runs a separate listener (raw upgrade + byte relay).
[[ws_proxy]]
name              = "chat"
listen            = "0.0.0.0:9000"
backends          = ["ws-backend1:8080", "ws-backend2:8080"]
connect_timeout_ms = 500
read_timeout_ms   = 30000

# ── Global middleware defaults ────────────────────────────────────────────────
# Applied to all [[route]] blocks unless the route overrides them.
[middleware]
rate_limit = { max_requests = 1000, window_secs = 60 }
log_format = "json"                  # "combined" | "json"

  [[middleware.rewrite.response]]
  type  = "header_remove"
  name  = "X-Powered-By"

  [[middleware.rewrite.response]]
  type  = "header_set"
  name  = "X-Content-Type-Options"
  value = "nosniff"

# ── CORS ──────────────────────────────────────────────────────────────────────
[cors]
allow_all         = false
allow_origins     = ["https://www.example.com"]
allow_methods     = ["GET", "POST", "PUT", "PATCH", "DELETE"]
allow_headers     = ["content-type", "authorization"]
allow_credentials = true
expose_headers    = ["content-type"]
max_age           = "86400"
```

---

## Route matching rules

Matches are evaluated in the order listed in the config file. First match wins. All match fields within a single `[route.match]` block are ANDed together.

| Field | Type | Semantics |
|---|---|---|
| `host` | string | Exact SNI / `Host` header match. Omit to match any host. |
| `path` | string | Prefix match when ending with `/*`; exact match otherwise. |
| `method` | string | HTTP method (`GET`, `POST`, …). Omit to match any method. |
| `content_type` | string | `Content-Type` prefix match (e.g. `application/grpc*`). |

---

## Action types

| `type` | Behaviour |
|---|---|
| `proxy` | Forward to an `[[upstream]]` pool over HTTP/1.1 |
| `grpc` | Forward to an `[[upstream]]` pool over HTTP/2 (`Content-Type: application/grpc*` enforced) |
| `static` | Serve files from a local directory |
| `redirect` | Return a 301/302 with a `Location` header; `$path` is substituted |
| `respond` | Return a fixed status + body (useful for maintenance, 404 catch-all) |
| `mcp` | Activate the built-in MCP Streamable HTTP server |

---

## Per-route middleware

All middleware keys under `[route.middleware]` are optional. Absent keys inherit global defaults.

| Key | Type | Behaviour |
|---|---|---|
| `rate_limit` | `{ max_requests, window_secs }` | Per-client-IP sliding-window; returns 429 |
| `cache` | `{ ttl_secs, vary_by }` | In-memory GET cache |
| `auth.type = "basic"` | `{ users_file }` | HTTP Basic auth against an htpasswd file |
| `auth.type = "jwt"` | `{ secret_env }` | HS256 JWT via `Authorization: Bearer` |
| `auth.type = "bearer"` | `{ token_env }` | Static bearer token comparison |
| `rewrite.request[]` | array of rules | Transform the request before forwarding |
| `rewrite.response[]` | array of rules | Transform the response before returning |
| `ip_filter.allow` | array of CIDR strings | Allowlist; 403 for non-matching IPs |
| `ip_filter.deny` | array of CIDR strings | Denylist; 403 for matching IPs |

---

## Rewrite rule types

Rules in `rewrite.request[]` and `rewrite.response[]` arrays:

| `type` | Fields | Effect |
|---|---|---|
| `header_set` | `name`, `value` | Add or replace a header |
| `header_remove` | `name` | Remove a header |
| `uri_set` | `value` | Replace the entire request URI |
| `uri_strip_prefix` | `prefix` | Remove a path prefix |
| `uri_add_prefix` | `prefix` | Prepend to the URI |
| `status_set` | `code`, `reason` | Override response status (response only) |
| `body_replace` | `from`, `to` | Byte-level find-and-replace in response body |

---

## Upstream health checks

When `[upstream.health_check]` is present, a background task probes each backend at `interval_secs`. Backends that fail `unhealthy_threshold` consecutive probes are removed from the rotation. They are re-added after `healthy_threshold` consecutive successes. The proxy always returns `502` if no healthy backend is available.

Health check state is visible via `GET /metrics` (Prometheus) and the `server_metrics` MCP tool.

---

## Load balancing strategies

| `strategy` | Behaviour |
|---|---|
| `round_robin` | Distribute requests in order (default) |
| `least_conn` | Send to the backend with the fewest active connections |
| `ip_hash` | Hash client IP to a backend for session affinity |
| `random` | Pick a backend at random |

---

## Implementation status (v17.36.0)

### ✅ Phase 1 — Config schema + route table
- `src/proxy_config/mod.rs` — all config types: `ProxyConfig`, `UpstreamConfig`, `RouteConfig`, `ActionConfig`, `MiddlewareConfig`, `HealthCheckConfig`, `RewriteRuleConfig`, etc.
- `src/proxy_config/parser.rs` — hand-rolled TOML parser (`SectionMap`); handles `[[arrays]]`, `[sub-tables]`, inline tables, and arrays of values
- `src/proxy_config/builder.rs` — `build_from_file()` / `build()` wires parsed config into a live `ConfigDrivenApp`
- `ConfigDrivenApp` — first-match router over `Arc<Vec<CompiledRoute>>`; implements `Application + Clone`
- `ProxyConfig::is_proxy_mode()` — detects `[[route]]` / `[[upstream]]` in `rws.config.toml`; `main()` checks this at startup for all three feature targets (http1 / http2 / http3)

### ✅ Phase 2 — Backend health checks
- `src/proxy_config/health.rs` — `start_health_checker()` spawns a daemon thread per upstream; tracks per-backend consecutive successes/failures; updates `Arc<RwLock<Vec<String>>>` live-backend list
- `DynamicProxy` reads from the same `Arc<RwLock<…>>` for zero-copy health-aware round-robin

### ⏳ Phase 3 — Load balancing strategies
- `round_robin` implemented (default); `least_conn`, `ip_hash`, `random` are parsed but fall back to round-robin

### ⏳ Phase 4 — Upstream TLS
- `https://` scheme stripped in health-check parser but TLS client handshake not yet implemented for proxy forwarding; connections use plain HTTP/1.1

### ✅ Phase 5 — Config-driven TCP/UDP/WS proxies
- `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]` sections spawn dedicated threads via `TcpProxy::bind()`, `UdpProxy::bind()`, `WsProxy::bind()`

### ⏳ Phase 6 — Per-route auth from config
- `bearer` auth via `BearerAuthMiddleware` ✅
- `jwt` auth is a no-op placeholder (requires `auth` feature future wiring) ⏳
- `basic` auth with htpasswd file not yet implemented ⏳

### ⏳ Phase 7 — Static site action
- `type = "static"` falls through to built-in `App` (which serves from the working directory); `root` / `directory_listing` config fields not yet honored

---

## Original implementation plan

The following captures the original design intent for reference:

### Phase 1 — Config schema + route table (highest value)
1. Extend `src/entry_point/mod.rs` to parse `[[upstream]]`, `[[route]]`, `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]` from `rws.config.toml`.
2. Add `RouteConfig`, `UpstreamConfig`, `ActionConfig`, `MiddlewareConfig` structs in a new `src/config/mod.rs`.
3. Build a `src/config/builder.rs` that turns parsed config into a `WithMiddleware<App>` stack at startup — this is the core of the shift.
4. `main.rs` calls `config::build_app_from_config()` instead of the hardcoded `build_app()`.

### Phase 2 — Backend health checks
5. Add `src/health_check/mod.rs` — background thread per upstream pool that probes backends and maintains an `Arc<RwLock<Vec<String>>>` of live backends.
6. `ReverseProxy` reads live backends from the health checker instead of a static list.

### Phase 3 — Load balancing strategies
7. Extend `src/proxy/mod.rs` with `LeastConn` (atomic active-connection counter per backend) and `IpHash` (FNV hash of client IP mod backends) strategies.

### Phase 4 — Upstream TLS
8. Add `https://` backend support to `ReverseProxy` via `rustls` client config.
9. Reuse `load_certs()` from `src/tls/mod.rs`; add `RWS_CONFIG_UPSTREAM_CA_FILE` for custom CA trust.

### Phase 5 — Config-driven TCP/UDP/WS proxies
10. On startup, spawn one thread per `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]` block calling `TcpProxy::bind()`, `UdpProxy::bind()`, `WsProxy::bind()` respectively.
11. SIGHUP restarts proxy threads whose config changed.

### Phase 6 — Per-route auth from config
12. Extend `BasicAuthLayer` to load users from an htpasswd file path (not just a closure).
13. `config::builder` wraps matching routes with the appropriate auth layer based on `[route.middleware.auth]`.

### Phase 7 — Static site action
14. The `static` action type is already handled by `StaticResourceController`; expose `root` and `directory_listing` as config fields so the route can override the working directory.

---

## What stays code-driven (library API)

Users who import `rust-web-server` as a library crate keep the existing API — `ReverseProxy::new(...)`, `.wrap(RewriteLayer::new()...)`, etc. The config-driven path is an additional layer on top, not a replacement. The library remains useful for embedding rws in custom Rust applications.
