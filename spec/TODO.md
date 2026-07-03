[Read Me](../README.md) > [Spec](.) > TODO

# TODO — rws v17.43.0+

Consolidated, prioritized task list synthesized from GAPS_V3.md, IDEAS.md, ADMIN_ROADMAP.md, and all open roadmap items. Items are ordered within each tier by the ratio of impact to implementation effort.

**Status as of 2026-07-03:** Priority 1 is fully complete. Priority 2 is the current focus. Code inspection this date found two Priority 2 items were silent-failure bugs (config accepted but ignored, no error) rather than plain feature gaps — the static-site action and the load-balancer `strategy` field. Both were promoted to the top of the tier with exact file:line root causes, and both are now fixed (see below).

---

## ✅ Priority 1 — Complete

All six blocking gaps have been resolved. rws is now suitable for real production workloads.

- [x] **Upstream connection pooling** (`src/proxy/pool.rs`) — `ConnPool` (Mutex-backed, per-backend VecDeque of TcpStream) is embedded in `ReverseProxy`. Idle connections are reused when the backend sends `Connection: keep-alive`; chunked `Transfer-Encoding` is decoded so body length is known. Share pools across instances with `Arc<ConnPool>` via `ReverseProxy::with_pool()`. Closes GAPS_V3 §1.1 and §2.6.

- [x] **TLS to HTTP/2 upstreams** (`H2ReverseProxy`) — `H2ReverseProxy` now supports `https://` and `h2s://` backend URLs. `Backend::parse()` detects TLS schemes (port defaults to 443); `forward_h2_async` branches: plain path uses `TcpStream` directly; TLS path wraps in `tokio_rustls::TlsConnector` with ALPN `h2` before the h2 handshake. Generic `send_h2_request<T>` accepts both stream types. Requires `http2` feature. Closes GAPS_V3 §2.2.

- [x] **TLS to gRPC upstreams** (`grpcs://`) — `GrpcProxy` inherits TLS from `H2ReverseProxy`. `grpcs://` and `https://` backend URLs connect over TLS with ALPN `h2`. Closes GAPS_V3 §2.3.

- [x] **TLS to WebSocket upstreams** (`wss://`) — `WsProxy` now accepts `wss://host:port` backend URLs (port defaults to 443). TLS path uses `rustls::StreamOwned` + a single-thread polling loop (5 ms timeout per side, 1 ms sleep when idle) to avoid the deadlock that arises when sharing a TLS stream between two blocking relay threads. Plain `ws://` backends continue to use the two-thread `std::io::copy` approach. Requires `http-client` or `http2` feature; returns 502 otherwise. Closes GAPS_V3 §2.4.

- [x] **Persistent sessions** (`src/session/mod.rs`) — Added `DbSessionStore` backed by the model layer (`rws_sessions` table: id TEXT PK, data TEXT URL-encoded, expires_at INTEGER epoch). Auto-creates table on first `new()`. All methods return `Result`. Added `RedisSessionStore` backed by a hand-rolled RESP v2 client (no external crate); sessions keyed as `rws:sess:{id}`, TTL via `SET … EX`, auto-reconnect. `from_env()` reads `RWS_REDIS_HOST/PORT/PASSWORD/TTL_SECS`. 10 new tests. Closes GAPS_V3 §3.5.

- [x] **Streaming response passthrough through proxy** — Added `Response::stream_pipe: Option<Box<dyn Read + Send>>` and `Server::pipe_stream()`. `ReverseProxy::try_backend()` now reads only headers, detects streaming signals (`Content-Type: text/event-stream`, `Transfer-Encoding: chunked`, `Content-Length > 1 MB`), and for matching responses sets `stream_pipe` to a `ConcatReader(body_prefix, TcpStream)` instead of buffering. `pipe_stream` forwards chunked-backend bytes as raw passthrough; plain SSE bytes are re-encoded as chunks. Closes GAPS_V3 §1.2.

- [x] **Email / SMTP** (`src/mailer/mod.rs`, `mailer` feature) — Added `Mailer`, `Email`, `EmailBuilder`, `MailerError`, `SmtpTls`. Hand-rolled SMTP client (no external crate): plain TCP (`SmtpTls::None`), STARTTLS upgrade (`SmtpTls::Starttls`, requires `http-client`/`http2`), implicit TLS (`SmtpTls::Smtps`). AUTH PLAIN, RFC 5322 message builder with text/html/multipart bodies, SMTP dot-stuffing. `Mailer::from_env()` reads `RWS_SMTP_HOST/PORT/USER/PASSWORD/FROM/TLS/TIMEOUT_MS`. 14 tests. Closes GAPS_V3 §3.1 and GAPS_V2 §5.

---

## Priority 2 — High friction without these

Commonly needed; workarounds exist but are painful. **This is the current focus.**

**Two silent-failure bugs confirmed by code inspection on 2026-07-03 — promoted to the top of this tier.** Both accept config that parses successfully and produce different behavior than the config states, with no error or log line. That's worse than a missing feature (which fails loudly) and each is a small, isolated fix.

- [x] **Static site action in config-driven proxy is a no-op** (`type = "static"`) — Fixed. Added `StaticAdapter` (`src/proxy_config/mod.rs`, "StaticAdapter" section) implementing `Application`: resolves the request path against the configured `root`, tries each `index` entry in order for directory requests (default `["index.html"]`), rejects any `..` path segment (pre- or post-percent-decode) with `403`, and returns `404` for anything else missing. Also canonicalizes and checks `starts_with(root)` as defense-in-depth against symlinks inside `root` pointing outside it. Reuses `Range::get_content_range_of_a_file()` for MIME detection and body construction — same code path the built-in static controller uses. `builder.rs:86-88` now constructs `StaticAdapter::new(root, index)` instead of falling back to `App::new()`. 4 new tests in `src/proxy_config/tests.rs` (serve file, serve directory index, reject traversal, 404 on missing file); full `cargo test` passes (1132 unit + 72 doc tests). Docs updated: `docs/proxy/config-driven.mdx`, `DEVELOPER.md` (building blocks table + Use Case #52), `llms.txt`; removed the stale "Coming Soon" callout from `docs/reference/roadmap.md`. Closed GAPS_V3 §2.8 and IDEAS.md §5.

- [x] **`strategy` field on `[[upstream]]` is parsed but never read** — Fixed. Added `LoadBalanceStrategy` enum (`src/proxy_config/mod.rs`, "DynamicProxy" section) with `RoundRobin` (default, also the fallback for unknown/empty values), `Random`, `IpHash`, and `LeastConnections`. `DynamicProxy::new()` now takes a `strategy: String` (parsed once via `LoadBalanceStrategy::parse`) and a `connections: Arc<RwLock<HashMap<String, Arc<AtomicUsize>>>>` map; `next_backend(client_ip)` branches on the strategy — `IpHash` hashes the client IP with `DefaultHasher` for per-client stickiness, `LeastConnections` picks the live backend with the lowest counter, `Random` mixes a nanosecond timestamp with the existing round-robin counter (no new crate dependency). A `ConnectionGuard` (RAII, decrements on `Drop`) tracks in-flight counts around each proxied request for `LeastConnections`. Both `builder.rs` call sites (`proxy` and `grpc` actions) now look up `upstream.strategy` and pass it through. 6 new white-box unit tests exercise each strategy directly against `DynamicProxy`, plus one end-to-end test (`config_driven_app_ip_hash_strategy_is_sticky_end_to_end`) that spins up two real TCP backends and confirms a client IP is pinned to one of them through the full `ProxyConfig::from_str` → `builder::build` → `ConfigDrivenApp` path. Full `cargo test` passes (1139 unit + 72 doc tests). Also fixed a nested-table bug in `llms.txt`'s config-driven proxy example (`upstream = "api"` was written directly under `[route.action]` instead of `[route.action.proxy]`, which the hand-rolled TOML parser — keyed by exact section path — would silently fail to parse; caught while adding the strategy docs there). Docs updated: `docs/proxy/config-driven.mdx`, `docs/configuration/config-file.md`, `DEVELOPER.md` (building blocks table + Use Case #52), `llms.txt`; removed the "Coming Soon" load-balancing-strategies callout from `docs/reference/roadmap.md`. Closed GAPS_V3 §2.1 and IDEAS.md §3.

- [x] **Background job queue** (`src/jobs/mod.rs`, `jobs` feature) — Added. `Job` trait (blanket-implemented for `Fn() -> Result<(), String> + Send` closures, so a plain closure or a named struct both work) and `JobQueue::new(workers)`, an in-memory fixed worker pool. `.submit(job)` enqueues; a failing job retries on the same worker thread with exponential backoff (`.max_retries(n)` / `.backoff(initial, multiplier)`, default 3 retries / 500ms / 2x — `max_retries` counts retries *after* the first attempt); `.join()` drains and waits. Also added `PersistentJobQueue` (gated additionally on `model-sqlite`/`model-postgres`/`model-mysql`), backed by a `rws_jobs` table via the model layer: since a closure can't be serialized, persisted jobs are `(job_type, payload)` string pairs dispatched to a handler registered by name via `.register(job_type, fn)`. `PersistentJobQueue::new(pool).await` creates the table and resets any row left `running` by a crash back to `pending`; `.enqueue()`/`.enqueue_with_retries()` persist a job; `.start(workers)` spawns polling worker threads (each with its own single-threaded Tokio runtime, since the rest of the queue is plain-thread/std-only and doesn't otherwise require a runtime); `.tick().await` runs one poll-claim-execute cycle for tests or a caller-owned loop. Row claiming uses `UPDATE ... WHERE status = 'pending'` so concurrent workers (same process or cross-process against the same DB) can't double-claim a row. 5 `JobQueue` tests + 5 `PersistentJobQueue` tests (incl. crash recovery: a row manually left `running`, then a fresh `PersistentJobQueue::new` against the same pool picks it back up). Full `cargo test` (default features) unaffected since `jobs` is opt-in; verified separately with `cargo test --features jobs` (5 passed) and `cargo test --no-default-features --features jobs,model-sqlite` (10 passed). Docs: new `docs/features/jobs.md` page (registered in `astro.config.mjs`), `DEVELOPER.md` (building blocks rows + Use Case #62), `README.md`, `llms.txt` (new section + module index + `reference/api.md` + `getting-started/features.md`). Closes GAPS_V3 §3.3 and GAPS_V2 §7.

- [x] **File / object storage abstraction** (`src/storage/`, `storage-local` / `storage-s3` features) — Added. `Storage` trait (`put`/`get`/`delete`/`url`) plus `LocalStorage` (files under a root dir; rejects `..` key segments; `.with_base_url()` for serving uploads back over HTTP) and `S3Storage` (AWS S3, R2, MinIO — path-style addressing, `S3Storage::from_env()` reads `RWS_S3_BUCKET/REGION/ACCESS_KEY/SECRET_KEY/ENDPOINT`). `S3Storage` signs every request with hand-rolled AWS SigV4 (`src/storage/aws_sigv4.rs`) using `hmac`+`sha2` over the existing `crate::http_client::Client` — no AWS SDK. One deviation from the original GAPS_V2 spec text: `storage-s3` depends on `hmac`+`sha2` directly (same crates the `auth` feature already uses for JWT HS256) rather than `crypto` (Argon2 password hashing), since SigV4 needs HMAC-SHA256, not password hashing. 32 tests: 9 `LocalStorage`/`aws_sigv4` unit tests plus an 18-test suite for `S3Storage`/`aws_sigv4`/`S3Config::from_env` that spins up a local mock TCP "S3" server to verify the actual request path, headers (`Authorization`, `x-amz-date`, `x-amz-content-sha256`, exactly one `Host`), and body bytes end-to-end — not just the signer in isolation. Verified across `storage-local`, `storage-s3`, and both together; full `cargo test` (default features) unaffected. Docs: new `docs/features/storage.md` page (registered in `astro.config.mjs`) plus a cross-link from `docs/building-apps/forms-uploads.md` (the exact gap this closes); `DEVELOPER.md` (2 building-blocks rows + Use Case #63), `README.md` (new section + 2 feature-table rows), `llms.txt` (new section + module index + `reference/api.md` + `getting-started/features.md`). Closes GAPS_V3 §3.2 and GAPS_V2 §6.

- [x] **OpenAPI / Swagger schema generation** (`src/openapi/`, `openapi` feature) — Added. `OpenApiConfig::new(title, version)` + `build_spec(&config, &[RouteInfo])` produce a hand-built OpenAPI 3.0.3 JSON document (same technique as the MCP server's JSON-RPC responses — no `serde_json` dependency). `AppWithState::openapi(config)` / `AsyncAppWithState::openapi(config)` are the ergonomic entry points: each snapshots `self.route_entries()` at call time and registers `GET /openapi.json` (the spec) and `GET /docs` (Swagger UI via the `unpkg.com/swagger-ui-dist` CDN). Scope is deliberately paths/methods/path-params only (`:id`/`*path` → `{id}`/`{path}` with a `parameters` entry) — no request/response body schemas, since Rust has no runtime type reflection to extract a JSON Schema from a `#[derive(Validate)]` struct without a much larger macro-level feature; every operation gets a generic `200 OK` response, documented as an explicit scope boundary rather than a silent gap. As a side effect of wiring `AsyncAppWithState::openapi()`, gave it a `route_entries()` method it didn't have before (mirroring `Router`/`AppWithState`), and moved the shared `segments_to_pattern` helper (previously private to `Router`) into `src/router/matcher.rs` alongside the `Segment`/`parse_pattern`/`try_match` dedup from the earlier dispatch-mechanism fix, so both app types build route-info strings from the same code. 19 new tests: 12 for `build_spec`/`swagger_ui_html` in isolation (title/version/description, path-param conversion for both `:name` and `*name`, multi-method-per-path merging, JSON escaping), 4 end-to-end for `AppWithState::openapi()`, 3 end-to-end for `AsyncAppWithState::openapi()` (via `Application::execute`, not just unit-level). Verified across `openapi`, `http2,openapi`, and default+`openapi` feature combinations; default build (no `openapi` feature) unaffected. Docs: new `docs/features/openapi.md` page (registered in `astro.config.mjs`) plus `getting-started/features.md` and `reference/api.md`; `DEVELOPER.md` (building-blocks row + Use Case #64), `README.md`, `llms.txt` (new section + module index). Closes GAPS_V3 §3.4 and GAPS_V2 §8.

- [x] **Async ORM** — `src/model/` rewritten to use `sqlx 0.8` as the async database driver. `DbPool` wraps `sqlx::Pool<Db>` (cheap to clone); `DbTransaction` wraps `sqlx::Transaction<'static, Db>`. Old `DbConnection` and `PooledConnection` types removed. All ORM methods (`save`, `find_all`, `find_by_id`, `delete_by_id`, `count`, `exists_by_id`, `QueryBuilder` terminals, relation `.load()`, `migrate()`, `migration_status()`) are now `async fn` and return `Result<_, DbError>`. `DbSessionStore` updated to `async fn`. All model and session tests use `#[tokio::test]`. 16 model integration tests + 31 session tests pass. Closes GAPS_V3 §3.7.

- [ ] **Per-route timeouts** — a single global read timeout applies to every route. A file-upload endpoint needs 120 s; a health check needs 500 ms. Add a per-route override in the config and a `TimeoutLayer` middleware. Closes GAPS_V3 §1.5.

- [ ] **Request ID middleware** (`src/request_id/mod.rs`) — no automatic `X-Request-Id` / `X-Correlation-Id` generation or propagation. Essential for correlating log lines across services. `OtelLayer` creates spans but does not inject a stable request ID header accessible to application code. Closes GAPS_V3 §1.6.

- [ ] **JWT / Basic auth from `rws.config.toml`** — `JwtLayer` and `BasicAuthLayer` require Rust code. Add `auth = { type = "jwt", secret_env = "JWT_SECRET" }` and `auth = { type = "basic", htpasswd_file = ".htpasswd" }` in `[route.middleware]`. Wire to existing middleware in `builder.rs`. Closes GAPS_V3 §2.7 and IDEAS.md §4.

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

- [ ] **`wss://` proxy health checks** — `health.rs` parses `https://` backends and performs TLS health checks, but `wss://` scheme is not recognised. Health checks for `wss://` backends in `rws.config.toml` fall back to plain TCP and fail silently on TLS-only backends. Add `wss://` to `parse_backend_url`.

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
| Priority 2 + 3 items | [GAPS_V3.md](GAPS_V3.md) — §1–§3 priority table |
| Storage, jobs, OpenAPI | [GAPS_V2.md](GAPS_V2.md) — §6–§8 |
| LB strategies, ForwardAuth, regex rewrite, access log | [IDEAS.md](IDEAS.md) — §1–§10 |
| Admin UI phases | [ADMIN_ROADMAP.md](ADMIN_ROADMAP.md) |
| GAPS_V3 shortest path | [GAPS_V3.md §Shortest path](GAPS_V3.md) |
