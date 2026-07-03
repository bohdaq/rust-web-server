[Read Me](README.md) > Ideas

# Ideas

Forward-looking ideas for evolving `rust-web-server`. All items from the original roadmap ([FRAMEWORK_ROADMAP.md](FRAMEWORK_ROADMAP.md)) are now complete. This file captures what comes next.

---

## Status: foundation is done

Every item from the original 2023–2024 ideas list is shipped:

| Original idea | Shipped |
|---|---|
| Merge duplicate dispatch | v17.6.0 |
| `ConnectionInfo` → `SocketAddr` helpers | v17.8.0 |
| Shared state (`Arc<S>` in `App`) | v17.9.0 |
| Dynamic routing (`:param`, `*wildcard`) | v17.9.0 |
| Middleware pipeline | v17.9.0 |
| HTTP/1.1 keep-alive | v17.4.0 |
| Async handlers | v17.11.0 |
| Typed request extractors (`FromRequest`) | v17.7.0 |
| Typed error handling (`IntoResponse`) | v17.6.0 |
| Streaming responses / chunked transfer | v17.4.0 |

What follows are the ideas worth building next, ordered by impact.

---

## 1. Database layer

The only item still marked ❌ in [LIKE_SPRING.md](LIKE_SPRING.md). Without it, any real application stores state in `Arc<Mutex<HashMap>>` and works around the missing piece manually.

`sqlx` is the right choice: async, compile-time query checking, no ORM overhead, supports PostgreSQL, MySQL, and SQLite. The integration point is `App::with_async_state(pool)` — pass a `sqlx::PgPool` as state and `await` queries inside `async fn` handlers.

```toml
# Cargo.toml — gated behind a new feature flag so non-DB users pay nothing
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio", "tls-rustls"], optional = true }
```

What to add:
- `src/db/mod.rs` — re-export `sqlx` pool types and a `DbError → AppError` adapter
- `features = ["db-postgres"]`, `["db-sqlite"]`, `["db-mysql"]` feature flags
- Use case example in DEVELOPER.md: `AsyncAppWithState<PgPool>` with a live query
- A `TestDb` helper for integration tests that spins up a SQLite in-memory pool

This is the highest-value addition. Nearly every serious application needs it, and right now there is no path at all.

---

## 2. Upstream TLS for the config-driven proxy

Phase 4 of [PROXY_SERVER_CONFIG.md](PROXY_SERVER_CONFIG.md). The `ConfigDrivenApp` / `DynamicProxy` currently speaks plain HTTP to backends. TLS upstreams (`https://` in `backends = [...]`) are silently treated as HTTP/1.1 plain text.

What to add in `src/proxy_config/`:
- `upstream.tls` table in `rws.config.toml`:

```toml
[[upstream]]
name = "secure-api"
backends = ["internal-api:443"]

  [upstream.tls]
  verify = true            # verify backend cert against system roots
  ca_file = "internal-ca.pem"  # optional custom CA
  client_cert = "client.pem"   # optional mTLS client cert
  client_key  = "client.key"
```

- In `health.rs` / `builder.rs`, detect `https://` prefix or `upstream.tls` presence and wrap the TCP stream with `tokio-rustls` before sending the HTTP request
- Reuse the existing `rustls` dep (already present in `http2` feature)

Until this lands, the proxy cannot talk to any backend that enforces TLS — which includes all managed cloud services.

---

## 3. Config-driven load balancing strategies

Phase 3 of [PROXY_SERVER_CONFIG.md](PROXY_SERVER_CONFIG.md). `DynamicProxy` today does round-robin only. Three more strategies cover most real-world needs:

```toml
[[upstream]]
name = "api"
backends = ["10.0.0.10:8080", "10.0.0.11:8080", "10.0.0.12:8080"]
load_balancing = "least_connections"  # or "ip_hash" | "random" | "round_robin"
```

| Strategy | Use case |
|---|---|
| `least_connections` | Long-lived connections (WebSocket, SSE) — routes new requests to the backend with the fewest active |
| `ip_hash` | Session stickiness without a cookie — same client IP always hits the same backend |
| `random` | Simplest possible distribution when backends are homogeneous and connections are short |

Implementation in `src/proxy_config/mod.rs`:
- Add `active_connections: Arc<Vec<AtomicUsize>>` alongside `backends` in `DynamicProxy`
- `least_connections`: select minimum index, `fetch_add`/`fetch_sub` on dispatch / response
- `ip_hash`: `hash(client_ip) % backend_count`, skip unhealthy entries
- `random`: `rand` crate already in-tree? If not, use `SystemTime` as a cheap entropy source

---

## 4. JWT and Basic auth from `rws.config.toml`

> **Status: resolved.** Wired as described below, with one deviation from the original plan: instead of a full htpasswd parser supporting MD5/SHA1/bcrypt, `BasicAuthLayer::from_htpasswd_file` supports only plain-text passwords and rws's own `{SHA256}` scheme (via the already-in-tree `sha2` crate) — real Apache `{SHA}` (SHA-1), `$apr1$`, and bcrypt are out of scope, since hand-rolling those hash algorithms isn't a risk worth taking for an auth check and this crate has no third-party crypto dependency for them. The `htpasswd_file` config key itself matches this doc's example exactly (the code previously read `users_file` — renamed to match, since the field was a no-op until this change). See `spec/TODO.md` for full detail.

Phase 6 of [PROXY_SERVER_CONFIG.md](PROXY_SERVER_CONFIG.md). Bearer token auth from config already works. JWT and htpasswd-file Basic auth are placeholders.

```toml
[route.middleware]
auth = { type = "jwt", secret_env = "JWT_SECRET" }
# or
auth = { type = "basic", htpasswd_file = ".htpasswd" }
```

What to add:
- In `apply_middleware()` in `builder.rs`, wire `"jwt"` arm to `JwtLayer` (already in `src/auth/`, gated on `auth` feature)
- For `"basic"`, add a minimal htpasswd parser (MD5/SHA1/bcrypt hashed entries, one `user:hash` per line) — or delegate to `BasicAuthLayer` with a closure that reads the file at startup
- Gate on `features = ["auth"]`; the config parser already reads the `type` field

---

## 5. Static site action in config proxy

Phase 7 of [PROXY_SERVER_CONFIG.md](PROXY_SERVER_CONFIG.md). A `type = "static"` action would let the config-driven proxy serve a directory of files without falling through to the built-in `App`.

```toml
[[route]]
name = "docs"

  [route.match]
  path = "/docs/*"

  [route.action]
  type = "static"
  root = "/var/www/docs"
  index = "index.html"
  strip_prefix = "/docs"
```

Implementation: a `StaticAdapter` that wraps the existing file-serving logic in `App` (range requests, ETag, MIME types, gzip, 304 Not Modified are already correct). The only new piece is the `strip_prefix` option.

This rounds out the three config-driven actions: `proxy`, `respond`, `redirect`, and now `static`.

---

## 6. Access log rotation

`GAPS.md` lists this as an open gap. The server writes access logs to stdout, which works fine with container runtimes (Docker, Kubernetes) that handle log collection. But bare-metal and VM deployments need on-disk rotation.

Two options:
- **Sidecar model (recommended for containers):** document using `logrotate` + `SIGHUP` to reopen the file. The server already handles `SIGHUP` and could reopen a log file descriptor in `config_reload::reload()`.
- **Built-in rotation:** add `RWS_CONFIG_ACCESS_LOG_FILE` and `RWS_CONFIG_ACCESS_LOG_MAX_MB` / `RWS_CONFIG_ACCESS_LOG_MAX_FILES`. A background thread rotates when the file exceeds the threshold.

The sidecar model is simpler and keeps the binary smaller. The built-in model is self-contained and useful for embedded deployments. Either way, the config flag and the `reload()` hook for file descriptor reopening are the same.

---

## 7. Regex URI rewriting

The `RewriteLayer` does literal prefix operations today. Production nginx configs are dominated by regex rewrites:

```nginx
rewrite ^/api/v1/(.*)$ /api/v2/$1 redirect;
```

Equivalent in `rws`:

```rust
.request_uri_rewrite(r"^/api/v1/(.*)", "/api/v2/$1")
```

Implementation in `src/rewrite/mod.rs`:
- Add a `RequestRule::RewriteUri { pattern: Regex, replacement: String }` variant
- Gate on a `regex` Cargo feature (adds the `regex` crate)
- Capture group substitution: replace `$1`–`$9` in `replacement` with matched groups
- Applied after `StripUriPrefix` in the request rule chain

The `regex` crate is already used by several Rust HTTP frameworks; pinning to `"1"` keeps compile times acceptable.

---

## 8. Forward-auth middleware

> **Status: resolved.** Implemented as `ForwardAuthLayer` in `src/auth/forward.rs` (per the more specific path named in `spec/TODO.md`, rather than directly in `src/auth/mod.rs` as sketched below). One addition beyond the original sketch: the internal HTTP client disables redirect-following (`max_redirects(0)`) so a `3xx` from the auth service reaches the client verbatim instead of being silently followed. See `spec/TODO.md` for full detail.

`GAPS.md` lists this under authentication: delegate auth decisions to an external HTTP service (Traefik's `ForwardAuth`, nginx `auth_request`).

```rust
let app = App::new()
    .wrap(ForwardAuthLayer::new("http://auth.internal/verify")
        .copy_header("X-User-Id")
        .copy_header("X-Roles")
        .timeout_ms(2000));
```

How it works:
1. Clone the request headers, send a `GET` to the auth service with them
2. `2xx` → continue; set any `copy_header` values from the auth response on the forwarded request
3. `4xx` → return the auth service's response verbatim (preserves `WWW-Authenticate`, `Location` for OAuth redirects)

Implementation: a new `ForwardAuthLayer` in `src/auth/mod.rs`; makes a plain HTTP/1.1 call to the auth endpoint (reuse the existing TCP connect logic from `ReverseProxy`). No new Cargo deps needed.

---

## 9. Multi-span distributed tracing

`OtelLayer` creates one span per HTTP request. Handlers cannot create child spans today — useful for database queries, external HTTP calls, and expensive computation.

```rust
use rust_web_server::otel::{current_span, SpanBuilder};

fn get_user(req: &Request, params: &PathParams, conn: &ConnectionInfo, state: &Db) -> Response {
    let span = SpanBuilder::new("db.query")
        .attribute("db.statement", "SELECT * FROM users WHERE id = ?")
        .start();
    let user = state.find_user(params.get("id").unwrap());
    span.finish();
    // ...
}
```

What to add in `src/otel/`:
- `thread_local!` active span stack; `SpanBuilder::start()` pushes onto it and sets `parentSpanId`
- `Span::finish()` sets `endTimeUnixNano` and enqueues for export
- `current_traceparent()` already exists — extend it to read from the span stack first

This is a significant quality-of-life improvement for anyone debugging latency in production.

---

## 10. Admin UI

The server already exposes `/metrics`, `/healthz`, `/readyz`, and `POST /admin/config/reload`. Grouping these into a lightweight browser UI would complete the observability story for local development and small deployments.

What to add:
- `GET /admin` → an HTML page (embedded as a `&'static str` or using the `tera` feature) showing:
  - Current config (`RWS_CONFIG_*` values)
  - Live metrics (requests/sec, error rate, active connections)
  - Rate limiter state per IP
  - Reload button → `POST /admin/config/reload`
- Gate behind `BasicAuthLayer` by default; configurable via `RWS_CONFIG_ADMIN_PASSWORD`
- No JavaScript frameworks — plain HTML + CSS + a `<meta http-equiv="refresh">` for auto-update, or minimal vanilla JS polling `/metrics`

The UI itself requires no new Cargo dependencies. The Prometheus text format from `/metrics` can be parsed client-side in under 30 lines of JavaScript.

---

## Recommended order

| Priority | Idea | Blocker for |
|---|---|---|
| 1 | Database layer (sqlx) | Every real application |
| 2 | Upstream TLS for proxy | Cloud-native deployments |
| 3 | Config-driven load balancing | Production traffic distribution |
| 4 | JWT/Basic auth from config | Config-only auth stories |
| 5 | Static site action in config proxy | Single-binary site serving |
| 6 | Access log rotation | Bare-metal deployments |
| 7 | Regex URI rewriting | Complex routing migrations |
| 8 | Forward-auth middleware | OAuth/SSO integration |
| 9 | Multi-span tracing | Production latency debugging |
| 10 | Admin UI | Ops convenience |
