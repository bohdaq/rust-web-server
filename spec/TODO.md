[Read Me](../README.md) > [Spec](.) > TODO

# TODO — rws v17.43.0+

Consolidated, prioritized task list synthesized from GAPS_V3.md, IDEAS.md, ADMIN_ROADMAP.md, and all open roadmap items. Items are ordered within each tier by the ratio of impact to implementation effort.

---

## Priority 1 — Blocking for production adoption

These gaps make rws unsuitable for real production workloads regardless of how complete the feature set is.

- [x] **Upstream connection pooling** (`src/proxy/pool.rs`) — `ConnPool` (Mutex-backed, per-backend VecDeque of TcpStream) is embedded in `ReverseProxy`. Idle connections are reused when the backend sends `Connection: keep-alive`; chunked `Transfer-Encoding` is decoded so body length is known. Share pools across instances with `Arc<ConnPool>` via `ReverseProxy::with_pool()`. Closes GAPS_V3 §1.1 and §2.6.

- [x] **TLS to HTTP/2 upstreams** (`H2ReverseProxy`) — `H2ReverseProxy` now supports `https://` and `h2s://` backend URLs. `Backend::parse()` detects TLS schemes (port defaults to 443); `forward_h2_async` branches: plain path uses `TcpStream` directly; TLS path wraps in `tokio_rustls::TlsConnector` with ALPN `h2` before the h2 handshake. Generic `send_h2_request<T>` accepts both stream types. Requires `http2` feature. Closes GAPS_V3 §2.2.

- [x] **TLS to gRPC upstreams** (`grpcs://`) — `GrpcProxy` inherits TLS from `H2ReverseProxy`. `grpcs://` and `https://` backend URLs connect over TLS with ALPN `h2`. Closes GAPS_V3 §2.3.

- [x] **TLS to WebSocket upstreams** (`wss://`) — `WsProxy` now accepts `wss://host:port` backend URLs (port defaults to 443). TLS path uses `rustls::StreamOwned` + a single-thread polling loop (5 ms timeout per side, 1 ms sleep when idle) to avoid the deadlock that arises when sharing a TLS stream between two blocking relay threads. Plain `ws://` backends continue to use the two-thread `std::io::copy` approach. Requires `http-client` or `http2` feature; returns 502 otherwise. Closes GAPS_V3 §2.4.

- [x] **Persistent sessions** (`src/session/mod.rs`) — Added `DbSessionStore` backed by the model layer (`rws_sessions` table: id TEXT PK, data TEXT URL-encoded, expires_at INTEGER epoch). Auto-creates table on first `new()`. All methods return `Result`. Added `RedisSessionStore` backed by a hand-rolled RESP v2 client (no external crate); sessions keyed as `rws:sess:{id}`, TTL via `SET … EX`, auto-reconnect. `from_env()` reads `RWS_REDIS_HOST/PORT/PASSWORD/TTL_SECS`. 10 new tests. Closes GAPS_V3 §3.5.

- [ ] **Email / SMTP** (`src/mailer/mod.rs`, `mailer` feature) — no path to send email from a handler today. Add `Mailer::from_env()` reading `RWS_SMTP_HOST/PORT/USER/PASSWORD/FROM`; minimal SMTP or thin `lettre` wrapper. Required for password reset, verification emails, transactional notifications. Closes GAPS_V3 §3.1 and GAPS_V2 §5.

- [ ] **Streaming response passthrough through proxy** — `read_response_from()` buffers the entire backend response before forwarding. Breaks SSE-through-proxy, AI token streams (OpenAI, Anthropic), and large file downloads. Pass body bytes to the client as they arrive. Closes GAPS_V3 §1.2.

---

## Priority 2 — High friction without these

Commonly needed; workarounds exist but are painful.

- [ ] **Config-driven load balancing strategies** — `DynamicProxy` round-robin only. Add `least_connections`, `ip_hash`, and `random` via `load_balancing = "…"` in `rws.config.toml`. `least_connections` is the most important (WebSocket and SSE stickiness). See IDEAS.md §3. Closes GAPS_V3 §2.1.

- [ ] **File / object storage abstraction** (`src/storage/`, `storage-local` / `storage-s3` features) — `FormMultipartData::parse()` returns raw bytes with no place to put them. Add a `Storage` trait with `LocalStorage` (no deps) and `S3Storage` using the existing outbound HTTP client to sign AWS Signature V4 requests. Closes GAPS_V3 §3.2 and GAPS_V2 §6.

- [ ] **Background job queue** (`src/jobs/`, `jobs` feature) — no facility for ad-hoc one-shot jobs from handlers ("send this email after signup"). Add a `Job` trait, `JobQueue::new(workers)`, retry-with-backoff, and a `PersistentJobQueue` backed by the model layer for crash safety. Closes GAPS_V3 §3.3 and GAPS_V2 §7.

- [ ] **OpenAPI / Swagger schema generation** (`src/openapi/`, `openapi` feature) — no API schema from routes. `AppWithState` route registrations and `#[derive(Validate)]` already capture all needed data. Serve `GET /openapi.json`; optionally serve Swagger UI as embedded HTML. Closes GAPS_V3 §3.4 and GAPS_V2 §8.

- [ ] **Async ORM** — `src/model/` uses blocking I/O; calling ORM methods in `AsyncAppWithState` handlers blocks a tokio worker thread. Add `async fn save`, `async fn find_all`, async `Repository` via `tokio::task::spawn_blocking` or `sqlx` as an alternative async backend. Closes GAPS_V3 §3.7.

- [ ] **Per-route timeouts** — a single global read timeout applies to every route. A file-upload endpoint needs 120 s; a health check needs 500 ms. Add a per-route override in the config and a `TimeoutLayer` middleware. Closes GAPS_V3 §1.5.

- [ ] **Request ID middleware** (`src/request_id/mod.rs`) — no automatic `X-Request-Id` / `X-Correlation-Id` generation or propagation. Essential for correlating log lines across services. `OtelLayer` creates spans but does not inject a stable request ID header accessible to application code. Closes GAPS_V3 §1.6.

- [ ] **JWT / Basic auth from `rws.config.toml`** — `JwtLayer` and `BasicAuthLayer` require Rust code. Add `auth = { type = "jwt", secret_env = "JWT_SECRET" }` and `auth = { type = "basic", htpasswd_file = ".htpasswd" }` in `[route.middleware]`. Wire to existing middleware in `builder.rs`. Closes GAPS_V3 §2.7 and IDEAS.md §4.

- [ ] **Static site action in config-driven proxy** (`type = "static"`) — serving a local directory from `rws.config.toml` without Rust code is currently broken; the builder falls through to `App::new()` without applying a root directory. Fix `builder.rs` to create a `StaticAdapter` that wraps the existing file-serving logic. Closes GAPS_V3 §2.8 and IDEAS.md §5.

- [ ] **ForwardAuth middleware** (`src/auth/forward.rs`) — delegate auth decisions to an external service (OPA, Casbin, a centralized auth API). On 2xx, copy nominated headers onto the downstream request. On 4xx, return the auth service response verbatim. No new deps. Closes GAPS_V3 §2.9 and IDEAS.md §8.

- [ ] **Cookie signing and encryption** — `SetCookie` builder produces plain-text values; no `signed_cookie(value, secret)` (HMAC-SHA256) or `encrypted_cookie(value, key)` (AES-GCM). Applications storing session tokens in cookies are vulnerable to tampering. Closes GAPS_V3 §3.11.

- [ ] **RS256 / ES256 in `JwtLayer`** — `JwtLayer` verifies HS256 only. Service-to-service auth where the caller presents an RS256 JWT currently requires importing the entire `sso` feature. Add RS256/ES256 support to `src/auth/mod.rs` directly (reuse `rsa`/`p256` from the `sso` dep, or gate on a slim `auth-asymmetric` feature). Closes GAPS_V3 §3.10.

- [ ] **Webhook signature verification** (`src/webhook/mod.rs`) — no `verify_webhook_signature(body, secret, header)` helper. Every webhook-receiving handler must independently wire `hmac` + `sha2` even though both are already in the dep tree. Add helpers for the most common schemes: Stripe, GitHub, Shopify. Closes GAPS_V3 §3.14.

---

## Priority 3 — Improves quality and completeness

Genuine gaps that real applications hit; none are blockers with a workaround.

- [ ] **Multi-span distributed tracing** — `OtelLayer` creates one flat span per request. Handlers cannot create child spans ("db.query", "http.outbound", "cache.lookup"). Add `thread_local!` span stack and a `SpanBuilder` API in `src/otel/`. Closes GAPS_V3 §3.15 and IDEAS.md §9.

- [ ] **Regex URI rewriting** — `RewriteLayer` supports only literal prefix operations. Add a `RequestRule::RewriteUri { pattern: Regex, replacement: String }` variant, gated on a `rewrite-regex` feature (adds `regex` crate). Closes GAPS_V3 §2.10 and IDEAS.md §7.

- [ ] **CanaryLayer TLS backends** — `CanaryLayer` calls `proxy::proxy_http1()` only; no `https://` scheme detection. Closes GAPS_V3 §2.5.

- [ ] **Async `H2ReverseProxy`** — currently uses `tokio::task::block_in_place` to bridge sync middleware into the tokio runtime; panics on `current_thread` runtime and blocks a worker thread. Replace with a natively async internal implementation. Closes GAPS_V3 §2.12.

- [ ] **Distributed rate limiter** — `RateLimiter` and `CircuitBreaker` are per-process. Two instances behind a load balancer have independent state. Add a `RedisRateLimiter` and a `SqliteRateLimiter` (shared file) for deployments that need global enforcement. Closes GAPS_V3 §2.11 and §3.6.

- [ ] **DB migration rollback** (`conn.rollback_to(version)`) — `conn.migrate()` applies only forward. Add a `_down.sql` convention and `conn.rollback_to_version(n)` for deploy rollback. Closes GAPS_V3 §3.9.

- [ ] **Streaming request body** — `Request::parse()` buffers the entire request body before any handler runs. Add a `BodyReader` / async iterator interface, and a per-route `max_body_size` config option. Closes GAPS_V3 §1.3 and §2.13.

- [ ] **Pagination helpers** — `QueryBuilder` has `.limit()` / `.offset()` but no `Page<T>` type, no `Link: <url>; rel="next"` builder, and no cursor-based pagination. Every list endpoint re-implements pagination. Closes GAPS_V3 §3.13.

- [ ] **Multiple DB backends per binary** — `model-sqlite`, `model-postgres`, `model-mysql` are `#[cfg]`-exclusive. A binary cannot hold connections to both SQLite (hot cache) and PostgreSQL (analytics). Refactor `DbConnection` into an enum. Closes GAPS_V3 §3.8.

- [ ] **Tera template hot-reload** — `template::init()` compiles templates once at startup. Add a watcher that re-reads the template directory on `SIGHUP` / `POST /admin/config/reload`. Closes GAPS_V3 §3.12.

- [ ] **`100 Continue` support** — rws reads the full body unconditionally; clients sending `Expect: 100-continue` waste bandwidth on requests the server would reject. Reply with `100 Continue` before reading the body. Closes GAPS_V3 §1.4.

- [ ] **`wss://` proxy health checks** — `health.rs` sends `GET / HTTP/1.1` over plain TCP. Health checks for TLS-only backends fail silently. Add TLS-aware health-check connections.

- [ ] **Proxy max body size** — no `max_body_size` in `MatchConfig` or `MiddlewareConfig`. A client can stream an unbounded POST to a proxied route consuming all RAM. Closes GAPS_V3 §2.13.

- [ ] **Circuit breaker persistence** — `CircuitBreaker` state resets on process restart; a backend that triggered the circuit just before a deploy appears healthy immediately on startup. Store state via the model layer. Closes GAPS_V3 §2.14.

---

## Priority 4 — Nice to have

Low urgency; workarounds are acceptable or audience is small.

- [ ] **Admin UI** (`src/admin/`) — 7-phase roadmap in `spec/ADMIN_ROADMAP.md`. Embedded single-page HTML at `GET /admin` backed by a JSON REST API (`/admin/api/*`). Covers live config editing, IP filter management, proxy backend management, metrics dashboard, session inspector, and SSE access log tail. Gated behind `RWS_ADMIN_TOKEN`. Closes GAPS_V2 §9 and GAPS_V3 §3.17.
  - [ ] Phase 1: `RuntimeConfig` + `AdminAuthLayer` + skeleton endpoint
  - [ ] Phase 2: Mutable rate-limit, CORS, IP filter via API
  - [ ] Phase 3: Reverse proxy backend management
  - [ ] Phase 4: JSON metrics endpoint
  - [ ] Phase 5: Session inspector
  - [ ] Phase 6: SSE access log tail
  - [ ] Phase 7: Embedded admin UI HTML

- [ ] **i18n / localization** (`src/i18n/mod.rs`) — `Accept-Language` is already parsed (`src/language/mod.rs`) but there is no locale resolver or string translation helper. Add a thin loader for `locales/*.toml` files and a `t(key, locale)` lookup. Closes GAPS_V3 §3.16 and GAPS_V2 §10.

- [ ] **Access log rotation** — logs go to stdout only. For bare-metal deployments, add `RWS_CONFIG_ACCESS_LOG_FILE` + `RWS_CONFIG_ACCESS_LOG_MAX_MB` / `MAX_FILES` and a background rotation thread. Alternatively, document `logrotate` + `SIGHUP` for the sidecar model. Closes GAPS_V3 §1.7 and IDEAS.md §6.

- [ ] **WebSocket `permessage-deflate` compression** (RFC 7692) — text-heavy WebSocket traffic (chat, JSON events) is 3–10× larger without compression. Negotiate `Sec-WebSocket-Extensions: permessage-deflate`; `flate2` is already in the dep tree. Closes GAPS_V3 §1.8.

- [ ] **WebSocket over HTTP/2** (RFC 8441) — clients that upgraded to HTTP/2 via ALPN must downgrade to HTTP/1.1 for WebSocket. Implement the RFC 8441 bootstrap to avoid the renegotiation round-trip. Closes GAPS_V3 §1.9.

- [ ] **GraphQL** — no integration with `async-graphql` or `juniper`. Add a thin `src/graphql/mod.rs` adapter that wraps `async-graphql`'s schema behind an `Application`. Closes GAPS_V3 §3.18 and GAPS_V2 §11.

- [ ] **WebAssembly / `wasm32-wasi` target** — OS threads, `TcpStream`, and `aws-lc-rs` do not compile to WASM. A `wasm32-wasi` shim layer would enable running rws handlers inside Wasmtime, WasmEdge, or Fastly Compute. Closes GAPS_V3 §3.19.

- [ ] **HTTP/2 and HTTP/3 server push** — no server-push API exposed to handlers. Pre-push CSS/JS alongside an HTML response. Minor gap given cache interaction problems. Closes GAPS_V3 §1.10.

---

## Cross-reference

| This file | Source spec |
|---|---|
| Priority 1 + 2 items | [GAPS_V3.md](GAPS_V3.md) — §1–§3 priority table |
| Email, storage, jobs, OpenAPI | [GAPS_V2.md](GAPS_V2.md) — §5–§8 |
| LB strategies, ForwardAuth, regex rewrite, access log | [IDEAS.md](IDEAS.md) — §1–§10 |
| Admin UI phases | [ADMIN_ROADMAP.md](ADMIN_ROADMAP.md) |
| GAPS_V3 shortest path | [GAPS_V3.md §Shortest path](GAPS_V3.md) |
