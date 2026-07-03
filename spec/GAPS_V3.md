[Read Me](../README.md) > [Spec](.) > Gaps V3

# Gaps V3 — Server · Proxy · Framework

`GAPS_V2.md` tracked what was missing for a self-sufficient application framework. All four critical gaps from that document are now closed: outbound HTTP client ✅, password hashing ✅, CSRF ✅, OAuth2/OIDC SSO ✅. This document audits the current state of v17.43.0 against production expectations across all three axes: **server**, **proxy/gateway**, and **framework/library**.

---

## Part 1 — Server

### 1.1 No upstream connection pooling

Every forwarded request (`ReverseProxy`, `DynamicProxy`, `H2ReverseProxy`, `CanaryLayer`) opens a fresh TCP connection to the backend, waits for a response, and closes it. High-throughput proxying suffers badly from TCP handshake overhead and port-exhaustion under load.

**What is missing:** a per-backend connection pool (`src/proxy/pool.rs`) that keeps `Keep-Alive` connections open and reuses them. Relevant standard: RFC 7230 §6.3.

---

### 1.2 Response not streamed through proxy

`read_response_from()` buffers the entire backend response into a `Vec<u8>` before the client receives any bytes. This breaks:
- **Server-Sent Events through a proxy** — the client never sees events until the upstream closes.
- **Large file downloads** — the full body is held in RAM.
- **Chunked streaming APIs** (OpenAI token stream, etc.) — latency is doubled.

**What is missing:** pass-through streaming where the response body is forwarded chunk-by-chunk as it arrives from the upstream.

---

### 1.3 Request body fully buffered

`Request::parse()` reads the entire request body into `request.body: Vec<u8>` before the handler sees anything. Multipart file uploads, large JSON payloads, and chunked request bodies must fit in memory.

**What is missing:** a streaming `BodyReader` or async iterator interface that hands body chunks to the handler as they arrive from the socket, with an opt-in size limit per route.

---

### 1.4 No `100 Continue` support

HTTP/1.1 clients may send `Expect: 100-continue` before sending a large body. The server must reply `100 Continue` before the client sends body bytes, or the client waits for a configurable timeout before sending anyway. rws never sends `100 Continue` — it reads the full body unconditionally. This wastes bandwidth if the server would reject the request (e.g. wrong Content-Type, auth failure).

---

### 1.5 No per-route timeout

The read timeout is set globally (`RWS_CONFIG_*` or hardcoded 30 s). All routes share the same value. A file-upload endpoint may legitimately need 120 s while a health-check must complete in 500 ms. There is no way to express this today without custom middleware.

---

### 1.6 No request ID generation

Distributed tracing (`OtelLayer`) generates a span but does not automatically inject `X-Request-Id` or `X-Correlation-Id` into every request/response pair. Correlating log lines from multiple services requires the caller to set the header, and the server does not propagate it unless `RewriteLayer` is configured manually.

---

### 1.7 Access log rotation

Logs go to stdout. Running `rws` as a long-running server without a log rotation tool (`logrotate`, `systemd` journal, Fluentd) means stdout grows unbounded or log files fill the disk. No built-in `SIGHUP`-triggered log rotation is provided. (Already in the docs roadmap.)

---

### 1.8 WebSocket: no permessage-deflate compression

The WebSocket implementation (RFC 6455) does not negotiate `Sec-WebSocket-Extensions: permessage-deflate` (RFC 7692). Text-heavy WebSocket traffic (chat, JSON events) can be 3–10× larger than necessary.

---

### 1.9 WebSocket over HTTP/2

RFC 8441 (Bootstrapping WebSockets with HTTP/2) is not implemented. Clients connected over HTTP/2 must downgrade to HTTP/1.1 for WebSocket. In practice this means TLS connections that upgraded to HTTP/2 via ALPN cannot use WebSocket without a round-trip protocol renegotiation.

---

### 1.10 No HTTP/2 or HTTP/3 server push

`Server::run_tls` and `Server::run_quic` do not expose a server-push API. Handlers have no way to pre-push CSS/JS resources alongside an HTML response. This is a minor gap (push is rarely used in practice due to cache interaction) but noted for completeness.

---

## Part 2 — Proxy / Gateway

### 2.1 Round-robin only — no other load balancing strategies

`ConfigDrivenApp`, `ReverseProxy`, `CanaryLayer`, and `BackendPool` all use atomic round-robin. Production gateways commonly need:
- **Least connections** — route to the backend with the fewest in-flight requests.
- **IP hash** — sticky sessions without a cookie (deterministic per source IP).
- **Random** — simpler than round-robin for very-low-traffic upstreams.
- **Weighted round-robin** — backends with different capacities.

The `CanaryLayer` already has weighted distribution but it is only for A/B traffic splitting, not general load balancing. (Already in the docs roadmap.)

---

### 2.2 No TLS to H2 upstreams

`H2ReverseProxy` connects to backends using `tokio::net::TcpStream` with no TLS wrapper. The only way to reach a TLS-only HTTP/2 upstream is to terminate TLS at the backend's own load balancer. Config-driven proxy supports `https://` for HTTP/1.1 upstreams (implemented in v17.43.0) but `H2ReverseProxy` is separate and does not.

---

### 2.3 No TLS to gRPC upstreams (`grpcs://`)

`GrpcProxy` wraps `H2ReverseProxy` and inherits the same plain-TCP limitation. Connecting to a gRPC service that requires TLS (the common production case) is not supported.

---

### 2.4 WsProxy does not support `wss://` backends

`WsProxy` opens a plain TCP connection to the backend. There is no path to proxy WebSocket traffic to a backend that requires TLS (`wss://`).

---

### 2.5 CanaryLayer backends are plain HTTP only

`CanaryLayer` calls `proxy::proxy_http1()` for every backend. It has no `tls: bool` flag or `https://` scheme detection. A canary rollout between two HTTPS-only upstream services is not possible via this layer.

---

### 2.6 No connection pooling to upstreams

Every forwarded request from `ReverseProxy`, `H2ReverseProxy`, `DynamicProxy`, or `CanaryLayer` opens a new TCP connection, performs the full handshake (or TLS handshake), and closes after one response. Under load this is the dominant latency contributor and a source of port exhaustion. See also §1.1.

---

### 2.7 No no-code auth from `rws.config.toml`

> **Status: resolved.** `type = "jwt"` (`secret_env`) and `type = "basic"` (`htpasswd_file`) now wire into `JwtLayer`/`BasicAuthLayer` via `apply_middleware()` in `builder.rs`, gated on the `auth` feature. `BasicAuthLayer::from_htpasswd_file` supports plain-text and rws's own `{SHA256}` scheme — not Apache's real `{SHA}`/`$apr1$`/bcrypt, a deliberate scope boundary (see `src/auth/mod.rs` and `docs/features/auth.md`). See `spec/TODO.md`'s entry for full detail.

`[[route]]` supports `[route.middleware] auth = { type = "bearer", token_env = "API_TOKEN" }` for static bearer tokens, but JWT verification and HTTP Basic auth must be added in Rust code. `JwtLayer` and `BasicAuthLayer` have no `rws.config.toml` equivalent. (Already in the docs roadmap.)

---

### 2.8 Static site action not available in config mode

The `type = "static"` action is documented in `ActionConfig` as a planned variant but the builder falls through to `App::new()` without applying a root directory. Serving a directory of files from the config-driven proxy mode requires writing Rust code. (Already in the docs roadmap.)

---

### 2.9 ForwardAuth middleware missing

Delegate authentication decisions to an external HTTP service (like Traefik's `forwardAuth`). The middleware calls the auth service with a copy of the incoming headers; a non-2xx response rejects the request. This is the standard pattern for integrating a policy engine (OPA, Casbin) or a centralized auth service without embedding the logic in rws. (Already in the docs roadmap.)

---

### 2.10 Regex URI rewriting

`RewriteLayer` supports prefix-strip, prefix-add, and exact URI replacement, but not regex pattern matching with capture groups. URL rewriting rules like `^/api/v1/(.*)$` → `/v2/$1` must be expressed as code-level transformations. (Already in the docs roadmap.)

---

### 2.11 Rate limiter and circuit breaker are single-process only

`RateLimiter` is backed by a `Mutex<HashMap<String, Window>>` in process memory. `CircuitBreaker` tracks failure counts in `AtomicUsize` fields. Running two rws instances behind a load balancer means each instance has independent state: the rate limit doubles effectively, and a circuit that opens on one instance stays closed on the other. For multi-instance deployments, these must be backed by a shared store (Redis, Valkey, a shared SQLite file, or a coordination sidecar).

---

### 2.12 `H2ReverseProxy` requires `block_in_place`

The `H2ReverseProxy::handle()` middleware is synchronous (`fn handle(&self, ...) -> Result<Response, String>`) but internally calls async H2 code via `tokio::task::block_in_place`. This works only on the multi-thread tokio scheduler — it panics on the `current_thread` runtime. It also blocks a worker thread for the duration of the upstream call, reducing throughput under high concurrency. An `async`-native proxy middleware is the correct long-term approach.

---

### 2.13 No proxy request/response body size limits

The config-driven proxy accepts and forwards request bodies of any size without a configurable maximum. A client can send an unbounded POST to a proxied route, consuming all available memory. There is no `max_body_size` field in `MatchConfig` or `MiddlewareConfig`.

---

### 2.14 No circuit breaker persistence

`CircuitBreaker` state (failure count, open/half-open) is in `AtomicUsize` fields. A process restart resets all circuit state. A backend that triggered the circuit just before a deploy appears healthy immediately after startup and may cascade failures again before the health checker has time to mark it down.

---

## Part 3 — Framework / Library

### 3.1 Email (SMTP) — still open from GAPS_V2

Password reset, email verification, and transactional notifications require sending email. No built-in path exists; users must add `lettre` and wire it themselves.

**What is needed:** `src/mailer/mod.rs` (`mailer` feature) — thin SMTP client reading `RWS_SMTP_HOST/PORT/USER/PASSWORD/FROM`. Internally uses the outbound HTTP client or raw TCP; no new public-API dependency.

---

### 3.2 File/object storage abstraction — still open from GAPS_V2

`FormMultipartData::parse()` returns raw bytes. There is no abstraction for storing uploaded files, no local-disk helper, and no S3/R2/GCS integration.

**What is needed:** `src/storage/mod.rs` — a `Storage` trait with `LocalStorage` (no new deps) and `S3Storage` (uses the outbound HTTP client to sign PutObject/GetObject via AWS Signature V4; no AWS SDK).

---

### 3.3 Background job queue — still open from GAPS_V2

`Scheduler` runs periodic tasks. There is no in-process queue for ad-hoc one-shot jobs enqueued from handlers ("send this email after registration", "process this image in the background"). No retry-with-backoff, no dead-letter queue, no visibility into pending jobs.

---

### 3.4 OpenAPI / Swagger — still open from GAPS_V2

No automatic API schema generation from `AppWithState` route registrations. API consumers must read source code or maintain separate spec files. `#[derive(Validate)]` and handler route macros already capture the information needed; a `GET /openapi.json` generator would require no new external dependencies.

---

### 3.5 Sessions are not persistent

`SessionStore` is an `Arc<RwLock<HashMap<String, Session>>>`. All sessions are lost on process restart. This is incompatible with zero-downtime deploys, horizontal scaling, and crash recovery.

**What is needed:**
- A `PersistentSessionStore` backed by the model layer (SQLite/Postgres `rws_sessions` table).
- A `RedisSessionStore` for shared state across multiple instances.

---

### 3.6 Rate limiter is not distributed

See §2.11. For library users building multi-instance deployments, `RateLimiter::new()` only protects the local process. A `RedisRateLimiter` or `SqliteRateLimiter` backed by atomic operations in shared storage is needed for true per-user or per-IP global rate enforcement.

---

### 3.7 ORM is synchronous only

The model layer (`src/model/`) uses `std::sync::Mutex`-guarded connections and blocking I/O. There is no `async fn save`, `async fn find_all`, or async `Repository` trait. In `AsyncAppWithState` handlers, calling any ORM method blocks the tokio worker thread for the duration of the database call.

**What is needed:** an async model layer variant using `tokio::task::spawn_blocking` or `sqlx` as a backend, with the same derive-based API.

---

### 3.8 Only one database backend per compilation unit

`model-sqlite`, `model-postgres`, and `model-mysql` are mutually exclusive through `#[cfg]` gates on the `DbConnection` fields. A single binary cannot hold a pool to SQLite for hot data and Postgres for analytics. Multi-tenancy patterns that isolate tenants in separate databases of the same type work, but cross-database-type queries do not.

---

### 3.9 No migration rollback

`conn.migrate("migrations/")` applies unapplied `*.sql` files in order. There is no `down` migration support: no `_down.sql` convention, no `conn.rollback_to_version(n)`. Rolling back a failed deploy means manual SQL.

---

### 3.10 `JwtLayer` is HS256 only

The programmatic `JwtLayer` middleware (in `src/auth/`) only verifies HS256 (HMAC-SHA256) Bearer tokens. RS256/ES256 asymmetric tokens are supported only through the `sso` feature's `JwksCache`. Building a service-to-service auth pattern where the caller presents an RS256 JWT (issued by a separate auth server) requires importing the `sso` feature even when the full OIDC flow is not needed.

---

### 3.11 Cookie signing/encryption absent

`SetCookie` builder and `session_cookie()` helper produce plain-text cookie values. There is no `signed_cookie(value, secret)` (HMAC-SHA256 to prevent tampering) or `encrypted_cookie(value, key)` (AES-GCM to prevent reading). Applications storing anything sensitive in a cookie must implement their own signing.

---

### 3.12 Tera templates are not hot-reloadable

`template::init("templates/")` loads and compiles all templates once at startup. Changing a template during development requires a process restart. The `SIGHUP`/`POST /admin/config/reload` hot-reload path does not re-read the template directory.

---

### 3.13 No built-in pagination helpers

`QueryBuilder` provides `.limit().offset()` but there is no `Page<T>` response type, no `Link: <url>; rel="next"` header builder, no cursor-based pagination helper, and no standard way to express page size from query params. Every endpoint that returns a list must re-implement pagination.

---

### 3.14 No webhook signature verification

Receiving webhooks from Stripe, GitHub, Shopify, and other services requires verifying an HMAC signature on the request body. There is no `WebhookVerifier` or `verify_webhook_signature(body, secret, header)` helper. Users add `hmac` + `sha2` and wire it manually even though those crates are already in the dependency tree.

---

### 3.15 Multi-span distributed tracing

`OtelLayer` creates one root span per HTTP request. There is no API for creating child spans within a handler (e.g. "db.query", "http.outbound", "cache.lookup"). Traces in Jaeger/Tempo show a single flat span with no internal structure. (Already in the docs roadmap.)

---

### 3.16 No i18n / localization helpers

No string translation framework. Apps targeting multiple locales must integrate `fluent`, `gettext`, or a hand-rolled lookup table. The `Accept-Language` header is already parsed (`src/language/mod.rs`) but there is no resolver that maps it to a locale and loads translated strings.

---

### 3.17 Admin UI missing

`GET /admin` showing live metrics, current config, active rate-limiter windows, circuit-breaker states, and a log tail is not implemented. The `/metrics` Prometheus endpoint provides counters but no human-readable dashboard without Grafana. (Already in the docs roadmap.)

---

### 3.18 GraphQL

No integration with `async-graphql` or `juniper`. GraphQL queries must be served by a hand-rolled JSON handler. The REST story is complete; this is a non-blocker for most users.

---

### 3.19 WebAssembly / `wasm32-wasi` target

The binary uses OS threads, `std::net::TcpStream`, and `aws-lc-rs` — none of which compile to WASM today. Running rws handlers inside a WASM sandbox (Wasmtime, WasmEdge, Fastly Compute) would require a thin `wasm32-wasi` shim layer. (Already in the docs roadmap.)

---

## Priority table

| # | Gap | Area | Priority | Effort | New deps? |
|---|-----|------|----------|--------|-----------|
| 1.1 / 2.6 | Upstream connection pooling | Server + Proxy | **High** | Medium | No |
| 1.2 | Streaming response passthrough | Server | **High** | Large | No |
| 2.2 | TLS to H2 upstreams | Proxy | **High** | Small | No (rustls already present) |
| 2.3 | TLS to gRPC upstreams (`grpcs://`) | Proxy | **High** | Small | No |
| 3.1 | Email / SMTP | Framework | **High** | Small | `lettre` or scratch |
| 3.5 | Persistent sessions | Framework | **High** | Medium | No (reuse model layer) |
| 3.7 | Async ORM | Framework | **High** | Large | `sqlx` or `spawn_blocking` |
| 2.1 | Additional LB strategies | Proxy | Medium | Small | No |
| 2.4 | WsProxy `wss://` backends | Proxy | Medium | Small | No (rustls present) |
| 2.5 | CanaryLayer TLS backends | Proxy | Medium | Small | No |
| 2.9 | ForwardAuth middleware | Proxy | Medium | Small | No |
| 2.11 | Distributed rate limiter + circuit breaker | Proxy + Framework | Medium | Medium | Redis client or model layer |
| 2.12 | Async H2ReverseProxy (no `block_in_place`) | Proxy | Medium | Medium | No |
| 3.2 | File / object storage (S3) | Framework | Medium | Medium | No (outbound HTTP client) |
| 3.3 | Background job queue | Framework | Medium | Medium | No |
| 3.4 | OpenAPI / Swagger | Framework | Medium | Medium | No |
| 3.9 | Migration rollback | Framework | Medium | Small | No |
| 3.10 | RS256/ES256 in `JwtLayer` (without sso feature) | Framework | Medium | Small | Already in dep tree |
| 3.11 | Cookie signing / encryption | Framework | Medium | Small | No |
| 1.3 | Streaming request body | Server | Medium | Large | No |
| 1.5 | Per-route timeout | Server | Medium | Small | No |
| 1.6 | Request ID generation | Server | Medium | Small | No |
| 2.7 | JWT/Basic auth from config | Proxy | Medium | Small | No |
| 2.8 | Static site action in config | Proxy | Medium | Small | No |
| 3.12 | Tera template hot-reload | Framework | Low | Small | No |
| 3.13 | Pagination helpers | Framework | Low | Small | No |
| 3.14 | Webhook signature verification | Framework | Low | Small | No |
| 3.15 | Multi-span tracing | Framework | Low | Medium | No |
| 3.16 | i18n | Framework | Low | Medium | `fluent` or scratch |
| 3.17 | Admin UI | Framework | Low | Medium | No |
| 1.4 | `100 Continue` | Server | Low | Small | No |
| 1.7 | Access log rotation | Server | Low | Small | No |
| 1.8 | WebSocket permessage-deflate | Server | Low | Medium | `flate2` (already in tree) |
| 1.9 | WebSocket over HTTP/2 (RFC 8441) | Server | Low | Large | No |
| 2.13 | Proxy max body size | Proxy | Low | Small | No |
| 2.14 | Circuit breaker persistence | Proxy | Low | Small | No |
| 3.6 | Distributed rate limiter (library) | Framework | Low | Medium | Redis client |
| 3.8 | Multiple DB backends at once | Framework | Low | Large | No |
| 3.18 | GraphQL | Framework | Low | Large | `async-graphql` |
| 3.19 | WASM target | Framework | Low | Very large | Shim layer |

---

## Shortest path to closing the most impactful gaps

1. **Upstream connection pooling** — one `HashMap<String, Pool<TcpStream>>` shared by all proxy variants. Closes §1.1, §2.6, and dramatically improves proxy throughput.
2. **TLS to H2 / gRPC / WS upstreams** — three small changes reusing the rustls code from §2.2 (just implemented for HTTP/1.1 upstreams). Closes §2.2, §2.3, §2.4, §2.5.
3. **Persistent sessions** — `PersistentSessionStore` backed by the model layer. Closes §3.5 and unblocks multi-instance deployment.
4. **Email** — `src/mailer/mod.rs` with `lettre`. Closes §3.1.
5. **Streaming response passthrough** — largest remaining server gap; required for SSE-over-proxy and large downloads. Closes §1.2.
