---
title: All Features
description: A complete checklist of rust-web-server capabilities organized by category, with feature flag annotations.
---

This page is a single-page reference for everything the crate ships. Items that require an opt-in feature flag are annotated with the flag name in backticks.

---

## Server / Protocol

- **HTTP/1.1** — keep-alive, chunked transfer encoding, gzip compression, ETags, range requests, 30-second per-connection read timeout
- **HTTP/2** — ALPN negotiation, full multiplexing, forbidden-header stripping, `Alt-Svc` advertisement — `http2`
- **HTTP/3 / QUIC** — UDP listener via `quinn`, `h3` + `h3-quinn`, QUIC connection-level SNI — `http3` *(default)*
- **TLS** — `rustls` with `aws-lc-rs` crypto backend; no system OpenSSL dependency; FIPS-compatible — `http2`
- **mTLS** — client certificate verification via `WebPkiClientVerifier`; set `RWS_CONFIG_TLS_CLIENT_CA_FILE` — `http2`
- **Virtual-host routing** — per-domain TLS certificates via `SniCertResolver`; `.with_host("example.com")` on any `Router` — `http2`
- **HTTP → HTTPS redirect** — `Server::run_redirect()` listens on a second port and issues `301` responses — `http2`
- **ACME / automatic TLS** — automatic certificate provisioning compatible with Let's Encrypt — `acme`
- **Hot reload** — `SIGHUP` (or `POST /admin/config/reload`) reloads CORS rules, rate limits, log format, and TLS certs without restarting — `http2`
- **Graceful shutdown** — `SIGINT` / `SIGTERM` drain in-flight requests before exit; `SERVER_READY` flag cleared on shutdown
- **Gzip compression** — applied automatically based on `Accept-Encoding`; configurable minimum size
- **Static file serving** — directory listing disabled by default; chunked file streaming for large files; ETag + `Last-Modified` caching headers
- **Thread pool** (http1 only) — hand-rolled `ThreadPool`; connection-per-thread; no tokio — `http1`
- **Tokio async runtime** — used for `http2` and `http3` builds; `http1` is fully synchronous

---

## Routing & App Building

- **`App`** — zero-config entry point; wraps all built-in controllers (static files, CORS, health, metrics, MCP, …)
- **`App::with_state(S)`** — state-aware dynamic router; `Arc<S>` shared across all handlers; builder methods `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()`
- **`App::with_async_state(S)`** — same API with `async fn` handlers — `http2`
- **`routes!` macro** — ergonomic route registration: `GET "/path" => handler` — `macros`
- **`Router`** — standalone path-based router with named parameters (`:name`) and trailing wildcards (`*name`); `PathParams::get(name)`
- **`Controller` trait** — low-level `is_matching` / `process` interface; register in `App::execute` for maximum control
- **Typed extractors** — `Body` (raw bytes), `BodyText` (UTF-8), `Query` (parsed query string), `RequestHeaders` (case-insensitive header map)
- **Typed errors** — `AppError` enum (`BadRequest`, `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `UnprocessableEntity`, `TooManyRequests`, `Internal`) with `IntoResponse` trait
- **`Middleware` trait** — `handle(request, connection, next) -> Result<Response, String>`; compose via `.wrap(layer)`
- **`WithMiddleware<A>`** — wraps any `Application`; layers applied in push order (first-pushed is outermost)
- **Tera templates** — render `.html` templates with a `serde_json::Value` context via `tera::render` — `tera`
- **In-process test client** — `TestClient::new(app)` dispatches requests without opening a TCP socket; use in unit and integration tests
- **WebSocket** — RFC 6455 handshake and frame codec built in; no third-party WebSocket library
- **SSE (Server-Sent Events)** — `Sse` builder for streaming events; ideal for streaming AI model output
- **Background scheduler** — `Scheduler::new().every(duration, fn).cron("…", fn).start()`
- **Background job queue** — `JobQueue` (in-memory) or `PersistentJobQueue` (crash-safe, model-backed); retry with exponential backoff — `jobs`
- **File / object storage** — `Storage` trait; `LocalStorage` (disk) or `S3Storage` (AWS S3, R2, MinIO via AWS SigV4, no SDK) — `storage-local` / `storage-s3`
- **OpenAPI / Swagger** — `.openapi(OpenApiConfig)` generates `GET /openapi.json` + `GET /docs` (Swagger UI) from registered routes — `openapi`
- **Per-route timeouts** — `with_timeout`/`with_timeout_state`/`with_timeout_async` wrap a handler with its own deadline; `TimeoutLayer` for a whole app; config-driven proxy's `timeout_ms`
- **Request ID middleware** — `RequestIdLayer` injects/echoes `X-Request-Id` on every request and response; `RequestId` extractor to read it

---

## Proxy & Gateway

- **`ReverseProxy`** — HTTP/1.1 reverse proxy middleware; round-robin backend selection
- **`H2ReverseProxy`** — HTTP/2 upstream proxy using `tokio::task::block_in_place` to bridge sync middleware — `http2`
- **`GrpcProxy`** — wraps `H2ReverseProxy`; filters on `Content-Type: application/grpc*` — `http2`
- **`TcpProxy`** — standalone L4 TCP proxy; bidirectional `std::io::copy` relay; round-robin backends
- **`UdpProxy`** — standalone UDP datagram proxy; per-datagram ephemeral socket; round-robin backends
- **`WsProxy`** — standalone WebSocket proxy; relays raw bytes after upgrade handshake
- **Config-driven proxy mode** — `rws.config.toml` with `[[upstream]]` + `[[route]]` sections; no code required
- **Health checking** — per-upstream background health checker; dead backends removed automatically; configurable thresholds and intervals
- **Load balancing** — atomic round-robin across live backends
- **`RewriteLayer`** — rewrite request headers, URI, and response headers / body / status code per route
- **Per-route rate limiting** — `PerRouteRateLimit` in config-driven mode; `RateLimitLayer` in code
- **Per-route Bearer auth** — `BearerAuthMiddleware` in config-driven mode; `JwtLayer` in code
- **Route matching** — host, path prefix, exact path, HTTP method, `Content-Type` prefix; first-match wins

---

## Security

- **`RateLimitLayer`** — sliding-window rate limiter keyed by client IP; global singleton via `rate_limit::global()`; returns `429` when budget exceeded
- **`BasicAuthLayer`** — HTTP Basic authentication middleware
- **`JwtLayer`** — JWT Bearer token validation middleware — `auth`
- **`IpFilter`** — allowlist / denylist middleware keyed by client IP
- **`crypto`** — Argon2id password hashing and verification — `crypto`
- **CSRF tokens** — secure random token generation and validation — `csrf`
- **SSO / OAuth 2.0 / OIDC** — RSA and ECDSA signing, outbound HTTPS token exchange — `sso`
- **CORS** — built-in CORS controller; configurable origins, methods, and headers; hot-reloadable
- **mTLS** — mutual TLS client certificate verification — `http2`
- **No OpenSSL** — TLS via `rustls` + `aws-lc-rs`; fully static binaries

---

## Observability & Ops

- **`/healthz`** — liveness probe; always `200 OK` while the server is up
- **`/readyz`** — readiness probe; returns `503` after graceful shutdown begins (`SERVER_READY` flag)
- **`/metrics`** — Prometheus text-format scrape endpoint
- **`MetricsLayer`** — per-route `rws_route_requests_total{method,path,status}` counters and `rws_route_duration_seconds{method,path}` histograms
- **Global metrics** — `record_request()`, `record_error()`, `connection_open()` / `connection_close()` counters
- **`OtelLayer`** — OpenTelemetry trace propagation middleware
- **`CacheLayer`** — response caching middleware
- **Structured logging** — `RWS_CONFIG_LOG_FORMAT=combined` (default) or `json`; hot-reloadable
- **Config hot reload** — `SIGHUP` or `POST /admin/config/reload` reloads CORS, rate limits, log format, TLS certs; no restart required
- **Kubernetes-ready** — `/healthz`, `/readyz`, `/metrics` all built in; graceful `SIGTERM` drain; static binary with no system dependencies

---

## AI & MCP

- **`McpServer`** — implements `Application`; serves the MCP Streamable HTTP protocol (`POST /mcp`, JSON-RPC 2.0)
- **Tool registration** — `.tool(name, description, schema, handler)` — any function returning a string
- **Resource registration** — `.resource(uri_template, name, description, handler)`
- **Prompt registration** — `.prompt(name, description, handler)`
- **Bearer auth gate** — `.require_bearer(token)` gates all MCP requests behind a static token
- **Fallthrough** — `.wrap(app)` passes non-MCP requests to another `Application`
- **Built-in rws tools** — `server_config`, `feature_flags`, `server_metrics`, `rate_limit_config`, `check_rate_limit`, `cors_config`, `list_static_files`, `reload_config`
- **SSE streaming** — `Sse` builder for streaming AI tokens to browser clients

---

## Database / ORM

All ORM features require exactly one of `model-sqlite`, `model-postgres`, or `model-mysql`.

- **`#[derive(Model)]`** — generates `impl Model`, `Struct::repository()`, `Struct::query()` — `macros`
- **Table and column mapping** — `#[table(name = "…")]`, `#[column(name = "…")]`, `#[primary_key]`, `#[primary_key(auto_increment)]`, `#[ignore]`
- **`DbConnection`** — `execute`, `query_rows`, `begin`, `commit`, `rollback`, `transaction(closure)`, `query::<T>`, `migrate`
- **`DbPool`** — pre-created connection pool; `Mutex<Vec<DbConnection>>`; `PooledConnection` returned to pool on `Drop`
- **`Repository<T, ID>`** — `save` (INSERT or UPDATE by primary key), `find_by_id`, `find_all`, `delete`
- **`QueryBuilder`** — fluent `where_eq`, `fetch_all`, `fetch_one`, `count`, `delete`, `update`; placeholder tokens auto-converted to `?` (SQLite/MySQL) or `$N` (PostgreSQL)
- **Migrations** — reads `*.sql` files in lexicographic order; `_schema_migrations` table tracks applied versions; each migration wrapped in a transaction
- **Relations** — `HasMany<T>`, `HasOne<O>`, `BelongsTo<O>` with explicit `.load(&mut conn)`; no lazy loading, no hidden N+1 queries
- **SQLite** — bundled `libsqlite3`; no system dependency — `model-sqlite`
- **PostgreSQL** — `postgres` client — `model-postgres`
- **MySQL / MariaDB** — `mysql` client — `model-mysql`

---

## Dependency Injection

- **`Container`** — `TypeId`-keyed service store for concrete types and `dyn Trait` objects
- **`register::<T>(value)`** — wraps in `Arc<T>`, keyed by `TypeId::of::<T>()`
- **`provide::<T: ?Sized>(Arc<T>)`** — stores trait objects directly
- **Named services** — `get_named::<T>(name)` / `register_named::<T>(name, value)` for multiple instances of the same type
- **`App::with_state(container)`** — pass the container directly as state; it's `Send + Sync + 'static` like any other state type, no wrapping needed
- **`into_arc()`** — for sharing one container across multiple hand-built `Application`s outside of `with_state`
- No external dependencies
