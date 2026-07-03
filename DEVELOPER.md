[Read Me](README.md) > Developer

# Developer Info

Make sure you have [Rust](https://www.rust-lang.org/tools/install) 1.75 or later installed.

```bash
rustup update
```

## Run

```bash
cargo run
```

Starts with HTTP/3, HTTP/2, and TLS compiled in (the default). Without a certificate configured it falls back to plain HTTP/1.1 automatically.

To run with HTTPS + HTTP/2 + HTTP/3 active:
```bash
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem
```

To run the HTTP/1.1-only build (no TLS):
```bash
cargo run --no-default-features --features http1
```

## Test

```bash
cargo test
```

Run a single test:
```bash
cargo test --package rust-web-server --bin rws client_hint::tests::client_hints_header -- --exact
```

## Build

Default (HTTP/3 + HTTP/2 + TLS):
```bash
cargo build --release
```

HTTP/2 + TLS only (no QUIC):
```bash
cargo build --release --no-default-features --features http2
```

HTTP/1.1 only (no TLS, smallest binary):
```bash
cargo build --release --no-default-features --features http1
```

## Release

Open [RELEASE](RELEASE.md) for details.

---

## Building Blocks

The crate exposes its core types so you can compose them in your own server or tooling without pulling in a framework. The key types are:

| Type | Module | Purpose |
|------|--------|---------|
| `Controller` trait | `controller` | Match a request and produce a response |
| `Application` trait | `application` | Wire controllers into a dispatch loop |
| `Server` | `server` | Bind, accept, and process TCP connections |
| `Request` | `request` | Parsed HTTP request (method, URI, headers, body) |
| `Response` | `response` | HTTP response builder |
| `Header` | `header` | Header constants and standard response header set |
| `Range` / `ContentRange` | `range` | Body construction (bytes, files, multipart) |
| `MimeType` | `mime_type` | Content-type detection from file extension |
| `STATUS_CODE_REASON_PHRASE` | `response` | All HTTP status codes as typed constants |
| `FormMultipartData` | `body::multipart_form_data` | Parse `multipart/form-data` uploads |
| `FormUrlEncoded` | `body::form_urlencoded` | Parse `application/x-www-form-urlencoded` bodies |
| `URL` | `url` | Parse and build URLs; percent-encode/decode |
| `Log` | `log` | Combined Log Format access log lines |
| `CookieJar` | `cookie` | Parse the `Cookie` request header into individual cookies |
| `SetCookie` | `cookie` | Build `Set-Cookie` response header values with all RFC 6265 attributes |
| `Router` / `PathParams` | `router` | Standalone dynamic router with `:param` and `*wildcard` path matching; `.with_host(name)` restricts a router to one virtual host. `App`'s own built-in controllers deliberately don't use `Router` — that set is small, static, and known at compile time, so a fixed if-chain is simpler than a segment matcher there. `Router` is for user-defined routes; `AppWithState`/`AsyncAppWithState` build on it and fall through to `App`'s controller chain for anything unmatched. |
| `with_timeout` / `with_timeout_state` / `with_timeout_async` / `TimeoutLayer` | `timeout` | Per-route request timeouts. `with_timeout`/`with_timeout_state` wrap a `Router`/`AppWithState` handler to return `504 Gateway Timeout` if it doesn't finish in time (runs on a background thread — bounds the client's wait, can't truly cancel sync work). `with_timeout_async` (requires `http2`) wraps an `AsyncAppWithState` handler with genuine cancellation via `tokio::time::timeout`. `TimeoutLayer::new`/`::from_arc` wraps a whole `Application` (used by the config-driven proxy's per-route `timeout_ms`). |
| `VirtualHostConfig` | `virtual_host` | Per-domain cert configuration `{ domain, cert_file, key_file }` for multi-domain SNI routing |
| `IntoResponse` / `AppError` | `error` | Typed errors that map to HTTP status codes |
| `TestClient` | `test_client` | In-process HTTP test client — no TCP socket required |
| `FromRequest` / `Body` / `BodyText` / `Query` / `RequestHeaders` / `RequestId` | `extract` | Typed request extractors — parse body, query params, headers, or the request-id header, returning a ready error on failure (`RequestId` never fails; empty string if unset) |
| `RateLimiter` | `rate_limit` | Per-IP sliding-window rate limiter; `global()` reads config from env vars |
| `WebSocket` / `Frame` | `websocket` | RFC 6455 WebSocket handshake, frame read/write; SHA-1 and base64 built in |
| `AppWithState<S>` | `state` | State-aware application with built-in dynamic routing; state shared via `Arc<S>`. Use `App::with_state(S)` as the entry point. `.with_config(ServerConfig)` pins the fallback `App` (for requests none of this app's routes match) to explicit CORS/CSP settings instead of reading `RWS_CONFIG_*` env vars per request — mirrors `App::with_config`. |
| `Middleware` / `WithMiddleware` | `middleware` | Composable middleware pipeline wrapping any `Application`. Use `App::new().wrap(layer)` or `AppWithState::wrap(layer)`. |
| `RateLimitLayer` | `middleware` | Built-in middleware that enforces the global rate limiter per client IP |
| `AsyncAppWithState<S>` | `async_state` | Like `AppWithState<S>` but handlers are `async fn`; requires `http2` feature. Entry point: `App::with_async_state(S)`. `.with_config(ServerConfig)` pins the fallback `App`, same as `AppWithState::with_config`. |
| `Sse` / `SseEvent` | `sse` | Build a buffered `text/event-stream` response from a sequence of events. Correct headers set automatically. |
| `SessionStore` / `Session` | `session` | Thread-safe in-memory session store with TTL expiry. Cookie helpers: `session_id_from_request`, `session_cookie`, `destroy_cookie`. |
| `DbSessionStore` | `session` (requires `model-sqlite`, `model-postgres`, or `model-mysql`) | Persistent session store backed by the model-layer `DbPool`. Created with `DbSessionStore::new(pool, ttl_secs).await`. Auto-creates `rws_sessions` table. All methods are `async fn` returning `Result`. Survives restarts and shared across multiple instances. |
| `RedisSessionStore` | `session` | Persistent session store backed by a Redis server via a hand-rolled RESP v2 client. `RedisSessionStore::new(addr, password, ttl)` or `from_env()` (reads `RWS_REDIS_HOST/PORT/PASSWORD/TTL_SECS`). Sessions expire automatically via Redis TTL. |
| `Json<T>` | `json` | Serde-backed JSON extractor (`from_request`) and responder (`into_response`). Requires `features = ["serde"]`. |
| `BasicAuthLayer<F>` | `auth` | HTTP Basic Auth middleware; validates `Authorization: Basic` credentials via a closure. Requires `features = ["auth"]`. |
| `JwtLayer` | `auth` | JWT HS256 middleware; verifies `Authorization: Bearer` tokens with constant-time HMAC-SHA256. Requires `features = ["auth"]`. |
| `build_jwt` / `verify_jwt` / `Claims` | `auth` | Sign and verify HS256 JWTs; `Claims` exposes `sub`, `exp`, and raw JSON payload. |
| `IpFilter` | `ip_filter` | Allow/deny middleware keyed on client IPv4 address or CIDR range. `IpFilter::allow([...])` passes only listed addresses; `IpFilter::deny([...])` blocks them. |
| `RequestIdLayer` / `RequestId` | `request_id` | Correlation-ID middleware. Echoes an incoming `X-Request-Id` unchanged, or generates one (`generate_request_id()`, UUID-v4-shaped, not crypto-random) — either way it's injected into the request (readable by handlers) and set on the response. `.header(name)` overrides the header name. `RequestId` (in `extract`) is a `FromRequest` convenience for reading it. |
| `routes!` | `macros` | Declarative routing macro — builds `AppWithState`, `AsyncAppWithState`, or `Router` from a `METHOD "path" => handler` table. |
| `OpenApiConfig` / `build_spec` | `openapi` (requires `openapi` feature) | OpenAPI 3.0 schema generation. `AppWithState::openapi(config)` / `AsyncAppWithState::openapi(config)` add `GET /openapi.json` (generated spec) and `GET /docs` (Swagger UI via CDN) covering every route registered so far. Scope: paths, methods, path parameters (`:name`/`*name` → `{name}`) — no request/response body schemas, since Rust has no runtime type reflection to extract them from `#[derive(Validate)]`/serde types. |
| `#[route]`, `#[get]`, `#[post]`, … | `macros` (proc-macro) | Attribute macros that annotate handler functions with their HTTP method and path. Requires `features = ["macros"]`. |
| `#[derive(FromRequest)]` | `macros` (proc-macro) | Derive `FromRequest` for a named-field struct; calls `from_request` on each field in declaration order, short-circuiting on the first error. Requires `features = ["macros"]`. |
| `Validate` / `ValidationErrors` | `validate` | Field-level validation trait; `ValidationErrors` collects all failures before returning. Implement manually or derive. |
| `Validated<T>` | `validate` | `FromRequest` wrapper — extracts then validates in one step; `400` on extraction failure, `422 Unprocessable Entity` with JSON error body on validation failure. |
| `is_email` / `is_url` | `validate` | Format check helpers used by the derive macro; callable directly. |
| `#[derive(Validate)]` | `macros` (proc-macro) | Derive `Validate` from `#[validate(...)]` field annotations. Validators: `length(min,max)`, `range(min,max)`, `email`, `required`, `url`. Requires `features = ["macros"]`. |
| `ReverseProxy` | `proxy` | Middleware that forwards requests to HTTP backends with round-robin load balancing and automatic failover. Returns `502` when all backends fail. Reuses idle TCP connections via the built-in `ConnPool`. SSE (`text/event-stream`), chunked AI token streams, and large downloads (`Content-Length > 1 MB`) are forwarded without buffering via `Response::stream_pipe`. |
| `ConnPool` | `proxy` | Per-backend HTTP/1.1 connection pool. `ConnPool::new(max_idle, idle_timeout)` or `ConnPool::new_default()`. Share across proxy instances with `Arc<ConnPool>` via `ReverseProxy::with_pool()`. |
| `RewriteLayer` | `rewrite` | Composable request/response rewriting middleware: request header add/replace/remove, URI set/strip-prefix/add-prefix, response header add/replace/remove, status override, body byte find-and-replace. |
| `LoadBalancing` | `proxy` | Enum selecting the balancing strategy (`RoundRobin`). Passed to `ReverseProxy::strategy()`. |
| `MetricsLayer` | `metrics` | Middleware that records per-route request counts and latency histograms. Adds `rws_route_requests_total{method,path,status}` and `rws_route_duration_seconds{method,path}` to `/metrics`. |
| `CacheLayer` | `cache` | In-memory TTL response cache middleware for GET requests. Builder: `.ttl(secs)`, `.vary_by_header(name)`. Injects `Age` on hits; respects `Cache-Control: no-store/private`. |
| `ServerConfig` | `server_config` | Typed snapshot of all per-instance config (CORS, CSP, log format, request allocation). `ServerConfig::from_env()` reads `RWS_CONFIG_*` vars; `ServerConfig::default()` returns hardcoded defaults. Pass to `App::with_config(config)` to create a fully isolated app instance. |
| `App::with_config` | `app` | Constructor that pins an `App` to a fixed `ServerConfig` — no env reads happen during request processing. Preferred pattern for parallel integration tests that verify CORS/CSP behavior; no `test_env::lock()` needed. |
| `ConfigSnapshot` | `config_reload` | Point-in-time snapshot of all hot-reloadable config values. Read with `config_reload::current()`. |
| `config_reload::reload` | `config_reload` | Re-reads `rws.config.toml` and applies CORS, rate-limit, log-format changes live. Triggered by SIGHUP or `POST /admin/config/reload`. |
| `OtelLayer` | `otel` | Middleware that creates HTTP server spans, propagates W3C `traceparent`, and exports to stdout or OTLP HTTP. Call `otel::setup()` or `otel::setup_from_env()` at startup. |
| `TracingConfig` / `ExporterConfig` | `otel` | Configure the tracing subsystem: service name, exporter backend, sample rate, batch size. |
| `otel::setup` / `otel::setup_from_env` | `otel` | Initialize the global tracer. Call once before the server starts. `setup_from_env` reads `OTEL_SERVICE_NAME`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_TRACES_SAMPLER_ARG`. |
| `otel::shutdown` / `otel::flush` | `otel` | Flush buffered spans to the exporter. Call `shutdown()` before process exit to avoid span loss. |
| `otel::current_traceparent` | `otel` | Returns the W3C `traceparent` for the span active on the current thread. Used by `ReverseProxy` for automatic context propagation. |
| `TraceContext` | `otel` | Parsed W3C `traceparent` value. `TraceContext::parse(header)` / `ctx.as_header(span_id)`. |
| `SpanData` | `otel` | A completed span ready for export. Contains trace/span/parent IDs, timing, HTTP attributes, and OTel status code. |
| `StdoutExporter` / `OtlpHttpExporter` | `otel` | Built-in exporters. `OtlpHttpExporter::new(endpoint, service_name, service_version)` posts OTLP JSON. |
| `H2ReverseProxy` | `proxy` | HTTP/2 upstream reverse proxy middleware. `h2://` plain TCP, `h2s://`/`https://` TLS with ALPN `h2` (port defaults to 443). Requires `http2` feature. |
| `GrpcProxy` | `proxy` | gRPC reverse proxy middleware — filters on `Content-Type: application/grpc*` and forwards over HTTP/2. `grpc://` plain, `grpcs://`/`https://` TLS. Requires `http2` feature. |
| `TcpProxy` | `tcp_proxy` | Standalone L4 TCP proxy. Accepts connections on a local address and relays bytes bidirectionally to round-robin backends. |
| `UdpProxy` | `udp_proxy` | Standalone UDP proxy (request-reply model). Forwards each datagram to a backend and returns the reply to the original client. |
| `WsProxy` | `ws_proxy` | Standalone WebSocket proxy. `ws://` plain TCP (two-thread relay), `wss://` TLS (single-thread polling relay; `http-client` or `http2` feature). Port defaults to 80/443. |
| `WeightedBackend` / `CanaryLayer` | `canary` | Weighted traffic-splitting proxy middleware; each backend has a `weight` — distribution is proportional. Useful for canary releases and A/B testing. |
| `CircuitBreaker` | `circuit_breaker` | Per-backend circuit breaker (Closed→Open→HalfOpen state machine). `global()` returns a process-wide singleton. Configurable failure threshold and recovery window. |
| `RetryLayer` | `circuit_breaker` | Middleware that retries requests on configurable status codes (default: 502, 503, 504) up to `max_retries` times. |
| `BackendPool` / `DiscoverySource` | `service_discovery` | Dynamic backend pool updated by a background thread. Sources: `Static`, `EnvPrefix` (env vars), `File` (one host:port per line), `Dns` (A-record lookup). |
| `IngressRule` / `KubernetesIngressWatcher` / `IngressRouter` | `ingress` | Kubernetes Ingress watcher: polls the K8s API, parses Ingress rules, and routes requests to the correct upstream service. `IngressRouter` implements `Application`. |
| `Scheduler` / `CronSchedule` | `scheduler` | `@Scheduled`-equivalent background task runner. Three modes: `.every(Duration, fn)` (fixed rate), `.after(Duration, fn)` (fixed delay), `.cron("sec min hour day month weekday", fn)`. Full cron syntax: `*`, exact, `*/step`, `N-M`, comma list. |
| `Job` / `JobQueue` | `jobs` (requires `jobs` feature) | In-memory background job queue for one-shot, fire-and-forget work from handlers. `Job` trait (blanket-implemented for `Fn() -> Result<(), String> + Send` closures); `JobQueue::new(workers)` spawns a fixed worker pool; `.submit(job)` enqueues; failed jobs retry with exponential backoff (`.max_retries(n)`, `.backoff(initial, multiplier)`); `.join()` drains and waits. Not crash-safe — see `PersistentJobQueue`. |
| `PersistentJobQueue` | `jobs` (requires `jobs` **and** a `model-sqlite`/`model-postgres`/`model-mysql` feature) | Crash-safe job queue backed by the model layer: jobs are written to a `rws_jobs` table before being acknowledged. Jobs are `(job_type, payload)` string pairs dispatched to a handler registered via `.register(job_type, fn)` — not arbitrary closures, since closures can't be persisted. `PersistentJobQueue::new(pool).await` creates the table and resets any row left `running` by a crash back to `pending`. `.enqueue(job_type, payload).await` / `.enqueue_with_retries(...)` persist a job; `.start(workers)` spawns polling worker threads (each with its own Tokio runtime); `.tick().await` runs a single poll-claim-execute cycle for tests or custom loops. |
| `Storage` / `LocalStorage` | `storage` (requires `storage-local` feature) | File storage abstraction. `Storage` trait: `put(key, data, content_type) -> Result<String, StorageError>`, `get(key)`, `delete(key)`, `url(key)` (no I/O). `LocalStorage::new(root)` stores objects as files under `root`; rejects `..` path segments; `.with_base_url(prefix)` makes `url()` return an HTTP path instead of a filesystem path. |
| `S3Storage` / `S3Config` | `storage` (requires `storage-s3` feature) | `Storage` implementation for S3-compatible object storage (AWS S3, Cloudflare R2, MinIO) via the outbound HTTP client — no AWS SDK. Signs every request with AWS Signature Version 4 (`hmac` + `sha2`, already-in-tree crates). `S3Storage::from_env()` reads `RWS_S3_BUCKET/REGION/ACCESS_KEY/SECRET_KEY/ENDPOINT`. Uses path-style addressing (`{endpoint}/{bucket}/{key}`) for compatibility with custom endpoints. |
| `TeraEngine` | `template` (requires `tera` feature) | Jinja2/Django HTML template engine. `from_dir(dir)` loads disk templates; `from_raw(&[(name, src)])` for inline templates. Global singleton via `template::init(dir)` / `template::render(name, &ctx)`. |
| `#[derive(Config)]` / `FromEnvStr` | `config_binding` (requires `macros` feature for derive) | Typed env-var binding. Generates `load() -> Result<Self, String>`. `#[config(env = "KEY", default = "v")]` per field; `Option<T>` for optional; struct-level `#[config(prefix = "APP_")]`. Implement `FromEnvStr` for custom types. |
| `ProxyConfig` / `ConfigDrivenApp` / `build_from_file` | `proxy_config` | Config-driven proxy server. `ProxyConfig::is_proxy_mode()` detects `[[route]]` / `[[upstream]]` sections in `rws.config.toml`; `build_from_file()` returns a `ConfigDrivenApp` (first-match router over `Arc<Vec<CompiledRoute>>`) plus L4/WS proxy thread handles. Per-route middleware: `PerRouteRateLimit`, `BearerAuthMiddleware`, `RewriteLayer`, `CacheLayer`, `IpFilter`. `DynamicProxy` performs health-aware proxying with a per-`[[upstream]]` `strategy`: `round_robin` (default), `random`, `ip_hash` (sticky per client IP), or `least_connections` (routes to the live backend with fewest in-flight requests). `ConfigDrivenApp::with_config(ServerConfig)` pins its fallback `App` (unmatched requests — healthz/readyz/metrics/static/404) to explicit settings instead of reading `RWS_CONFIG_*` env vars per request. |
| `StaticAdapter` | `proxy_config` | Action handler for `type = "static"` routes in `rws.config.toml`. Serves files from a configured `root` directory (independent of the process working directory), trying each `index` entry in order for directory requests; rejects any request path with a `..` segment (before or after percent-decoding) with `403`, missing files with `404`. |
| `Container` | `di` | Type-keyed dependency injection container. `register::<T>(service)` stores concrete types; `provide::<dyn Trait>(Arc::new(...))` stores trait objects; both keyed by `TypeId`. Named services via `register_named` / `provide_named` / `get_named`. Pass the container directly as `AppWithState`/`AsyncAppWithState`'s state (`App::with_state(container)`) — **not** `container.into_arc()`, which double-wraps in `Arc` since `with_state` already wraps `S` internally. `into_arc()` is for sharing one container across multiple hand-built `Application`s outside of `with_state`. |
| `DbPool` / `DbTransaction` | `model` (requires `model-sqlite`, `model-postgres`, or `model-mysql`; implies `http2`) | Async connection pool backed by `sqlx`. `DbPool::new(DbConfig).await` or `DbPool::from_env().await`. All SQL operations are `async fn`: `execute`, `query_rows`, `query::<T>`, `begin`, `transaction(closure)`, `migrate`, `migration_status`. **SQLite in-memory shortcut:** `DbPool::memory().await` creates a single-connection pool backed by `":memory:"` — each call is an isolated empty database, ideal for tests. Cheap to clone (Arc-wrapped). |
| `DbConfig` | `model` | Database configuration. `DbConfig::from_env()` reads `RWS_DB_*` env vars; construct manually with `DbConfig { host, port, user, password, database, pool_size }`. |
| `ModelRepository<T, i64>` | `model` | Async JPA-style CRUD: `find_by_id`, `find_all`, `save` (INSERT when pk==0, UPDATE otherwise), `save_all`, `delete_by_id`, `delete_all_by_id`, `count`, `exists_by_id` — all `async fn`. Obtain via `T::repository(&pool)` when using `#[derive(Model)]`. |
| `QueryBuilder<T>` | `model` | Async fluent SQL builder: `where_eq`, `filter`, `order_by`, `limit`, `offset`, then `fetch_all`, `fetch_one`, `count`, `update`, `delete` (all `.await`). Obtain via `T::query(&pool)` when using `#[derive(Model)]`. |
| `#[derive(Model)]` | `model` (requires `macros` + a model feature) | Proc-macro that maps a struct to a DB table. Attributes: `#[table(name = "…")]` struct-level override; `#[primary_key(auto_increment)]`; `#[column(name = "…")]`; `#[ignore]`. Generates `Model` impl plus `T::repository(&pool)` and `T::query(&pool)` helpers. |
| `HasMany<T>` / `HasOne<T>` / `BelongsTo<O>` | `model` | Async explicit-load relationship helpers. `HasMany::new(owner_pk, fk_col).load(&pool).await` returns `Vec<T>`; no hidden N+1 queries. |
| `Client` / `RequestBuilder` / `Response` | `http_client` | Synchronous outbound HTTP/1.1 client. `Client::new().get(url).header(k,v).timeout_ms(ms).send()` returns `Response`. Follows redirects automatically. Plain HTTP works in all builds; HTTPS requires `http-client` or `http2` feature. |
| `AsyncClient` / `AsyncRequestBuilder` | `http_client` (requires `http2` feature) | Async variant of the outbound client. Same builder API with `.send().await`. |
| `HttpClientError` | `http_client` | Error type returned by the HTTP client; implements `std::error::Error`. |
| `hash_password` / `verify_password` | `crypto` | Argon2id password hashing and verification. `hash_password(pwd)` returns a PHC string (salt embedded). `verify_password(pwd, hash)` is constant-time. |
| `generate_token` | `crypto` | CSPRNG-backed `generate_token(n_bytes) -> String` — lowercase hex, suitable for reset tokens and API keys. |
| `CsrfLayer` | `csrf` | Double-submit cookie CSRF middleware. Validates `X-CSRF-Token` header or `_csrf` form field against the `_csrf` cookie on mutating requests; returns 403 on mismatch. Builder: `.cookie_name()`, `.http_only()`, `.secure()`. |
| `CsrfToken` | `csrf` | Extractor for the current CSRF token. `CsrfToken::from_request(&req)` returns the token inside a GET handler (after `CsrfLayer` has run). Implements `Display` for easy HTML embedding. |
| `OidcAuth` | `sso` | OAuth2/OIDC middleware. Intercepts `/auth/login`, `/auth/callback`, `/auth/logout`; validates sessions; injects `OidcClaims` via `OidcAuth::claims(req)`. Builder: `.exclude(prefix)`, `.login_path()`, `.callback_path()`. |
| `OidcConfig` | `sso` | OAuth2/OIDC configuration. Provider presets: `OidcConfig::google(id, secret, uri)`, `::microsoft(tenant, …)`, `::github(…)`, `::okta(domain, …)`, `::auth0(domain, …)`, `::keycloak(base, realm, …)`, `::from_env()`. |
| `OidcProvider` | `sso` | OIDC provider endpoints (authorization, token, JWKS, userinfo). Presets match `OidcConfig`; `OidcProvider::discover(issuer)` fetches `/.well-known/openid-configuration`. |
| `OidcClaims` | `sso` | Standard OIDC claims: `sub`, `iss`, `aud`, `exp`, `iat`, `nonce`, `email`, `name`, `picture`, etc. Extracted from a verified `id_token` (RS256/ES256) or a UserInfo response. |
| `JwksCache` | `sso` | Thread-safe JWKS key cache. Lazy-fetches and auto-rotates on `kid` miss. `verify_jwt(token, opts)` validates RS256 and ES256 JWTs end-to-end (signature + expiry + aud + iss). |
| `OidcClient` | `sso` | OAuth2 client: `authorization_url(pkce, state, nonce)` builds the IdP redirect URL; `exchange_code(code, verifier)` posts to the token endpoint; `fetch_user_info(token)` calls the UserInfo endpoint. |
| `PkceVerifier` / `PkceChallenge` | `sso` | RFC 7636 PKCE. `PkceVerifier::new()` generates a 32-byte random code verifier; `.challenge()` returns the S256 `PkceChallenge` to include in the authorization URL. |
| `Mailer` | `mailer` (requires `mailer` feature; STARTTLS/SMTPS additionally require `http-client` or `http2`) | SMTP mailer. `Mailer::from_env()` reads `RWS_SMTP_HOST/PORT/USER/PASSWORD/FROM/TLS/TIMEOUT_MS`. `mailer.send(&email)` opens a TCP connection, negotiates TLS (STARTTLS or SMTPS), authenticates with `AUTH PLAIN`, and delivers the message. Three TLS modes: `SmtpTls::None` (plain, port 25), `SmtpTls::Starttls` (default, port 587), `SmtpTls::Smtps` (implicit TLS, port 465). |
| `Email` / `EmailBuilder` | `mailer` | RFC 5322 email builder. `Email::builder().to(addr).subject(s).text(body).html(body).cc(addr).bcc(addr).reply_to(addr).build()`. Validates that at least one `To:` address, a subject, and a body (text or HTML) are provided. Generates `multipart/alternative` when both text and HTML are set. |
| `SmtpTls` | `mailer` | Enum for SMTP TLS mode: `None`, `Starttls`, `Smtps`. |
| `MailerError` | `mailer` | Error type for SMTP failures: `MissingConfig`, `Io`, `Smtp`, `Build`. Implements `std::error::Error`. |

---

## Use Case Scenarios

### 1. Add a custom route

Implement the `Controller` trait and register it in `App::execute`.

```rust
use rust_web_server::controller::Controller;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::ConnectionInfo;

pub struct HelloController;

impl Controller for HelloController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/hello"
    }

    fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        let body = b"Hello, world!".to_vec();
        response.content_range_list = vec![Range::get_content_range(body, MimeType::TEXT_PLAIN.to_string())];
        response
    }
}
```

Then in your `App::execute` before the `NotFoundController` fallthrough:

```rust
if HelloController::is_matching(&request, connection) {
    response = HelloController::process(&request, response, connection);
    return Ok(response);
}
```

---

### 2. Return a JSON response

```rust
fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    let json = r#"{"status":"ok","count":42}"#;
    response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    response.content_range_list = vec![
        Range::get_content_range(json.as_bytes().to_vec(), MimeType::APPLICATION_JSON.to_string())
    ];
    response
}
```

---

### 3. Read a request header

```rust
use rust_web_server::header::Header;

let auth = request.get_header(Header::_AUTHORIZATION.to_string());
match auth {
    Some(h) => println!("Authorization: {}", h.value),
    None    => { /* not present */ }
}
```

---

### 4. Read query parameters from the URI

```rust
use rust_web_server::url::URL;

// request_uri is e.g. "/search?q=rust&page=2"
let full_url = format!("http://localhost{}", request.request_uri);
if let Ok(components) = URL::parse(&full_url) {
    if let Some(query) = components.query {
        let params = URL::parse_query(&query);
        let q = params.get("q").map(String::as_str).unwrap_or("");
        let page = params.get("page").map(String::as_str).unwrap_or("1");
    }
}
```

---

### 5. Parse a URL-encoded POST body

```rust
use rust_web_server::body::form_urlencoded::FormUrlEncoded;

fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    match FormUrlEncoded::parse(request.body.clone()) {
        Ok(fields) => {
            let name = fields.get("name").map(String::as_str).unwrap_or("unknown");
            // ... build response
        }
        Err(e) => {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();
        }
    }
    response
}
```

---

### 6. Parse a multipart file upload

```rust
use rust_web_server::body::multipart_form_data::FormMultipartData;
use rust_web_server::header::Header;

fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    let content_type = request.get_header(Header::_CONTENT_TYPE.to_string());
    if let Some(ct) = content_type {
        // extract boundary from "multipart/form-data; boundary=----WebKitFormBoundaryXYZ"
        if let Some(boundary_part) = ct.value.split("boundary=").nth(1) {
            let boundary = boundary_part.trim().to_string();
            if let Ok(parts) = FormMultipartData::parse(&request.body, boundary) {
                for part in parts {
                    let cd = part.get_header(Header::_CONTENT_DISPOSITION.to_string());
                    // part.body contains the raw bytes of the uploaded file
                }
            }
        }
    }
    response
}
```

---

### 7. Return a redirect

```rust
use rust_web_server::header::Header;

fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    response.status_code = *STATUS_CODE_REASON_PHRASE.n301_moved_permanently.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n301_moved_permanently.reason_phrase.to_string();
    response.headers.push(Header {
        name: Header::_LOCATION.to_string(),
        value: "https://example.com/new-path".to_string(),
    });
    response
}
```

---

### 8. Return a plain error response

```rust
fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    response.status_code = *STATUS_CODE_REASON_PHRASE.n403_forbidden.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n403_forbidden.reason_phrase.to_string();
    response.content_range_list = vec![
        Range::get_content_range(b"Access denied".to_vec(), MimeType::TEXT_PLAIN.to_string())
    ];
    response
}
```

---

### 9. Read client IP and port

`ConnectionInfo` is passed into every controller and carries the peer address. Use `peer_addr()` to get a `std::net::SocketAddr`, or read the raw `client.ip` / `client.port` fields directly.

```rust
fn process(_request: &Request, mut response: Response, connection: &ConnectionInfo) -> Response {
    // Typed SocketAddr (preferred)
    if let Some(addr) = connection.peer_addr() {
        println!("request from {}", addr);  // e.g. "127.0.0.1:54321"
    }
    // Raw string fields (backward-compatible)
    println!("ip={} port={}", connection.client.ip, connection.client.port);
    response
}
```

---

### 10. Detect MIME type from a file path

```rust
use rust_web_server::mime_type::MimeType;

let mime = MimeType::detect_mime_type("/assets/app.wasm");   // "application/wasm"
let mime = MimeType::detect_mime_type("/styles/main.css");   // "text/css"
let mime = MimeType::detect_mime_type("/data/feed.json");    // "application/json"
```

---

### 11. Write a Combined Log Format line

```rust
use rust_web_server::log::Log;

// inside a request handler or middleware
let line = Log::combined(&request, &response, &peer_addr);
println!("{}", line);
// 192.168.1.1 - - [29/Jun/2026:14:23:05 +0000] "GET /index.html HTTP/1.1" 200 1234
```

---

### 12. Start the server with a custom application

```rust
use rust_web_server::server::Server;
use rust_web_server::core::New;

fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }
    let (listener, pool) = new_server.unwrap();
    let app = MyApp::new();          // implements Application trait
    Server::run(listener, pool, app);
}
```

`Server::setup()` reads configuration (IP, port, thread count) from the standard layered config (defaults → env vars → `rws.config.toml` → CLI args). See [CONFIGURE.md](CONFIGURE.md) for all options.

---

### 13. Read and set cookies

`CookieJar` parses the `Cookie` request header. `SetCookie` builds the `Set-Cookie` response header value.

```rust
use rust_web_server::cookie::{CookieJar, SetCookie};
use rust_web_server::header::Header;

fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    // Read cookies from the request
    if let Some(cookie_header) = request.get_header("Cookie".to_string()) {
        let jar = CookieJar::parse(&cookie_header.value);
        if let Some(session) = jar.get("session") {
            println!("session cookie: {}", session.value);
        }
    }

    // Set a cookie in the response
    let set_cookie = SetCookie::new("session", "abc123")
        .path("/")
        .http_only()
        .secure()
        .same_site("Lax")
        .max_age(3600)
        .build();

    response.headers.push(Header {
        name: "Set-Cookie".to_string(),
        value: set_cookie,
    });

    response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    response
}
```

`SetCookie::build()` produces a string like:
```
session=abc123; Path=/; Max-Age=3600; Secure; HttpOnly; SameSite=Lax
```

Pass that string as the value of a `Set-Cookie` header.

---

### 14. Dynamic routing with path parameters

`Router` matches a path pattern against an incoming request and extracts named segments into [`PathParams`]. Use it standalone, or call `Router::handle` from inside your own `Application::execute` to add path-parameter routes alongside the static controller chain.

```rust
use rust_web_server::router::Router;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

let router = Router::new()
    .get("/users/:id", |_req, params, _conn| {
        let id = params.get("id").unwrap_or("unknown");
        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(format!("user {}", id).into_bytes(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    })
    .get("/files/*path", |_req, params, _conn| {
        let path = params.get("path").unwrap_or("");
        // path == "a/b/c.txt" for a request to /files/a/b/c.txt
        Response::new()
    });
```

Wildcard segments (`*name`) must be the last segment in the pattern and capture the remaining path joined with `/`.

---

### 15. Typed error handling

Implement `IntoResponse` on your own error enum, or use the built-in `AppError`:

```rust
use rust_web_server::error::{AppError, IntoResponse};
use rust_web_server::response::Response;

fn find_user(id: u64) -> Result<Response, AppError> {
    if id == 0 {
        return Err(AppError::NotFound("user not found".to_string()));
    }
    // ... build a 200 response
    Ok(Response::new())
}

fn process(request: &Request, _response: Response, _connection: &ConnectionInfo) -> Response {
    find_user(42).unwrap_or_else(|e| e.into_response())
}
```

`AppError` variants map to: `BadRequest` → 400, `Unauthorized` → 401, `Forbidden` → 403, `NotFound` → 404, `Conflict` → 409, `UnprocessableEntity` → 422, `Internal` → 500.

---

### 16. Testing with `TestClient`

`TestClient` dispatches requests directly through an `Application` implementation — no TCP socket, no server process.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::test_client::TestClient;

#[test]
fn healthz_returns_ok() {
    let client = TestClient::new(App::new());
    let res = client.get("/healthz").send();
    assert_eq!(200, res.status());
    assert_eq!("OK", res.body_text());
}

#[test]
fn post_with_headers_and_body() {
    let client = TestClient::new(App::new());
    let res = client.post("/echo")
        .header("Content-Type", "text/plain")
        .body_text("hello")
        .send();
    assert!(res.is_success());
}
```

---

### 17. Typed request extraction

`FromRequest` implementations let you pull typed values out of a request at the start of a handler, returning a ready `Response` on failure rather than writing the error handling inline.

```rust
use rust_web_server::extract::{BodyText, Query, FromRequest};
use rust_web_server::controller::Controller;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::ConnectionInfo;

pub struct EchoController;

impl Controller for EchoController {
    fn is_matching(request: &Request, _: &ConnectionInfo) -> bool {
        request.method == METHOD.post && request.request_uri.starts_with("/echo")
    }

    fn process(request: &Request, mut response: Response, _: &ConnectionInfo) -> Response {
        let text = match BodyText::from_request(request) {
            Ok(t) => t,
            Err(err_response) => return err_response,
        };
        let q = Query::from_request(request).unwrap();
        let prefix = q.get("prefix").map(String::as_str).unwrap_or("");
        let body = format!("{}{}", prefix, text.as_str());

        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    }
}
```

Implement `FromRequest` on your own types for reusable extraction logic:

```rust
use rust_web_server::extract::FromRequest;
use rust_web_server::request::Request;
use rust_web_server::response::Response;

pub struct BearerToken(pub String);

impl FromRequest for BearerToken {
    fn from_request(request: &Request) -> Result<Self, Response> {
        use rust_web_server::error::{AppError, IntoResponse};
        use rust_web_server::header::Header;
        let auth = request.get_header(Header::_AUTHORIZATION.to_string());
        match auth {
            Some(h) if h.value.starts_with("Bearer ") => {
                Ok(BearerToken(h.value[7..].to_string()))
            }
            _ => Err(AppError::Unauthorized.into_response()),
        }
    }
}
```

---

### 18. Per-IP rate limiting

`RateLimiter` enforces a sliding-window request cap per client key (typically the client IP). Use the process-wide `global()` instance (configured via env vars) or create a custom one.

```rust
use rust_web_server::response::Response;

// check on every request, e.g. at the top of Application::execute
fn check_rate_limit(ip: &str) -> Option<Response> {
    use rust_web_server::error::{AppError, IntoResponse};
    let limiter = rust_web_server::rate_limit::global();
    if limiter.check(ip) {
        None  // allowed
    } else {
        Some(AppError::TooManyRequests.into_response())
    }
}
```

Configure limits at startup:

```bash
RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS=200   # requests per window (default 1000)
RWS_CONFIG_RATE_LIMIT_WINDOW_SECS=60    # window length in seconds (default 60)
```

---

### 19. WebSocket connections

`WebSocket` provides RFC 6455 handshake and frame I/O. Because WebSocket requires taking over the raw TCP stream after the `101` response is sent, the connection cannot be handled inside a normal `Controller::process` call (which has no access to the stream). Drive your own accept loop instead:

```rust
use std::net::TcpListener;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::websocket::{WebSocket, Frame};

let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
for stream in listener.incoming() {
    let mut stream = stream.unwrap();

    // Read and parse the HTTP upgrade request (simplified)
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap();
    // ... parse into a Request ...

    if WebSocket::is_upgrade_request(&request) {
        // Send the 101 handshake
        let resp = WebSocket::handshake_response(&request).unwrap();
        stream.write_all(&resp.generate_response()).unwrap();

        // Frame loop
        loop {
            match WebSocket::read_frame(&mut stream) {
                Ok(Frame::Text(msg)) => {
                    WebSocket::send_text(&mut stream, &msg).unwrap();  // echo back
                }
                Ok(Frame::Ping(payload)) => {
                    WebSocket::send_pong(&mut stream, payload).unwrap();
                }
                Ok(Frame::Close(code, reason)) => {
                    WebSocket::send_close(&mut stream, code.unwrap_or(1000), &reason).unwrap();
                    break;
                }
                Ok(Frame::Binary(data)) => {
                    WebSocket::write_frame(&mut stream, Frame::Binary(data)).unwrap();
                }
                _ => break,
            }
        }
    }
}
```

Key primitives:
- `WebSocket::is_upgrade_request(&request)` — checks `Upgrade: websocket`, `Connection: Upgrade`, and `Sec-WebSocket-Key`
- `WebSocket::handshake_response(&request)` — returns a `101 Switching Protocols` `Response` with the correct `Sec-WebSocket-Accept` key
- `WebSocket::read_frame(&mut stream)` → `Frame` — handles client-to-server masking automatically
- `WebSocket::write_frame(&mut stream, frame)` — sends a server-to-client unmasked frame
- `Frame::Text`, `Frame::Binary`, `Frame::Ping`, `Frame::Pong`, `Frame::Close`, `Frame::Continuation`

---

### 20. Shared application state

`AppWithState<S>` wraps any `S: Send + Sync` behind an `Arc` and provides state-aware route registration with full `:param` / `*wildcard` path matching. Unmatched routes fall through to the built-in `App` controller chain.

```rust
use rust_web_server::state::AppWithState;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

struct AppState {
    greeting: String,
    counter: std::sync::atomic::AtomicU64,
}

let app = AppWithState::new(AppState {
    greeting: "Hello".to_string(),
    counter: std::sync::atomic::AtomicU64::new(0),
})
.get("/greet", |_req, _params, _conn, state| {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![
        Range::get_content_range(state.greeting.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())
    ];
    r
})
.get("/users/:id/posts/:post_id", |_req, params, _conn, state| {
    let user_id = params.get("id").unwrap_or("?");
    let post_id = params.get("post_id").unwrap_or("?");
    let count = state.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let body = format!("{} user={} post={} count={}", state.greeting, user_id, post_id, count + 1);
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())];
    r
})
.post("/items", |req, _params, _conn, _state| {
    // process req.body, return 201
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
})
.delete("/items/:id", |_req, params, _conn, _state| {
    let _ = params.get("id");
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
});
```

The state is stored once (not cloned per request). `app.state()` returns `&S` for inspection or testing.

---

### 21. Middleware pipeline

`WithMiddleware<A>` wraps any `Application` with a stack of [`Middleware`] layers. Implement `Middleware::handle` to intercept requests before they reach the inner application. Call `next.execute(request, connection)` to continue the chain, or return early to short-circuit.

```rust
use rust_web_server::middleware::{Middleware, WithMiddleware};
use rust_web_server::application::Application;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::app::App;
use rust_web_server::error::{AppError, IntoResponse};
use rust_web_server::header::Header;
use rust_web_server::core::New;

// ── Logging middleware ────────────────────────────────────────────────────────

pub struct LoggingMiddleware;

impl Middleware for LoggingMiddleware {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        println!("{} {}", request.method, request.request_uri);
        let response = next.execute(request, connection)?;
        println!("  → {}", response.status_code);
        Ok(response)
    }
}

// ── Auth middleware ───────────────────────────────────────────────────────────

pub struct RequireApiKey {
    valid_key: String,
}

impl Middleware for RequireApiKey {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let key = request.headers.iter().find(|h| h.name.to_lowercase() == "x-api-key");
        match key {
            Some(h) if h.value == self.valid_key => next.execute(request, connection),
            _ => Ok(AppError::Unauthorized.into_response()),
        }
    }
}

// ── Wire up with App::wrap (fluent builder) ───────────────────────────────────

let app = App::new()
    .wrap(LoggingMiddleware)
    .wrap(RequireApiKey { valid_key: "secret-key".to_string() });
```

Use the built-in `RateLimitLayer` to enforce the global rate limit (configured via `RWS_CONFIG_RATE_LIMIT_*` env vars):

```rust
use rust_web_server::app::App;
use rust_web_server::middleware::RateLimitLayer;
use rust_web_server::core::New;

let app = App::new().wrap(RateLimitLayer);
```

Compose middleware with `AppWithState` using `.wrap()` on the state app directly:

```rust
use rust_web_server::app::App;

struct MyState { db_url: String }

let app = App::with_state(MyState { db_url: "postgres://...".to_string() })
    .get("/users", |_req, _params, _conn, state| {
        // access state.db_url
        Response::new()
    })
    .wrap(LoggingMiddleware)
    .wrap(RateLimitLayer);
```

---

### 22. Server-Sent Events

`Sse` builds a complete `text/event-stream` response from a chain of events. The entire body is buffered before sending — suitable for pre-known event sequences (progress updates, batch push, AI responses already collected). For true live streaming over an open connection, write the SSE headers and event lines directly to the TCP stream in a custom loop (same pattern as WebSocket).

```rust
use rust_web_server::sse::{Sse, SseEvent};
use rust_web_server::state::AppWithState;

struct State { messages: Vec<String> }

let app = AppWithState::new(State {
    messages: vec!["first".to_string(), "second".to_string()],
})
.get("/events", |_req, _params, _conn, state| {
    let mut sse = Sse::new().event("open", "");
    for (i, msg) in state.messages.iter().enumerate() {
        sse = sse.push(
            SseEvent::data(msg)
                .id(&(i + 1).to_string())
                .event_type("message"),
        );
    }
    sse.retry(3000).into_response()
});
```

`SseEvent` fields: `data` (required, multi-line supported), `id`, `event_type`, `retry`. `Sse` methods: `event(type, data)`, `data(data)`, `push(SseEvent)`, `retry(ms)`, `comment(text)`.

---

### 23. Auth middleware — Basic Auth and JWT

`BasicAuthLayer` and `JwtLayer` are `Middleware` implementations from `src/auth/` (enabled with `features = ["auth"]`).

```toml
rust-web-server = { version = "17", features = ["auth"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::auth::{BasicAuthLayer, JwtLayer, build_jwt, verify_jwt, extract_bearer_token};
use rust_web_server::core::New;

// ── Basic Auth ────────────────────────────────────────────────────────────────

let app = App::new()
    .wrap(BasicAuthLayer::new(|user, pass| {
        // constant-time comparison recommended for production
        user == "admin" && pass == "s3cret"
    }));

// ── JWT ───────────────────────────────────────────────────────────────────────

let secret = b"my-hs256-secret";

// Issue a token (e.g. from a login handler):
let token = build_jwt(r#"{"sub":"42","exp":9999999999}"#, secret);

// Protect routes:
let app = App::new().wrap(JwtLayer::new(secret));

// Access claims inside a handler (re-verify — cheap):
// let claims = extract_bearer_token(&req).and_then(|t| verify_jwt(&t, secret));
// let user_id = claims?.sub;
```

`verify_jwt` returns `None` on: bad format, algorithm other than HS256, signature mismatch (constant-time), or expired `exp` claim. `Claims` exposes `sub`, `exp` (Unix seconds), and `raw` (the full JSON payload string for custom claims).

---

### 24. Serde JSON (deserialize request / serialize response)

`Json<T>` (`serde` feature) wraps a serde type and bridges request bodies to typed structs and typed structs back to JSON responses. Enable the feature in your `Cargo.toml`:

```toml
rust-web-server = { version = "17", features = ["serde"] }
```

```rust
use serde::{Deserialize, Serialize};
use rust_web_server::json::Json;
use rust_web_server::state::AppWithState;

#[derive(Deserialize)]
struct CreateUser { name: String, age: u32 }

#[derive(Serialize)]
struct UserResponse { id: u64, name: String }

let app = AppWithState::new(())
    .post("/users", |req, _params, _conn, _state| {
        // Deserialize — returns 400 on bad/missing JSON
        let Json(payload) = match Json::<CreateUser>::from_request(&req) {
            Ok(j)  => j,
            Err(r) => return r,
        };
        // Serialize — returns 200 application/json
        Json(UserResponse { id: 1, name: payload.name }).into_response()
    })
    .get("/users/:id", |_req, params, _conn, _state| {
        let id: u64 = params.get("id").unwrap_or("0").parse().unwrap_or(0);
        Json(UserResponse { id, name: "Alice".to_string() }).into_response()
    });
```

`Json<T>` implements `Deref<Target = T>` so you can access fields directly without unwrapping:

```rust
let json = Json::<CreateUser>::from_request(&req)?;
println!("{}", json.name); // via Deref
```

It also implements `FromRequest`, so it composes with the typed extractor pattern.

---

### 25. Session management

`SessionStore` is a thread-safe in-memory session store. Place one in your application state (`AppWithState<S>`) and share it across all handlers. `create()` generates a session, `save()` persists mutations, `load()` retrieves live sessions, `destroy()` deletes one, and `purge_expired()` reclaims memory.

```rust
use rust_web_server::app::App;
use rust_web_server::session::{self, SessionStore};
use rust_web_server::header::Header;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

struct State { sessions: SessionStore }

let app = App::with_state(State { sessions: SessionStore::new(3600) })
    .post("/login", |req, _params, _conn, state| {
        // validate credentials (not shown)…
        let mut sess = state.sessions.create();
        sess.set("user_id", "42");
        state.sessions.save(&sess);

        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.headers.push(Header {
            name: "Set-Cookie".to_string(),
            value: session::session_cookie(&sess.id, "sid", 3600),
        });
        r
    })
    .post("/logout", |req, _params, _conn, state| {
        if let Some(sid) = session::session_id_from_request(&req, "sid") {
            state.sessions.destroy(&sid);
        }
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.headers.push(Header {
            name: "Set-Cookie".to_string(),
            value: session::destroy_cookie("sid"),
        });
        r
    })
    .get("/profile", |req, _params, _conn, state| {
        let mut r = Response::new();
        let sess = session::session_id_from_request(&req, "sid")
            .and_then(|sid| state.sessions.load(&sid));
        match sess {
            Some(s) => {
                let _user_id = s.get("user_id").unwrap_or("guest");
                r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            }
            None => {
                r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
            }
        }
        r
    });
```

Call `store.purge_expired()` periodically (e.g. from a background thread) to reclaim memory from expired sessions.

**Persistent sessions with DbSessionStore** (requires `model-sqlite` / `model-postgres` / `model-mysql`):

```rust
use rust_web_server::model::DbPool;
use rust_web_server::session::DbSessionStore;

// One pool → one persistent database; sessions survive restarts.
// For production use a file or network database instead of :memory:.
let pool = DbPool::memory().await?;
let store = DbSessionStore::new(pool, 3600).await?;

// All methods are async fn returning Result
let mut sess = store.create().await?;
sess.set("user_id", "99");
store.save(&sess).await?;

let loaded = store.load(&sess.id).await?.unwrap();
assert_eq!(Some("99"), loaded.get("user_id"));

// Purge expired rows when needed (no automatic sweep)
store.purge_expired().await?;
```

`DbSessionStore` auto-creates the `rws_sessions(id, data, expires_at)` table on first construction.

**Persistent sessions with RedisSessionStore** (requires a running Redis server):

```rust
use rust_web_server::session::RedisSessionStore;

// Connect to localhost:6379 with no auth; or use from_env()
let store = RedisSessionStore::new("127.0.0.1:6379", None, 3600);

let mut sess = store.create().unwrap();
sess.set("role", "editor");
store.save(&sess).unwrap();

let loaded = store.load(&sess.id).unwrap().unwrap();
assert_eq!(Some("editor"), loaded.get("role"));

// Redis TTL expires sessions automatically — purge_expired() is a no-op
store.destroy(&sess.id).unwrap();
```

`RedisSessionStore::from_env()` reads `RWS_REDIS_HOST`, `RWS_REDIS_PORT`, `RWS_REDIS_PASSWORD`, `RWS_REDIS_TTL_SECS`.

---

### 26. Async handlers with shared state

`AsyncAppWithState<S>` (requires the `http2` Cargo feature) lets handlers be `async fn` closures that can `await` database queries, HTTP clients, or any async I/O. Use `App::with_async_state(state)` as the entry point.

```rust
use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use std::sync::Arc;
use tokio::sync::Mutex;

struct Db {
    items: Mutex<Vec<String>>,
}

// cargo build --features http2
let app = App::with_async_state(Db { items: Mutex::new(vec![]) })
    .get("/items", |_req, _params, _conn, state| async move {
        let items = state.items.lock().await;
        let body = items.join(",");
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())];
        r
    })
    .post("/items", |req, _params, _conn, state| async move {
        let name = String::from_utf8_lossy(&req.body).to_string();
        state.items.lock().await.push(name);
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
        r
    });
```

Handler signature: `Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut` where `Fut: Future<Output = Response> + Send + 'static`. Handlers receive owned values so the future is `'static`. Path matching (`:param`, `*wildcard`) and fall-through to the built-in `App` chain are included.

---

### 27. IP allowlist / denylist

`IpFilter` middleware (`src/ip_filter/`) blocks or gates requests by client IPv4 address. Entries may be exact addresses (`"1.2.3.4"`) or CIDR ranges (`"10.0.0.0/8"`). Malformed entries are silently skipped.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::ip_filter::IpFilter;

// Restrict to internal networks only.
let internal_only = App::new()
    .wrap(IpFilter::allow(["127.0.0.1", "10.0.0.0/8", "192.168.0.0/16"]));

// Block a known-bad range.
let with_block = App::new()
    .wrap(IpFilter::deny(["198.51.100.0/24"]));

// Chain both: allow internal, then deny a specific internal address.
let layered = App::new()
    .wrap(IpFilter::allow(["10.0.0.0/8"]))
    .wrap(IpFilter::deny(["10.0.0.99"]));
```

Non-matching IPs in allow mode and IPv6 addresses both receive `403 Forbidden`. Use `IpFilter::deny` when most traffic should pass and only specific addresses need blocking.

---

### 28. Declarative routing table with `routes!`

`routes!` replaces repeated `.get(path, handler)` calls with a single declarative table. Any builder that exposes `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()` works — `AppWithState`, `AsyncAppWithState`, or `Router`.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::routes;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

struct Db;

// AppWithState<S> passes &S (not &Arc<S>) to the handler.
fn list_items(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Db) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

fn create_item(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Db) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}

let app = routes! {
    App::with_state(Db),
    GET    "/items"     => list_items,
    POST   "/items"     => create_item,
    GET    "/items/:id" => list_items,   // reuse handler
    DELETE "/items/:id" => list_items,   // reuse handler
};
```

Combine with `#[route]` / `#[get]` attributes (requires `features = ["macros"]`) to co-locate the route declaration with the handler function:

```toml
# Cargo.toml
rust-web-server = { version = "17", features = ["macros"] }
```

```rust
use rust_web_server::{get, post};

// Route: `GET /items` is added as a doc-comment; function is unchanged.
#[get("/items")]
fn list_items(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Db) -> Response { /* ... */ }

#[post("/items")]
fn create_item(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Db) -> Response { /* ... */ }
```

---

### 29. Request validation

`Validate` trait and `#[derive(Validate)]` (requires `features = ["macros"]`) add
field-level validation. `Validated<T>` chains extraction and validation in one `from_request`
call: `400` if extraction fails, `422 Unprocessable Entity` with a structured JSON error body
if any field constraint is violated. All failures are collected before returning so the caller
sees every invalid field at once.

```toml
# Cargo.toml
rust-web-server = { version = "17", features = ["macros"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::extract::{FromRequest, BodyText};
use rust_web_server::validate::{Validate, Validated, ValidationErrors};
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

// ── Step 1: define your input type ──────────────────────────────────────────

struct CreateUser {
    name: String,
    email: String,
    age: u8,
}

// ── Step 2: extract from the request (or use Json<T> with the serde feature) ─

impl FromRequest for CreateUser {
    fn from_request(req: &Request) -> Result<Self, Response> {
        // Simplified: in practice use Json<T> or a custom body parser
        let BodyText(body) = BodyText::from_request(req)?;
        let mut parts = body.splitn(3, ',');
        Ok(CreateUser {
            name:  parts.next().unwrap_or("").to_string(),
            email: parts.next().unwrap_or("").to_string(),
            age:   parts.next().unwrap_or("0").trim().parse().unwrap_or(0),
        })
    }
}

// ── Step 3: implement Validate (or use #[derive(Validate)]) ──────────────────

impl Validate for CreateUser {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if self.name.is_empty() || self.name.chars().count() > 50 {
            errors.add("name", "must be 1–50 characters");
        }
        if !rust_web_server::validate::is_email(&self.email) {
            errors.add("email", "must be a valid email address");
        }
        if self.age > 150 {
            errors.add("age", "must be at most 150");
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

// ── Or, with the derive macro (fields must be String / numeric types) ─────────

// use rust_web_server::Validate;  // re-exported from rws-macros when features = ["macros"]
//
// #[derive(Validate)]
// struct CreateUser {
//     #[validate(length(min = 1, max = 50))]
//     name: String,
//     #[validate(email)]
//     email: String,
//     #[validate(range(min = 0, max = 150))]
//     age: u8,
// }

// ── Step 4: use Validated<T> in a handler ────────────────────────────────────

fn create_user(req: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
    // Extraction failure → 400; validation failure → 422 with JSON errors body
    let Validated(user) = match Validated::<CreateUser>::from_request(req) {
        Ok(v)    => v,
        Err(res) => return res,
    };

    // user.name, user.email, user.age are guaranteed valid here
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}

let app = App::with_state(())
    .post("/users", create_user);
```

On validation failure the response body looks like:

```json
{"errors":[{"field":"email","message":"must be a valid email address"},{"field":"age","message":"must be at most 150"}]}
```

### 30. Reverse proxy / load balancing

`ReverseProxy` (in `src/proxy/`) is a `Middleware` that forwards incoming
requests to one or more plain-HTTP backends.  Backends are selected in
round-robin order; when a backend is unreachable the proxy tries the next one
before returning `502 Bad Gateway`.  Hop-by-hop headers are stripped;
`X-Forwarded-For` and `Via: 1.1 rws` are added to every forwarded request.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::{LoadBalancing, ReverseProxy};

// Forward every request in round-robin across two backends.
let app = App::new()
    .wrap(ReverseProxy::new(["http://backend-1:8080", "http://backend-2:8080"])
        .strategy(LoadBalancing::RoundRobin));

// Proxy only /api/* to one backend; everything else is handled locally.
let app = App::new()
    .wrap(ReverseProxy::new(["http://api-service:3000"])
        .path_prefix("/api")
        .connect_timeout_ms(2000)
        .read_timeout_ms(10000));
```

**Behaviour summary**

| Condition | Result |
|-----------|--------|
| Backend returns a response | Forward the status code and body as-is |
| Backend connection fails | Try next backend in round-robin order |
| All backends fail | `502 Bad Gateway` |
| Path prefix set and URI does not match | Pass through to the inner application |

**Connection pooling**

`ReverseProxy` ships with an embedded `ConnPool` (8 idle connections per backend, 60 s idle timeout).  When a backend sends `Connection: keep-alive`, the TCP stream is returned to the pool and reused for the next request, eliminating per-request TCP handshakes.

To share one pool across multiple `ReverseProxy` instances or to tune pool parameters:

```rust
use std::sync::Arc;
use std::time::Duration;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::{ConnPool, ReverseProxy};

let pool = Arc::new(ConnPool::new(32, Duration::from_secs(120)));

let api  = App::new().wrap(
    ReverseProxy::new(["http://api:3000"]).with_pool(Arc::clone(&pool)));
let auth = App::new().wrap(
    ReverseProxy::new(["http://auth:4000"]).with_pool(Arc::clone(&pool)));
```

To tune the built-in pool without sharing it:

```rust
use rust_web_server::proxy::ReverseProxy;

let proxy = ReverseProxy::new(["http://backend:8080"]).max_idle_conns(16);
```

**Streaming responses (SSE, AI token streams, large downloads)**

`ReverseProxy` automatically detects streaming backend responses and forwards bytes to the client as they arrive without buffering the full body. Detection criteria:

| Condition | Example |
|-----------|---------|
| `Content-Type: text/event-stream` | SSE endpoints |
| `Transfer-Encoding: chunked` | OpenAI / Anthropic streaming APIs |
| `Content-Length > 1 MB` | Large file downloads |

For chunked backends, raw chunk frames are forwarded as-is (client decodes). For SSE and plain streams, bytes are re-encoded as chunked so the browser receives each fragment immediately. Streamed connections are never returned to the pool — the TCP socket is consumed by the pipe.

The same `Response::stream_pipe` field is available to any handler for custom streaming:

```rust
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::header::Header;

fn sse_handler(_req: &_, _p: &_, _c: &_, _s: &_) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.headers.push(Header { name: "Content-Type".into(), value: "text/event-stream".into() });
    r.stream_pipe = Some(Box::new(std::io::Cursor::new(b"data: hello\n\n".to_vec())));
    r
}
```

### 33. Per-route metrics

`MetricsLayer` (in `src/metrics/`) is a `Middleware` that instruments every
request passing through it and appends per-route counters and latency histograms
to the existing `GET /metrics` Prometheus output.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::metrics::MetricsLayer;

let app = App::new().wrap(MetricsLayer);
```

After wrapping, `GET /metrics` emits the standard server-wide metrics plus:

```
# HELP rws_route_requests_total Total requests handled per route
# TYPE rws_route_requests_total counter
rws_route_requests_total{method="GET",path="/api/users",status="200"} 1204
rws_route_requests_total{method="GET",path="/api/users",status="404"} 7

# HELP rws_route_duration_seconds Request duration in seconds per route
# TYPE rws_route_duration_seconds histogram
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.005"} 980
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.01"} 1100
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.025"} 1190
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.05"} 1200
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.1"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.25"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="0.5"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="1"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="2.5"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="5"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="10"} 1204
rws_route_duration_seconds_bucket{method="GET",path="/api/users",le="+Inf"} 1204
rws_route_duration_seconds_sum{method="GET",path="/api/users"} 4.876543210
rws_route_duration_seconds_count{method="GET",path="/api/users"} 1204
```

**Behaviour notes:**

- Query strings are stripped: `/users?page=2` is keyed as `/users`.
- Handler errors (`Err` return) are attributed to status `500`.
- The route section is absent from `/metrics` until at least one route is observed.
- `record_route(method, path, status, elapsed_secs)` is also callable directly for custom instrumentation.

---

### 31. Response caching

`CacheLayer` (in `src/cache/`) is a `Middleware` that stores successful `GET`
responses in memory and serves subsequent identical requests directly from the
cache, bypassing the handler.

```rust
use rust_web_server::app::App;
use rust_web_server::cache::CacheLayer;
use rust_web_server::core::New;

// Cache up to 1 000 entries, each valid for 60 seconds.
let app = App::new()
    .wrap(CacheLayer::memory(1000).ttl(60));

// Separate cache entries by Accept header (content negotiation).
let app = App::new()
    .wrap(CacheLayer::memory(500)
        .ttl(120)
        .vary_by_header("Accept")
        .vary_by_header("Accept-Language"));
```

**Behaviour summary**

| Condition | Result |
|-----------|--------|
| GET, 2xx, no `Cache-Control: no-store/private` | Response stored; subsequent hits served from cache with `Age` header |
| Non-GET method | Always passes through to the handler; never cached |
| Response has `Cache-Control: no-store` or `private` | Handler is called, response is **not** stored |
| Request has `Cache-Control: no-cache` | Cache is bypassed, handler is called; the fresh response **is** stored (revalidation) |
| Entry exceeds TTL | Next request is a miss; handler is called and result replaces the stale entry |
| Store reaches capacity | Expired entries are purged first; if still full, the oldest entry is evicted (insertion order) |

---

### 32. Hot config reload

`config_reload::reload()` re-reads `rws.config.toml` and applies changes to
CORS rules, rate-limit thresholds, log format, and request allocation size
**without restarting the server**.

**Trigger via SIGHUP (recommended)**

```bash
kill -HUP $(pidof rws)
# or
kill -HUP $(cat /var/run/rws.pid)
```

The server prints a confirmation line:

```
Config reloaded — cors_allow_all=false rate_limit=1000/60 log_format=clf
```

**Trigger via HTTP endpoint**

```bash
curl -X POST http://localhost:7878/admin/config/reload
```

**Read the current config snapshot in a handler**

```rust
use rust_web_server::config_reload;

fn my_handler(request: &Request, response: Response, conn: &ConnectionInfo) -> Response {
    let cfg = config_reload::current();
    if cfg.cors_allow_all {
        // ...
    }
    response
}
```

**What is hot-reloadable vs. what requires restart**

| Setting | Hot-reloadable |
|---------|---------------|
| CORS (`RWS_CONFIG_CORS_*`) | ✅ |
| Rate-limit thresholds (`RWS_CONFIG_RATE_LIMIT_*`) | ✅ |
| Log format (`RWS_CONFIG_LOG_FORMAT`) | ✅ |
| Request allocation size | ✅ |
| IP / port | ❌ bound socket cannot move |
| Thread count | ❌ thread pool is fixed at startup |
| TLS cert / key paths | ❌ acceptor is built once |

---

### 34. Distributed tracing (OtelLayer)

`OtelLayer` is a `Middleware` that creates an HTTP server span for each request,
reads W3C `traceparent` headers from upstream services, and exports spans to
stdout or an OTLP HTTP collector — all with zero new Cargo dependencies.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::otel::{OtelLayer, TracingConfig, ExporterConfig, setup};

// Call once at startup, before the server starts accepting requests.
setup(TracingConfig {
    service_name: "checkout-service".to_string(),
    service_version: env!("CARGO_PKG_VERSION").to_string(),
    exporter: ExporterConfig::Otlp {
        endpoint: "http://localhost:4318".to_string(), // OTLP HTTP port
    },
    sample_rate: 1.0,   // 0.0–1.0; head-based sampling
    batch_size: 128,    // flush to exporter when batch fills
});

let app = App::new().wrap(OtelLayer);
```

Or via environment variables (compatible with the OpenTelemetry spec):

```bash
export OTEL_SERVICE_NAME=checkout-service
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
export OTEL_TRACES_SAMPLER_ARG=0.1   # sample 10% of requests
```

```rust
rust_web_server::otel::setup_from_env();
let app = App::new().wrap(OtelLayer);
```

Flush all buffered spans before the process exits:

```rust
rust_web_server::otel::shutdown();
```

**What OtelLayer does per request:**

| Step | Detail |
|------|--------|
| Read `traceparent` | Parses W3C Trace Context from request header; starts new root span if absent |
| Create span | New `span_id`; shares `trace_id` with upstream if continuing a trace |
| Thread-local context | `current_traceparent()` returns the active `traceparent` on the current thread; used by `ReverseProxy` to propagate context to backends |
| Record span | On response: records `http.method`, `http.target`, `http.status_code`; status code 2 (Error) for 5xx |
| Export | Batch flushed to exporter when `batch_size` reached or on `shutdown()` |

**Exporters:**

| Config | Behaviour |
|--------|-----------|
| `ExporterConfig::Stdout` | JSON lines to stdout; pipe to `jq` or a log aggregator |
| `ExporterConfig::Otlp { endpoint }` | HTTP POST `/v1/traces` (OTLP JSON); works with Jaeger ≥ 1.35, Grafana Tempo, OpenTelemetry Collector |
| `ExporterConfig::Discard` | No-op; useful in tests to silence output |

**Testing with OtelLayer:**

```rust
use rust_web_server::otel::{ExporterConfig, TracingConfig, OtelLayer, setup};
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::test_client::TestClient;
use std::sync::Once;

static INIT: Once = Once::new();
fn init() {
    INIT.call_once(|| {
        setup(TracingConfig {
            exporter: ExporterConfig::Discard,
            ..Default::default()
        });
    });
}

#[test]
fn my_test() {
    init();
    let client = TestClient::new(App::new().wrap(OtelLayer));
    let res = client.get("/api/users").send();
    assert_eq!(200, res.status());
}
```

---

## Kubernetes Deployment

A `Dockerfile` is included in the repository root. Build and push with:

```bash
docker build -t my-registry/rws:latest .
docker push my-registry/rws:latest
```

### Health probes

`rws` exposes two endpoints for Kubernetes liveness and readiness probes:

- `GET /healthz` — always returns `200 OK`; use for `livenessProbe`
- `GET /readyz` — returns `200 OK` after startup completes, `503` during drain; use for `readinessProbe`

Example pod spec:
```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 7878
  initialDelaySeconds: 5
readinessProbe:
  httpGet:
    path: /readyz
    port: 7878
  initialDelaySeconds: 5
```

### Prometheus metrics

`GET /metrics` returns counters and gauges in Prometheus text format (`text/plain; version=0.0.4`):

**Server-wide (always present)**
- `rws_requests_total` — total HTTP requests handled
- `rws_errors_total` — requests that returned an application error
- `rws_active_connections` — currently open connections

**Per-route (present when `MetricsLayer` is in the middleware stack)**
- `rws_route_requests_total{method,path,status}` — request count per route and status code
- `rws_route_duration_seconds{method,path}` — latency histogram (11 standard Prometheus buckets: 5 ms … 10 s)

See [use case #33](#33-per-route-metrics) for the setup snippet.

---

### 35. Automatic TLS (ACME / Let's Encrypt)

`AcmeManager` (`acme` feature flag) provisions a TLS certificate from Let's Encrypt at startup and renews it automatically before expiry — no `certbot`, no cron jobs, no manual restarts.

```toml
# Cargo.toml
[dependencies]
rust-web-server = { version = "17", features = ["acme"] }
```

```bash
# Required env vars
RWS_CONFIG_ACME_DOMAINS=example.com,www.example.com
RWS_CONFIG_ACME_EMAIL=admin@example.com

# Optional
RWS_CONFIG_ACME_STAGING=true            # use Let's Encrypt staging (testing only)
RWS_CONFIG_ACME_CHALLENGE_PORT=80       # port for HTTP-01 challenge server (default 80)
RWS_CONFIG_ACME_RENEW_BEFORE_DAYS=30   # renew when fewer than N days remain
RWS_CONFIG_ACME_ACCOUNT_KEY_PATH=acme_account.key  # persisted ACME account key
```

The library integration is automatic when the `acme` feature is enabled — `main.rs` checks `AcmeConfig::from_env()` at startup:

```rust
#[cfg(feature = "acme")]
{
    use rust_web_server::acme::{AcmeConfig, AcmeManager};
    if let Some(cfg) = AcmeConfig::from_env() {
        let mgr = AcmeManager::new(cfg);
        // Provision cert now (or skip if one is still valid).
        if let Err(e) = mgr.provision_if_needed().await {
            eprintln!("[ACME] Startup provisioning failed: {e}");
        }
        // Background loop checks every 12 hours; after renewal sends SIGHUP
        // so the TLS acceptor reloads without restarting.
        tokio::spawn(mgr.run_renewal_loop());
    }
}
```

**Protocol flow:**
1. Fetches the ACME directory from Let's Encrypt.
2. Creates or loads an ECDSA P-256 account key (stored at `acme_account.key`).
3. Places a `newOrder` for all configured domains.
4. For each domain, starts a temporary TCP server on port 80 that answers the HTTP-01 challenge (`/.well-known/acme-challenge/<token>`), signals the ACME server, and polls until `valid`.
5. Generates a fresh P-256 certificate key and CSR with `rcgen`.
6. Finalises the order and downloads the signed certificate chain.
7. Writes the chain to `RWS_CONFIG_TLS_CERT_FILE` and the key to `RWS_CONFIG_TLS_KEY_FILE`.

**Zero-downtime renewal:** after writing new files, `run_renewal_loop` sends `SIGHUP` to the process. The `run_tls` accept loop catches the signal and replaces the `TlsAcceptor` in-place — no TCP connections are dropped.

### 36. MCP server (Model Context Protocol)

`McpServer` turns any `rws` application into an MCP server reachable from Claude, Cursor, and other LLM tool-callers. It uses the Streamable HTTP transport: a single `POST /mcp` endpoint that speaks JSON-RPC 2.0. No extra Cargo features or dependencies are needed.

**Standalone MCP server** — tools, resources, and prompts only:

```rust
use rust_web_server::mcp::{McpContent, McpServer, PromptArgDef, PromptMessage, extract_arg};
use rust_web_server::server::Server;

fn main() {
    let srv = McpServer::new("my-app", "1.0")
        .require_bearer(std::env::var("MCP_TOKEN").unwrap())
        .tool(
            "add",
            "Add two numbers",
            r#"{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}}}"#,
            |args| {
                let a: f64 = extract_arg(args, "a").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let b: f64 = extract_arg(args, "b").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                Ok(McpContent::text(format!("{}", a + b)))
            },
        )
        .resource(
            "docs://{topic}",
            "Documentation",
            "Fetch docs for a topic",
            |uri| Ok(McpContent::text(format!("Docs for {uri}"))),
        )
        .prompt_with_args(
            "translate",
            "Translate text",
            vec![
                PromptArgDef::required("text", "Text to translate"),
                PromptArgDef::optional("lang", "Target language (default English)"),
            ],
            |args| {
                let text = extract_arg(args, "text").unwrap_or_default();
                let lang = extract_arg(args, "lang").unwrap_or_else(|| "English".to_string());
                Ok(vec![PromptMessage::user(format!("Translate to {lang}: {text}"))])
            },
        );

    Server::new(None).run(srv);
}
```

**Combined HTTP routes + MCP** — use `.mcp()` on `AppWithState` to layer MCP on top of existing routes. Requests that don't match `POST /mcp` fall through to the HTTP layer:

```rust
use rust_web_server::app::App;
use rust_web_server::mcp::{McpContent, extract_arg};
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::routes;
use rust_web_server::server::{ConnectionInfo, Server};

#[derive(Clone)]
struct State { version: &'static str }

fn get_version(_req: &Request, _p: &PathParams, _c: &ConnectionInfo, s: &State) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

fn main() {
    // Dispatch chain: McpServer → AppWithState → App (built-in controllers)
    let http = routes! {
        App::with_state(State { version: "1.0.0" }),
        GET "/api/version" => get_version,
    };

    let mut mcp = http.mcp("my-app", "1.0.0");
    if let Ok(token) = std::env::var("MCP_TOKEN") {
        mcp = mcp.require_bearer(token);
    }
    let app = mcp.tool(
        "server_version",
        "Return the running server version",
        r#"{"type":"object"}"#,
        |_| Ok(McpContent::json(r#"{"version":"1.0.0"}"#)),
    );

    let (listener, pool) = Server::setup().unwrap();
    Server::run(listener, pool, app);
}
```

`require_bearer(token)` enforces `Authorization: Bearer <token>` on every MCP request; set `MCP_TOKEN` in the environment. Requests from unknown clients receive a `401 Unauthorized` with `WWW-Authenticate: Bearer`.

Use `.at("/custom/path")` to override the default `/mcp` endpoint. The bundled `rws` binary ships 8 built-in tools: `server_config`, `feature_flags`, `server_metrics`, `rate_limit_config`, `check_rate_limit`, `cors_config`, `list_static_files`, `reload_config`.

**Supported MCP methods:** `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`, `notifications/initialized`.

---

### 37. Virtual hosting / SNI routing

A single `rws` instance can serve multiple domains from one IP+port, each with its own TLS certificate. The TLS layer selects the right cert at handshake time via SNI; `ConnectionInfo::sni_hostname` exposes it to every handler; `Router::with_host()` narrows a router to one domain.

**Step 1 — configure virtual hosts in `rws.config.toml`:**

```toml
# Default cert (used when no virtual host matches or client omits SNI)
tls_cert_file = '/etc/ssl/default.pem'
tls_key_file  = '/etc/ssl/default.key'

[[virtual_host]]
domain    = 'example.com'
cert_file = '/etc/ssl/example.pem'
key_file  = '/etc/ssl/example.key'

[[virtual_host]]
domain    = 'other.com'
cert_file = '/etc/ssl/other.pem'
key_file  = '/etc/ssl/other.key'
```

Or via environment variables (useful in Kubernetes):

```bash
RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN=example.com
RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE=/etc/ssl/example.pem
RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE=/etc/ssl/example.key
```

**Step 2 — route per domain in the app:**

```rust
use rust_web_server::app::App;
use rust_web_server::application::Application;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::{PathParams, Router};
use rust_web_server::server::ConnectionInfo;
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

fn example_home(_req: &Request, _p: &PathParams, _c: &ConnectionInfo) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(b"example.com home".to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

fn other_home(_req: &Request, _p: &PathParams, _c: &ConnectionInfo) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(b"other.com home".to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

pub struct VhostApp;

impl Application for VhostApp {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let example = Router::new()
            .with_host("example.com")
            .get("/", example_home);

        let other = Router::new()
            .with_host("other.com")
            .get("/", other_home);

        if let Some(r) = example.handle(request, connection) { return Ok(r); }
        if let Some(r) = other.handle(request, connection)   { return Ok(r); }

        // Fall through to built-in App controllers (static files, /healthz, etc.)
        App::new().execute(request, connection)
    }
}
```

For plain-HTTP virtual hosting (no TLS), `with_host()` falls back to the `Host` request header when `sni_hostname` is `None`.

**Hot-reload certs** — send `SIGHUP` or `POST /admin/config/reload` to pick up renewed certificates (e.g. after ACME renewal) without restarting the server.

---

### 38. Request / response rewriting

`RewriteLayer` is a `Middleware` that transforms requests before they reach handlers and responses before they leave the server. Compose it with any `App` or middleware stack using `.wrap()`.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::rewrite::RewriteLayer;

let app = App::new()
    .wrap(RewriteLayer::new()
        // ── Request rewrites ──────────────────────────────────────────────
        // Add or replace a request header (case-insensitive match).
        .request_header_set("X-Env", "production")
        // Remove a request header.
        .request_header_remove("X-Debug")
        // Strip a path prefix before routing (no-op if absent; normalises to "/").
        .request_uri_strip_prefix("/api/v1")
        // Alternatively, replace the URI entirely or add a prefix:
        // .request_uri_set("/internal/resource")
        // .request_uri_add_prefix("/v2")

        // ── Response rewrites ─────────────────────────────────────────────
        // Add or replace a response header.
        .response_header_set("Cache-Control", "no-store")
        // Remove a response header.
        .response_header_remove("Server")
        // Override the status code (useful when proxying to change 404 → 410).
        // .response_status(410, "Gone")
        // Byte-level find-and-replace in the response body (all content ranges).
        .response_body_replace("http://staging.internal", "https://example.com"));
```

Rules are applied in the order they are registered. The incoming `Request` is cloned before modification — the original is never mutated, so other middleware layers in the stack see the unmodified request.

---

### 39. L4 TCP proxy

`TcpProxy` is a standalone listener that accepts TCP connections and relays bytes bidirectionally to a pool of backends. It is useful for proxying any TCP protocol — databases, legacy services, or plain HTTP/1.1 — without parsing the stream.

```rust
use rust_web_server::tcp_proxy::TcpProxy;

// Listens on 0.0.0.0:5432, round-robins across two Postgres backends.
TcpProxy::new(["10.0.0.10:5432", "10.0.0.11:5432"])
    .connect_timeout_ms(500)
    .bind("0.0.0.0:5432")
    .expect("TCP proxy failed");
```

`bind()` blocks until an error occurs. Run it in a separate thread alongside your HTTP server.

---

### 40. UDP proxy

`UdpProxy` is a request-reply UDP proxy. Each incoming datagram is forwarded to a backend; the reply is returned to the original sender. Suitable for DNS, syslog, and other datagram protocols.

```rust
use rust_web_server::udp_proxy::UdpProxy;

UdpProxy::new(["10.0.0.10:53", "10.0.0.11:53"])
    .reply_timeout_ms(2000)
    .buffer_size(8192)
    .bind("0.0.0.0:53")
    .expect("UDP proxy failed");
```

`bind()` blocks indefinitely. Run it in a separate thread.

---

### 41. WebSocket proxy

`WsProxy` listens for HTTP upgrade requests, performs the WebSocket handshake with the client, connects to a backend, and relays frames bidirectionally. Plain `ws://` backends use two threads; `wss://` backends use a single-thread polling relay (5 ms timeout) to avoid TLS stream-sharing deadlocks.

```rust
use rust_web_server::ws_proxy::WsProxy;

// Plain TCP backends
WsProxy::new(["ws://chat-backend:9000", "ws://chat-backend:9001"])
    .connect_timeout_ms(500)
    .read_timeout_ms(30_000)
    .bind("0.0.0.0:8080")
    .expect("WS proxy failed");

// TLS backend — wss:// (requires http-client or http2 feature)
WsProxy::new(["wss://chat.example.com"])   // port defaults to 443
    .bind("0.0.0.0:8080")
    .expect("WS proxy failed");

WsProxy::new(["wss://chat.internal:8443"]) // explicit port
    .bind("0.0.0.0:8080")
    .expect("WS proxy failed");
```

`bind()` blocks indefinitely. The proxy does raw byte relay after the handshake, so any WebSocket subprotocol passes through transparently.

---

### 42. HTTP/2 reverse proxy

`H2ReverseProxy` forwards requests to HTTP/2 backends over plain TCP or TLS. It works as a `Middleware` in the normal stack; `block_in_place` bridges the sync handler into the tokio runtime. Requires the `http2` feature.

Backend URL schemes:
- `h2://host:port` — plain TCP (cleartext HTTP/2)
- `h2s://host:port` — TLS with ALPN `h2`; port defaults to 443
- `https://host:port` — same as `h2s://`

```rust
#[cfg(feature = "http2")]
{
    use rust_web_server::app::App;
    use rust_web_server::core::New;
    use rust_web_server::proxy::H2ReverseProxy;

    // Plain TCP backends
    let app = App::new()
        .wrap(H2ReverseProxy::new(["h2://backend1:8080", "h2://backend2:8080"])
            .path_prefix("/api")
            .connect_timeout_ms(1000)
            .read_timeout_ms(5000));

    // TLS backends (h2s:// or https://)
    let app = App::new()
        .wrap(H2ReverseProxy::new(["h2s://api.example.com"])  // port defaults to 443
            .connect_timeout_ms(1000)
            .read_timeout_ms(5000));
}
```

Requests whose URI does not start with `path_prefix` pass through to the next middleware. `X-Forwarded-For` and `Via` are injected automatically.

---

### 43. gRPC proxy

`GrpcProxy` wraps `H2ReverseProxy` and filters on `Content-Type: application/grpc*`. All gRPC traffic is forwarded over HTTP/2; non-gRPC requests fall through to the next handler. Requires the `http2` feature.

Backend URL schemes:
- `grpc://host:port` — plain TCP (cleartext gRPC)
- `grpcs://host:port` — TLS; port defaults to 443
- `https://host:port` — same as `grpcs://`

```rust
#[cfg(feature = "http2")]
{
    use rust_web_server::app::App;
    use rust_web_server::core::New;
    use rust_web_server::proxy::GrpcProxy;

    // Plain TCP
    let app = App::new()
        .wrap(GrpcProxy::new(["grpc://grpc-service:50051"])
            .connect_timeout_ms(1000)
            .read_timeout_ms(10_000));

    // TLS — grpcs:// (every managed cloud gRPC service requires this)
    let app = App::new()
        .wrap(GrpcProxy::new(["grpcs://grpc.example.com"])  // port defaults to 443
            .connect_timeout_ms(1000)
            .read_timeout_ms(10_000));
}
```

---

### 44. mTLS (mutual TLS / client certificate authentication)

Set `RWS_CONFIG_TLS_CLIENT_CA_FILE` to the path of a PEM-encoded CA certificate file. The server will require every client to present a certificate signed by that CA; connections without a valid certificate are rejected at the TLS handshake level.

```bash
export RWS_CONFIG_TLS_CLIENT_CA_FILE=/etc/pki/ca.crt
cargo run -- --tls-cert-file=server.crt --tls-key-file=server.key
```

Or in `rws.config.toml`:

```toml
tls_client_ca_file = "/etc/pki/ca.crt"
```

The setting applies to both HTTPS (HTTP/2 + HTTP/1.1) and QUIC (HTTP/3) listeners. When `RWS_CONFIG_TLS_CLIENT_CA_FILE` is empty or unset the server performs no client certificate verification (default behaviour).

---

### 45. Canary routing / traffic splitting

`CanaryLayer` distributes requests across backends proportionally to their weights. Use it for canary releases (send 10% of traffic to a new version), A/B testing, or gradual rollouts. The distribution is deterministic: it expands the backend list by weight and uses a lock-free `AtomicUsize` counter to cycle through it.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::canary::{CanaryLayer, WeightedBackend};

// 90% of traffic to stable, 10% to the new canary build.
let app = App::new()
    .wrap(CanaryLayer::new(vec![
        WeightedBackend::new("stable-backend:8080", 9),
        WeightedBackend::new("canary-backend:8080", 1),
    ])
    .connect_timeout_ms(500)
    .read_timeout_ms(5000));
```

Setting `weight = 0` removes a backend from the rotation without removing it from the config — useful for quickly disabling a canary.

---

### 46. Circuit breaker

`CircuitBreaker` tracks per-backend failure counts and prevents sending traffic to unhealthy backends. States: **Closed** (healthy, counting failures) → **Open** (blocked until recovery window elapses) → **HalfOpen** (testing with the next request) → back to **Closed** on success.

```rust
use rust_web_server::circuit_breaker::{CircuitBreaker, global as global_breaker};

// Process-wide singleton (threshold=5, recovery=30s).
{
    let mut cb = global_breaker().lock().unwrap();
    cb.record_failure("backend-1:8080");
    if cb.is_available("backend-1:8080") {
        // send request
    }
    cb.record_success("backend-1:8080");
}

// Or create a custom breaker.
let mut cb = CircuitBreaker::new(3, 10); // open after 3 failures, recover after 10s
```

`RetryLayer` pairs naturally with the circuit breaker — it retries requests that return 502/503/504:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::circuit_breaker::RetryLayer;
use rust_web_server::proxy::ReverseProxy;

let app = App::new()
    .wrap(ReverseProxy::new(["backend-1:8080", "backend-2:8080"]))
    .wrap(RetryLayer::new().max_retries(2));
```

---

### 47. Service discovery

`BackendPool` maintains a live list of backend addresses that a background thread refreshes automatically. Four discovery sources:

```rust
use rust_web_server::service_discovery::{BackendPool, DiscoverySource};

// Static list — no polling.
let pool = BackendPool::r#static(vec!["10.0.0.1:8080".into(), "10.0.0.2:8080".into()]);

// Read from environment variables: RWS_BACKENDS_0, RWS_BACKENDS_1, …
let pool = BackendPool::env_prefix("RWS_BACKENDS").poll_interval_secs(60);

// One host:port per line in a file (blank lines and # comments ignored).
let pool = BackendPool::file("/etc/rws/backends.txt").poll_interval_secs(30);

// A-record DNS resolution — resolves all IPs for the hostname.
let pool = BackendPool::dns("my-service.internal", 8080).poll_interval_secs(15);

// Start background polling thread (no-op for Static).
pool.start();

// Read current backend list.
let backends: Vec<String> = pool.backends();
```

`BackendPool` is `Clone` — all clones share the same `Arc<RwLock<Vec<String>>>` underneath, so a refresh on one clone is visible to all.

---

### 48. Kubernetes Ingress routing

`KubernetesIngressWatcher` polls the Kubernetes API for Ingress resources and maintains a live route table. `IngressRouter` implements `Application` and forwards matching requests to the appropriate upstream service.

```rust
use rust_web_server::ingress::{IngressRouter, KubernetesIngressWatcher};
use rust_web_server::server::Server;

// Configure from environment variables:
//   RWS_K8S_API_SERVER=http://localhost:8001   (kubectl proxy)
//   RWS_K8S_TOKEN=<bearer-token>
//   RWS_K8S_NAMESPACE=default
let watcher = KubernetesIngressWatcher::from_env()
    .expect("K8s config not found");

// Start background polling (default: every 30 s).
watcher.start();

// Use as the top-level Application — routes to services via
// {service}.{namespace}.svc.cluster.local:{port}.
let router = IngressRouter::new(watcher)
    .connect_timeout_ms(500)
    .read_timeout_ms(5000);

let (listener, pool) = Server::setup().unwrap();
Server::run(listener, pool, router);
```

For **in-cluster** deployments, mount the service account and point kubectl proxy to the API server. TLS to `kubernetes.default.svc` is not yet handled natively — use `kubectl proxy` or set `RWS_K8S_API_SERVER=http://kubernetes.default.svc:80` with an appropriately configured proxy sidecar.

---

### 49. Background task scheduler

`Scheduler` is a `@Scheduled`-equivalent background task runner. Each task runs in its own dedicated thread started by `.start()`.

```rust
use std::time::Duration;
use rust_web_server::scheduler::Scheduler;

Scheduler::new()
    // Fixed rate: every 60 s, measured from task start.
    .every(Duration::from_secs(60), || {
        println!("flush metrics");
    })
    // Fixed delay: 30 s after each run completes.
    .after(Duration::from_secs(30), || {
        println!("heartbeat");
    })
    // Cron: fire at second 0 of every minute (UTC).
    // Format: "sec min hour day-of-month month day-of-week"
    .cron("0 * * * * *", || {
        println!("every minute");
    }).unwrap()
    // 10 s pause before the first run of the previous task.
    .initial_delay(Duration::from_secs(10))
    // Cron: every day at 02:30:00 UTC.
    .cron("0 30 2 * * *", || {
        println!("nightly job");
    }).unwrap()
    // Cron: weekdays only (Mon=1..Fri=5) at 09:00:00 UTC.
    .cron("0 0 9 * * 1-5", || {
        println!("business hours open");
    }).unwrap()
    .start(); // spawns one thread per task, returns immediately
```

**Cron field syntax** (6 space-separated fields, UTC):

| Field | Range | Example |
|---|---|---|
| second | 0–59 | `0`, `*/10`, `0,30` |
| minute | 0–59 | `*`, `0,15,30,45` |
| hour | 0–23 | `9-17` |
| day-of-month | 1–31 | `1`, `*/7` |
| month | 1–12 | `*`, `3-11` |
| day-of-week | 0–6 (0=Sun) | `1-5` |

---

### Structured JSON logging

Set `RWS_CONFIG_LOG_FORMAT=json` (or `log_format = 'json'` in `rws.config.toml`) to emit access logs as JSON:

```json
{"time":"2026-06-30T12:00:00Z","remote_addr":"10.0.0.5:54321","method":"GET","path":"/index.html","protocol":"HTTP/1.1","status":200,"bytes":4096}
```

### Environment variables via ConfigMap

All `RWS_CONFIG_*` variables map directly to Kubernetes ConfigMaps and Secrets:

```yaml
env:
  - name: RWS_CONFIG_IP
    value: "0.0.0.0"
  - name: RWS_CONFIG_PORT
    value: "7878"
  - name: RWS_CONFIG_LOG_FORMAT
    value: "json"
  - name: RWS_CONFIG_TLS_CERT_FILE
    valueFrom:
      secretKeyRef:
        name: tls-secret
        key: tls.crt
  - name: RWS_CONFIG_TLS_KEY_FILE
    valueFrom:
      secretKeyRef:
        name: tls-secret
        key: tls.key
```

### 50. HTML template rendering (TeraEngine)

`TeraEngine` wraps the [Tera](https://keats.github.io/tera/) crate to add Jinja2/Django-style server-side rendering. Enable it with `features = ["tera"]` in `Cargo.toml`.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["tera"] }
```

**Directory-based setup (production)**

Put templates under `templates/` (or another directory configured via `RWS_CONFIG_TEMPLATE_DIR`):

```
templates/
  base.html
  index.html
  users/list.html
```

Initialize once at startup, then call `template::render` from any handler:

```rust
use rust_web_server::template::{self, Context};
use rust_web_server::response::Response;

// In main() or App setup:
template::init("templates").unwrap();
// or read from RWS_CONFIG_TEMPLATE_DIR env var:
// template::init_from_env().unwrap();

fn home_handler(_: &Request, _: &PathParams, _: &ConnectionInfo) -> Response {
    let mut ctx = Context::new();
    ctx.insert("title", "Home");
    ctx.insert("user", &"Alice");
    template::render("index.html", &ctx).unwrap()
}
```

`templates/index.html`:

```html
{% extends "base.html" %}
{% block content %}
  <h1>{{ title }}</h1>
  <p>Hello, {{ user }}!</p>
{% endblock %}
```

**Inline templates (testing / embedded apps)**

```rust
use rust_web_server::template::{TeraEngine, Context};

let engine = TeraEngine::from_raw(&[
    ("base.html", "HEADER {% block body %}{% endblock %} FOOTER"),
    ("page.html", "{% extends \"base.html\" %}{% block body %}{{ msg }}{% endblock %}"),
]).unwrap();

let mut ctx = Context::new();
ctx.insert("msg", "Hello from rws!");
let html = engine.render("page.html", &ctx).unwrap();
// → "HEADER Hello from rws! FOOTER"

let response = engine.response("page.html", &ctx).unwrap();
// → 200 OK, Content-Type: text/html
```

**Inserting complex values**

`Context::insert` accepts anything that implements `serde::Serialize`:

```rust
#[derive(serde::Serialize)]
struct Product { name: String, price: f64 }

let products = vec![
    Product { name: "Widget".into(), price: 9.99 },
    Product { name: "Gadget".into(), price: 24.99 },
];
ctx.insert("products", &products);
```

Template:

```html
{% for p in products %}
  <li>{{ p.name }} — ${{ p.price }}</li>
{% endfor %}
```

**Configuration**

| Env var | Default | Purpose |
|---|---|---|
| `RWS_CONFIG_TEMPLATE_DIR` | `templates` | Directory scanned by `template::init_from_env()` |

### 51. Typed configuration binding (`#[derive(Config)]`)

`#[derive(Config)]` (`macros` feature) generates `fn load() -> Result<Self, String>` that reads environment variables and parses them into strongly-typed struct fields — equivalent to Spring's `@ConfigurationProperties`.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["macros"] }
```

**Basic usage**

```rust
use rust_web_server::Config;

#[derive(Config)]
#[config(prefix = "APP_")]
struct AppConfig {
    // reads APP_PORT; falls back to "8080" if absent
    #[config(env = "PORT", default = "8080")]
    port: u16,

    // reads APP_DATABASE_URL; Err if absent
    #[config(env = "DATABASE_URL")]
    database_url: String,

    // reads APP_DEBUG; None if absent or empty
    #[config(env = "DEBUG")]
    debug: Option<bool>,

    // reads APP_MAX_CONNS; required
    #[config(env = "MAX_CONNS", default = "100")]
    max_connections: u32,
}

fn main() {
    let cfg = AppConfig::load().expect("invalid config");
    println!("port={} db={} debug={:?}", cfg.port, cfg.database_url, cfg.debug);
}
```

**Field derivation rules**

| Field annotation | Absent | Present |
|---|---|---|
| `#[config(env = "KEY", default = "v")]` | use `"v"` | parse to type |
| `#[config(env = "KEY")]` (non-`Option`) | `Err` | parse to type |
| `#[config(env = "KEY")]` (`Option<T>`) | `None` | `Some(parsed)` |
| no `#[config]` | auto-key = `PREFIX + FIELD_UPPERCASED` | same |

**Supported field types**

All Rust scalar types implement `FromEnvStr` out of the box: `String`, `bool`, `u8`–`u128`, `i8`–`i128`, `f32`, `f64`, `usize`, `isize`. For `bool`, the values `"true"`, `"1"`, `"yes"` parse to `true`; `"false"`, `"0"`, `"no"` to `false`.

**Custom types**

Implement `FromEnvStr` to support custom field types:

```rust
use rust_web_server::config_binding::FromEnvStr;

#[derive(Debug)]
enum LogLevel { Debug, Info, Warn, Error }

impl FromEnvStr for LogLevel {
    fn from_env_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info"  => Ok(LogLevel::Info),
            "warn"  => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            other   => Err(format!("unknown log level {:?}", other)),
        }
    }
}

#[derive(Config)]
struct ServerConfig {
    #[config(env = "LOG_LEVEL", default = "info")]
    log_level: LogLevel,
}
```

**Low-level helpers** (no derive needed)

```rust
use rust_web_server::config_binding::{load_required, load_with_default, load_optional};

let port: u16 = load_with_default("APP_PORT", "8080")?;
let db: String = load_required("DATABASE_URL")?;
let debug: Option<bool> = load_optional("APP_DEBUG")?;
```

### 52. Config-driven proxy server

When `rws.config.toml` contains `[[route]]` or `[[upstream]]` sections, `rws` starts as a full reverse proxy — no Rust code required.

**Detect proxy mode and build the app**

```rust
// main.rs — already wired automatically in the binary.
// To use programmatically:
if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
    let (app, _handles) = rust_web_server::proxy_config::build_from_file();
    // `app` implements Application + Clone; pass to Server::run / run_tls / run_quic
}
```

**Minimal `rws.config.toml`**

```toml
[[upstream]]
name     = "backend"
backends = ["10.0.0.10:8080", "10.0.0.11:8080"]
strategy = "least_connections"  # "round_robin" (default) | "random" | "ip_hash" | "least_connections"

  [upstream.health_check]
  path                = "/healthz"
  interval_secs       = 10
  timeout_ms          = 2000
  healthy_threshold   = 2
  unhealthy_threshold = 3

[[route]]
name = "api"

  [route.match]
  path = "/api/*"

  [route.action]
  type     = "proxy"
  upstream = "backend"

  [route.middleware]
  rate_limit = { max_requests = 200, window_secs = 60 }

    [[route.middleware.rewrite.request]]
    type  = "header_set"
    name  = "X-Forwarded-Host"
    value = "api.example.com"

[[route]]
name = "catch-all"

  [route.match]
  path = "/*"

  [route.action]
  type   = "respond"
  status = 404
  body   = "Not Found"
```

**Action types**

| `type` | Behaviour |
|---|---|
| `proxy` | Forward to a named `[[upstream]]` pool over HTTP/1.1 |
| `grpc` | Forward over HTTP/2 (`Content-Type: application/grpc*`) |
| `static` | Serve a directory (`root`, `index`) via `StaticAdapter` — see `[route.action.static]` below |
| `redirect` | 301/302 with a `Location` header; `$path` interpolated |
| `respond` | Fixed status + body (catch-all 404, maintenance page) |
| `mcp` | Built-in MCP Streamable HTTP server |

**`static` action example**

```toml
[route.action]
type = "static"

[route.action.static]
root  = "/var/www/site"        # absolute or relative to the process working directory
index = ["index.html"]         # tried in order for directory requests; defaults to ["index.html"]
```

Requests with a `..` path segment (before or after percent-decoding) return `403`; a
resolved path that doesn't exist returns `404`.

**Per-route middleware keys**

| Key | Example |
|---|---|
| `rate_limit` | `{ max_requests = 500, window_secs = 60 }` |
| `cache` | `{ ttl_secs = 3600, vary_by = ["Accept-Encoding"] }` |
| `auth` | `{ type = "bearer", token_env = "API_TOKEN" }` |
| `ip_allow` / `ip_deny` | `["192.168.1.0/24", "10.0.0.1"]` |
| `rewrite.request[]` | `[{ type = "header_set", name = "X-Real-IP", value = "$client_ip" }]` |
| `rewrite.response[]` | `[{ type = "header_remove", name = "Server" }]` |
| `timeout_ms` | `500` — flat key directly under `[route.middleware]`, not a sub-table. Bounds this route's *total* time (including its other middleware); `0`/absent means no timeout. |

**L4 proxies (separate listeners, spawned from config)**

```toml
[[tcp_proxy]]
name               = "postgres"
listen             = "0.0.0.0:5432"
backends           = ["db1:5432", "db2:5432"]
connect_timeout_ms = 500

[[udp_proxy]]
name             = "dns"
listen           = "0.0.0.0:53"
backends         = ["8.8.8.8:53", "1.1.1.1:53"]
reply_timeout_ms = 2000
buffer_size      = 8192

[[ws_proxy]]
name               = "chat"
listen             = "0.0.0.0:9000"
backends           = ["ws-backend:8080"]
connect_timeout_ms = 500
read_timeout_ms    = 30000
```

See [`spec/PROXY_SERVER_CONFIG.md`](spec/PROXY_SERVER_CONFIG.md) for the full annotated config reference.

### 53. Dependency injection

`Container` is a type-keyed service store. Register services at startup; resolve them in handlers via `Container` passed directly as `AppWithState`/`AsyncAppWithState`'s state — `Container` is `Send + Sync + 'static` like any other state type, so no special-cased integration or wrapper is needed.

**Concrete services**

```rust
use rust_web_server::di::Container;

struct DatabasePool { url: String }
struct EmailService { host: String }

let mut c = Container::new();
c.register(DatabasePool { url: "postgres://localhost/app".into() })
 .register(EmailService { host: "smtp.example.com".into() });

// In a handler:
let db = state.get::<DatabasePool>().unwrap();
let email = state.get::<EmailService>().unwrap();
```

**Trait objects**

Register under the trait type so handlers depend on the abstraction, not the implementation:

```rust
use std::sync::Arc;
use rust_web_server::di::Container;

pub trait UserRepository: Send + Sync {
    fn find(&self, id: u64) -> Option<String>;
}

pub struct PgUserRepository;
impl UserRepository for PgUserRepository {
    fn find(&self, id: u64) -> Option<String> {
        Some(format!("user-{}", id))
    }
}

let mut c = Container::new();
c.provide::<dyn UserRepository>(Arc::new(PgUserRepository));

// In tests, swap to a fake:
// c.provide::<dyn UserRepository>(Arc::new(FakeUserRepository));

let repo = c.get::<dyn UserRepository>().unwrap();
assert_eq!(repo.find(1).as_deref(), Some("user-1"));
```

**Named services** — multiple instances of the same type:

```rust
use rust_web_server::di::Container;

let mut c = Container::new();
c.register_named("primary", 5432u16)   // primary DB port
 .register_named("replica", 5433u16);  // replica DB port

assert_eq!(*c.get_named::<u16>("primary").unwrap(), 5432);
assert_eq!(*c.get_named::<u16>("replica").unwrap(), 5433);
```

**Wire into `App::with_state`**

Pass the container itself — **not** `container.into_arc()`. `App::with_state` already wraps its state in an `Arc` internally, so calling `.into_arc()` first just double-wraps it (`Arc<Arc<Container>>>`) for no benefit; handlers then receive `&Container` directly instead of `&Arc<Container>`.

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::di::Container;
use rust_web_server::routes;

fn get_user(
    req: &Request,
    params: &PathParams,
    _conn: &ConnectionInfo,
    state: &Container,
) -> Response {
    let repo = state.get::<dyn UserRepository>().unwrap();
    // use repo.find(...)
    Response::new()
}

let mut container = Container::new();
container.provide::<dyn UserRepository>(Arc::new(PgUserRepository));
// register more services...

let app = routes! {
    App::with_state(container),
    GET "/users/:id" => get_user,
};
```

**Wire into `App::with_async_state`**

Same pattern, `async fn` handlers (requires the `http2` feature):

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::di::Container;

let mut container = Container::new();
container.provide::<dyn UserRepository>(Arc::new(PgUserRepository));

let app = App::with_async_state(container)
    .get("/users/:id", |_req, params, _conn, state| async move {
        let repo = state.get::<dyn UserRepository>().unwrap();
        let id: u64 = params.get("id").unwrap_or("0").parse().unwrap_or(0);
        let _user = repo.find(id);
        Response::new()
    });
```

**API summary**

| Method | Effect |
|---|---|
| `register::<T>(value)` | Store concrete service; wraps in `Arc<T>` |
| `provide::<dyn Trait>(Arc::new(...))` | Store trait object |
| `register_named("name", value)` | Named concrete service |
| `provide_named("name", Arc::new(...))` | Named trait object |
| `get::<T>()` | Resolve concrete or trait object → `Option<Arc<T>>` |
| `get_named::<T>("name")` | Resolve named service |
| `contains::<T>()` | Check if registered |
| `into_arc()` | Seal into `Arc<Container>` for sharing |

---

### 54. Basic CRUD with the Model layer

Enable the `model-sqlite` (or `model-postgres` / `model-mysql`) feature in `Cargo.toml`:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["macros", "model-sqlite"] }
```

Define a model struct:

```rust
use rust_web_server::Model;

#[derive(Model, Debug, Clone)]
#[table(name = "users")]
pub struct User {
    #[primary_key(auto_increment)]
    pub id: i64,
    #[column(name = "first_name")]
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    #[ignore]
    pub display_label: String,   // not stored in DB
}
```

Open a connection pool and run migrations (all async; requires a tokio runtime — implied by any `model-*` feature):

```rust
use rust_web_server::model::{DbConfig, DbPool};

let pool = DbPool::new(DbConfig {
    host:      "localhost".into(),
    port:      5432,
    user:      "app".into(),
    password:  "secret".into(),
    database:  "myapp.db".into(),   // file path for SQLite
    pool_size: 5,
}).await?;

pool.migrate("migrations/").await?;
```

CRUD via the repository:

```rust
let repo = User::repository(&pool);

// INSERT
let alice = repo.save(&User { id: 0, name: "Alice".into(), email: "alice@example.com".into(), age: Some(30), display_label: "".into() }).await?;
println!("saved with id={}", alice.id);

// SELECT by PK
let found: Option<User> = repo.find_by_id(alice.id).await?;

// UPDATE
let mut updated = found.unwrap();
updated.age = Some(31);
repo.save(&updated).await?;

// DELETE
repo.delete_by_id(alice.id).await?;

// COUNT
let n: i64 = repo.count().await?;
```

Fluent query builder:

```rust
let page: Vec<User> = User::query(&pool)
    .filter("age >= ?", vec![rust_web_server::model::Value::Int(18)])
    .order_by("name", rust_web_server::model::Order::Asc)
    .limit(20)
    .offset(40)
    .fetch_all().await?;
```

Transactions:

```rust
pool.transaction(|mut tx| async move {
    let user = tx.execute("INSERT INTO users ...", &[...]).await?;
    // more work...
    tx.commit().await?;
    Ok(user)
}).await?;
```

---

### 55. Call an external API from a handler

Use `http_client::Client` to make outbound HTTP requests.  Plain HTTP works in
all builds.  HTTPS requires the `http-client` feature (or `http2` / `http3`
which already include it).

```toml
[dependencies]
rust-web-server = { version = "17", features = ["http-client"] }
```

```rust
use rust_web_server::http_client::{Client, HttpClientError};

fn fetch_user(id: u64) -> Result<String, HttpClientError> {
    let client = Client::new();
    let resp = client
        .get(&format!("https://api.example.com/users/{id}"))
        .header("Authorization", "Bearer tok_…")
        .timeout_ms(5_000)
        .send()?;

    if resp.is_success() {
        resp.text()
    } else {
        Err(HttpClientError(format!("upstream returned {}", resp.status())))
    }
}
```

POST with a JSON body:

```rust
let resp = Client::new()
    .post("https://api.example.com/charges")
    .body_json(r#"{"amount":1000,"currency":"usd"}"#)
    .header("Authorization", "Bearer sk_…")
    .send()?;

assert!(resp.is_success());
```

Async variant (requires `http2` feature):

```rust
use rust_web_server::http_client::AsyncClient;

async fn fetch_async(url: &str) -> Result<String, rust_web_server::http_client::HttpClientError> {
    AsyncClient::new().get(url).send().await?.text()
}
```

---

### 56. Password hashing

`crypto` feature — Argon2id with random salt. Store the PHC string; the salt is embedded so no separate column is needed.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["crypto"] }
```

```rust
use rust_web_server::crypto::{hash_password, verify_password, generate_token};

// At registration:
let hash = hash_password(&req.body_text()?)?;
// Store `hash` in the database.

// At login:
let ok = verify_password(&submitted_password, &stored_hash)?;
if !ok {
    return Ok(AppError::Unauthorized.into_response());
}

// Password reset / API key generation:
let reset_token = generate_token(32); // 64-char lowercase hex
```

---

### 57. CSRF protection

`csrf` feature — double-submit cookie pattern. Validates every mutating request (`POST`, `PUT`, `PATCH`, `DELETE`) against a `_csrf` cookie; passes safe methods through unconditionally.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["csrf"] }
```

Add `CsrfLayer` to the middleware stack:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::csrf::CsrfLayer;

let app = App::new().wrap(CsrfLayer::new());
```

Inside a `GET` handler, embed the token in the HTML form:

```rust
use rust_web_server::csrf::CsrfToken;
use rust_web_server::request::Request;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::Response;

fn show_form(req: &Request, _conn: &ConnectionInfo) -> Response {
    let token = CsrfToken::from_request(req)
        .map(|t| t.value().to_string())
        .unwrap_or_default();

    let html = format!(
        r#"<form method="POST" action="/submit">
  <input type="hidden" name="_csrf" value="{token}">
  <input type="text" name="email">
  <button type="submit">Subscribe</button>
</form>"#
    );
    // build your HTML response with `html` body
    Response::new()
}
```

For AJAX, read the cookie with JavaScript (cookie is not `HttpOnly` by default) and include it as a header:

```js
const token = document.cookie
    .split('; ')
    .find(c => c.startsWith('_csrf='))
    ?.split('=')[1];

fetch('/api/action', {
    method: 'POST',
    headers: { 'X-CSRF-Token': token },
    body: JSON.stringify(payload),
});
```

Options:

```rust
// Restrict to HTML forms only (disables JS cookie access):
CsrfLayer::new().http_only(true)

// Add Secure flag for production HTTPS deployments:
CsrfLayer::new().secure(true)

// Custom cookie / field / header names:
CsrfLayer::new()
    .cookie_name("xsrf")
    .field_name("xsrf")
    .header_name("X-XSRF-Token")
```

---

### 58. OAuth2 / OIDC SSO ("Sign in with Google / GitHub / …")

`sso` feature — authorization-code + PKCE flow, RS256/ES256 JWT verification via
JWKS, session-backed identity, and built-in login/callback/logout routes.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["sso"] }
```

**Minimal setup — Google:**

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::session::SessionStore;
use rust_web_server::sso::{OidcAuth, OidcConfig};

let sessions = Arc::new(SessionStore::new(86_400));

let app = App::new()
    .wrap(
        OidcAuth::new(
            OidcConfig::google(
                &std::env::var("GOOGLE_CLIENT_ID").unwrap(),
                &std::env::var("GOOGLE_CLIENT_SECRET").unwrap(),
                "https://myapp.com/auth/callback",
            ),
            Arc::clone(&sessions),
        )
        .exclude("/healthz")
        .exclude("/public/"),
    );
// OidcAuth automatically handles:
//   GET /auth/login    → redirect to Google
//   GET /auth/callback → exchange code, verify id_token, create session
//   GET /auth/logout   → destroy session
// All other routes require a session or are redirected to /auth/login.
```

**Access claims in any handler:**

```rust
use rust_web_server::sso::OidcAuth;
use rust_web_server::request::Request;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::Response;

fn dashboard(req: &Request, _conn: &ConnectionInfo) -> Response {
    let claims = OidcAuth::claims(req).expect("OidcAuth middleware is in the stack");
    // claims.sub      — unique user ID at the IdP
    // claims.email    — email address (Option<String>)
    // claims.name     — display name  (Option<String>)
    // claims.picture  — avatar URL    (Option<String>)
    todo!("build response")
}
```

**Load config from environment variables:**

```rust
// RWS_OIDC_PROVIDER=google RWS_OIDC_CLIENT_ID=... RWS_OIDC_CLIENT_SECRET=...
// RWS_OIDC_REDIRECT_URI=https://myapp.com/auth/callback
let config = OidcConfig::from_env().unwrap();
```

**Other built-in providers:**

```rust
// Microsoft Entra ID (Azure AD)
OidcConfig::microsoft("my-tenant-id", client_id, client_secret, redirect_uri)

// GitHub (OAuth 2.0 only — no OIDC; fetches user via /user API)
OidcConfig::github(client_id, client_secret, redirect_uri)

// Okta
OidcConfig::okta("mycompany.okta.com", client_id, client_secret, redirect_uri)

// Auth0
OidcConfig::auth0("mycompany.auth0.com", client_id, client_secret, redirect_uri)

// Keycloak
OidcConfig::keycloak("https://keycloak.example.com", "my-realm",
                     client_id, client_secret, redirect_uri)

// Any OIDC-compliant provider (fetches discovery doc once at startup)
OidcConfig::discover("https://idp.example.com", client_id, client_secret, redirect_uri)
    .unwrap()
```

**JWKS and JWT verification (standalone):**

```rust
use rust_web_server::sso::{JwksCache, VerifyOptions};

let cache = JwksCache::new("https://www.googleapis.com/oauth2/v3/certs");
// fetch() is called automatically on first use; re-fetches on key rotation
let claims = cache.verify_jwt(
    &id_token,
    &VerifyOptions {
        audience:    "my-client-id.apps.googleusercontent.com",
        issuer:      "https://accounts.google.com",
        leeway_secs: 30,
    },
)?;
```

**Custom login/callback paths:**

```rust
OidcAuth::new(config, sessions)
    .login_path("/sso/start")
    .callback_path("/sso/return")
    .logout_path("/sso/end")
```

---

### 59. SQLite in-memory database for tests and prototyping

`DbPool::memory()` and `DbConnection::memory()` are SQLite-only shortcuts that open a `":memory:"` database without configuring host, credentials, or a file path.

**`DbPool::memory()` — isolated in-memory database (SQLite only)**

Each call to `DbPool::memory().await` returns an independent empty database backed by a single connection. All async pool operations go through that connection. Ideal for tests where isolation is required:

```rust
use rust_web_server::model::{DbPool, Value};

// Each call is a new, isolated in-memory database
let pool = DbPool::memory().await.unwrap();

pool.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", &[]).await.unwrap();
pool.execute("INSERT INTO items (name) VALUES (?)", &[Value::Text("apple".into())]).await.unwrap();

let rows = pool.query_rows("SELECT name FROM items", &[]).await.unwrap();
assert_eq!(1, rows.len());
```

Enable with the `model-sqlite` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["model-sqlite"] }
```

---

### 60. Sending transactional email (SMTP)

Enable the `mailer` feature, set `RWS_SMTP_*` env vars, and call `Mailer::send()` from any handler. STARTTLS and SMTPS require the `http-client` or `http2` feature for TLS.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["mailer", "http-client"] }
```

**Sending a password-reset email**

```rust,no_run
use rust_web_server::mailer::{Email, Mailer};

// Read SMTP config from environment
let mailer = Mailer::from_env().expect("SMTP not configured");

let email = Email::builder()
    .to("user@example.com")
    .subject("Reset your password")
    .text("Click here to reset: https://example.com/reset?token=abc123")
    .html("<p>Click <a href=\"https://example.com/reset?token=abc123\">here</a> to reset.</p>")
    .build()
    .unwrap();

mailer.send(&email).expect("send failed");
```

**From a handler with shared mailer state**

```rust,no_run
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::mailer::{Email, Mailer};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

struct State {
    mailer: Arc<Mailer>,
}

let state = State {
    mailer: Arc::new(Mailer::from_env().unwrap()),
};

let app = App::with_state(state)
    .post("/register", |req, _params, _conn, state| {
        let email = Email::builder()
            .to("new_user@example.com")
            .subject("Welcome!")
            .text("Thanks for signing up.")
            .build()
            .unwrap();
        let _ = state.mailer.send(&email); // send in background in real code

        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r
    });
```

**Direct construction (no env vars)**

```rust,no_run
use rust_web_server::mailer::{Mailer, SmtpTls};

let mailer = Mailer {
    host: "smtp.sendgrid.net".into(),
    port: 587,
    user: Some("apikey".into()),
    password: Some("SG.xxxxx".into()),
    from: "noreply@example.com".into(),
    tls: SmtpTls::Starttls,
    timeout_ms: 10_000,
};
```

| Env variable | Default | Notes |
|---|---|---|
| `RWS_SMTP_HOST` | — (required) | Hostname of the SMTP server |
| `RWS_SMTP_PORT` | `587` | 25 = relay, 587 = STARTTLS, 465 = SMTPS |
| `RWS_SMTP_USER` | — | Omit to skip AUTH |
| `RWS_SMTP_PASSWORD` | — | |
| `RWS_SMTP_FROM` | — (required) | Envelope and `From:` address |
| `RWS_SMTP_TLS` | `starttls` | `starttls`, `smtps`, or `none` |
| `RWS_SMTP_TIMEOUT_MS` | `10000` | Connect / read / write timeout |

### 61. Isolated configuration in tests (`App::with_config`)

Tests that verify CORS, CSP, or security-header behavior should not read from environment variables — that causes races when `cargo test` runs in parallel. Use `App::with_config` to pin the app to a fixed `ServerConfig`:

```rust
use rust_web_server::app::App;
use rust_web_server::application::Application;
use rust_web_server::server_config::ServerConfig;
use rust_web_server::test_client::TestClient;
use rust_web_server::header::Header;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::http::VERSION;

#[test]
fn cors_denied_when_origin_not_listed() {
    // Build a ServerConfig with CORS allow-list enabled but no origins listed.
    // No env writes → no test_env::lock() needed → runs safely in parallel.
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(),
        ..ServerConfig::default()
    };
    let client = TestClient::new(App::with_config(config));

    let resp = client
        .options("/static/file.png")
        .header(Header::_ORIGIN, "https://evil.example.com")
        .send();

    // No ACAO header → CORS denied
    assert!(resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).is_none());
}

#[test]
fn cors_allowed_for_listed_origin() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: "https://trusted.example.com".to_string(),
        cors_allow_credentials: "true".to_string(),
        ..ServerConfig::default()
    };
    let client = TestClient::new(App::with_config(config));

    let resp = client
        .options("/static/file.png")
        .header(Header::_ORIGIN, "https://trusted.example.com")
        .header(Header::_ACCESS_CONTROL_REQUEST_METHOD, METHOD.get)
        .send();

    let acao = resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    assert_eq!("https://trusted.example.com", acao.value);
}
```

For tests that depend on filesystem paths (e.g. serving static files), continue using `App::handle_request` with `test_env::lock()` and `override_environment_variables_from_config`, since those tests need the config file to set the correct root directory.

**`AppWithState`, `AsyncAppWithState`, and `ConfigDrivenApp` all support the same pattern.** Each falls through to a built-in `App` for anything its own routes don't handle; by default that fallback is `App::new()` (reads `RWS_CONFIG_*` per request, same as if you'd called `App::new()` yourself). Call `.with_config(ServerConfig)` on any of them to pin that fallback instead:

```rust
use rust_web_server::state::AppWithState;
use rust_web_server::server_config::ServerConfig;
use rust_web_server::test_client::TestClient;

let config = ServerConfig { cors_allow_all: false, cors_allow_origins: String::new(), ..ServerConfig::default() };

// No env writes → no test_env::lock() needed, even though this app
// registers its own routes and falls through to App for everything else.
let app = AppWithState::new(())
    .with_config(config)
    .get("/version", |_req, _params, _conn, _state| Response::new());
let client = TestClient::new(app);
```

`AsyncAppWithState::with_config` (requires `http2`) and `ConfigDrivenApp::with_config` (config-driven proxy — call it on the `ConfigDrivenApp` returned by `build_from_file()`/`build()`) work identically.

### 62. Background job queue

Run one-shot work — "send this email after signup" — off the request path without blocking the response. Requires the `jobs` feature.

**In-memory queue**

```toml
[dependencies]
rust-web-server = { version = "17", features = ["jobs"] }
```

```rust
use rust_web_server::jobs::JobQueue;
use std::time::Duration;

// 4 worker threads; retry failed jobs up to 5 times with a 200ms initial backoff.
let queue = JobQueue::new(4)
    .max_retries(5)
    .backoff(Duration::from_millis(200), 2);

// In a request handler, after committing the signup:
let to = "new-user@example.com".to_string();
queue.submit(move || {
    // send_welcome_email(&to) — return Err(msg) to trigger a retry
    Ok(())
});
```

Named, stateful jobs implement `Job` directly instead of using a closure:

```rust
use rust_web_server::jobs::Job;

struct SendWelcomeEmail { to: String }

impl Job for SendWelcomeEmail {
    fn run(&self) -> Result<(), String> {
        // send_welcome_email(&self.to)
        Ok(())
    }
    fn name(&self) -> &str { "send_welcome_email" } // used in retry/failure log lines
}

queue.submit(SendWelcomeEmail { to: "new-user@example.com".to_string() });
```

**Persistent queue** (survives a crash/restart) — additionally requires a `model-*` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["jobs", "model-sqlite"] }
```

```rust,no_run
# async fn example() -> Result<(), rust_web_server::model::DbError> {
use rust_web_server::jobs::PersistentJobQueue;
use rust_web_server::model::DbPool;
use std::sync::Arc;

let pool = DbPool::from_env().await?;
let queue = Arc::new(PersistentJobQueue::new(pool).await?);

// Register every job_type this process enqueues *before* starting workers —
// this must also run on the process that restarts after a crash, since jobs
// are resumed from the `rws_jobs` table by job_type, not by closure.
queue.register("send_welcome_email", |payload /* the email address */| {
    // send_welcome_email(payload)
    Ok(())
});

let _worker_handles = Arc::clone(&queue).start(4); // 4 polling workers

// From a request handler:
queue.enqueue("send_welcome_email", "new-user@example.com").await?;
# Ok(())
# }
```

Unlike the in-memory queue, jobs here are `(job_type, payload)` strings, not closures — a closure can't survive a process restart, but a row in `rws_jobs` can. A job left `running` when the process crashes is reset to `pending` the next time `PersistentJobQueue::new` runs.

### 63. File / object storage abstraction

Store uploaded files on local disk in development and swap in S3-compatible object storage in production without changing handler code.

**Local disk** (requires the `storage-local` feature)

```toml
[dependencies]
rust-web-server = { version = "17", features = ["storage-local"] }
```

```rust
use rust_web_server::storage::{LocalStorage, Storage};

// `.with_base_url()` is only needed if you also serve `root` as a static
// directory and want `url()` to return an HTTP path instead of a filesystem path.
let store = LocalStorage::new("/var/data/uploads").with_base_url("/uploads");

// In a multipart upload handler, after FormMultipartData::parse():
let key = store.put("avatars/42.png", &file_bytes, "image/png")?;
let public_url = store.url(&key); // "/uploads/avatars/42.png"
```

**S3-compatible storage** (requires the `storage-s3` feature — AWS S3, Cloudflare R2, MinIO)

```toml
[dependencies]
rust-web-server = { version = "17", features = ["storage-s3"] }
```

```rust,no_run
use rust_web_server::storage::{S3Storage, Storage};

// Reads RWS_S3_BUCKET, RWS_S3_REGION, RWS_S3_ACCESS_KEY, RWS_S3_SECRET_KEY,
// and optionally RWS_S3_ENDPOINT (point this at R2 / MinIO / any S3-compatible host).
let store = S3Storage::from_env()?;

let key = store.put("avatars/42.png", &file_bytes, "image/png")?;
let public_url = store.url(&key);
# Ok::<(), rust_web_server::storage::StorageError>(())
```

Write handler code against the `Storage` trait so it works with either backend:

```rust
use rust_web_server::storage::Storage;

fn save_avatar(store: &dyn Storage, user_id: u64, bytes: &[u8]) -> Result<String, rust_web_server::storage::StorageError> {
    let key = format!("avatars/{user_id}.png");
    store.put(&key, bytes, "image/png")
}
```

`S3Storage` signs every request with AWS Signature Version 4 using the existing outbound HTTP client — no AWS SDK dependency. Path-style addressing (`{endpoint}/{bucket}/{key}`) is used throughout, since it works against every S3-compatible provider including custom endpoints where virtual-hosted-style (`{bucket}.{host}`) DNS isn't set up.

### 64. OpenAPI / Swagger documentation for your API

Generate an OpenAPI 3.0 spec and a browsable Swagger UI directly from your registered routes — no separate spec file to keep in sync. Requires the `openapi` feature.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["openapi"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::openapi::OpenApiConfig;
use rust_web_server::response::Response;
use rust_web_server::core::New;

struct Db;

let app = App::with_state(Db)
    .get("/users", |_req, _params, _conn, _db| Response::new())
    .get("/users/:id", |_req, _params, _conn, _db| Response::new())
    .post("/users", |_req, _params, _conn, _db| Response::new())
    // Register .openapi() last — it snapshots routes registered so far.
    .openapi(OpenApiConfig::new("My API", "1.0.0").description("Example API"));
```

This adds two routes:

- `GET /openapi.json` — the generated OpenAPI 3.0.3 document (`Content-Type: application/json`)
- `GET /docs` — Swagger UI (loaded from the `unpkg.com/swagger-ui-dist` CDN), pointed at `/openapi.json`

`AsyncAppWithState::openapi(config)` works identically for apps with `async fn` handlers (requires `http2`).

**Scope**: paths, HTTP methods, and path parameters only. `:id` and `*path` segments both become `{id}`/`{path}` in the OpenAPI path template, with a `parameters` entry (`in: "path"`, `type: "string"`). Every operation is documented with a generic `200 OK` response and no request/response body schema — Rust has no runtime type reflection, so extracting a JSON Schema from a `#[derive(Validate)]` struct or a plain Rust type isn't something this can do without a much larger, separate code-generation feature. If you need full body schemas today, generate them by hand into the same `paths` structure, or post-process `build_spec`'s output.

```rust
use rust_web_server::openapi::{build_spec, OpenApiConfig};
use rust_web_server::router::RouteInfo;

// Build the spec string directly from a route list (e.g. to inspect or
// post-process it, without wiring it into an app):
let routes = vec![RouteInfo { method: "GET".to_string(), pattern: "/users/:id".to_string() }];
let spec_json = build_spec(&OpenApiConfig::new("My API", "1.0.0"), &routes);
```

### 65. Per-route timeouts

A single global read timeout applies to every route by default. A file-upload endpoint may need 120 s while a health check should fail fast at 500 ms. Wrap individual handlers with `crate::timeout`'s helpers to give them their own budget.

**`Router` / stateless handlers**

```rust
use rust_web_server::router::Router;
use rust_web_server::timeout::with_timeout;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

let router = Router::new()
    .get("/healthz", with_timeout(Duration::from_millis(500), |_req, _params, _conn| Response::new()))
    .post("/upload", with_timeout(Duration::from_secs(120), |_req, _params, _conn| Response::new()));
```

**`AppWithState<S>`** — requires `S: Clone` (the wrapped call runs on a background thread and needs its own owned copy of the state, since the handler only receives `&S`):

```rust
use rust_web_server::app::App;
use rust_web_server::timeout::with_timeout_state;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

#[derive(Clone)]
struct Db; // holds e.g. an Arc<DbPool> internally — cheap to clone

let app = App::with_state(Db)
    .get("/healthz", with_timeout_state(Duration::from_millis(500), |_req, _params, _conn, _db| Response::new()))
    .post("/upload", with_timeout_state(Duration::from_secs(120), |_req, _params, _conn, _db| Response::new()));
```

**`AsyncAppWithState<S>`** — no `Clone` bound needed (state is already passed as an owned `Arc<S>`), and genuine cancellation via `tokio::time::timeout` (requires `http2`):

```rust
use rust_web_server::app::App;
use rust_web_server::timeout::with_timeout_async;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

struct Db;

let app = App::with_async_state(Db)
    .post("/upload", with_timeout_async(Duration::from_secs(120), |_req, _params, _conn, _db| async {
        Response::new()
    }));
```

**Config-driven proxy** — `timeout_ms` as a flat key under `[route.middleware]`:

```toml
[[route]]
name = "slow-upload"

[route.match]
path = "/upload"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "backend"

[route.middleware]
timeout_ms = 120000
```

**The honest limitation, in one place**: Rust cannot forcibly stop a running synchronous thread. `with_timeout`, `with_timeout_state`, and `TimeoutLayer` (used internally for the config-driven proxy case) all run the wrapped work on a background thread and return `504 Gateway Timeout` to the caller as soon as the deadline passes — but if the handler ignores its deadline, it keeps running to completion in the background; its result is just discarded. This bounds the **client's** wait time, not the handler's actual resource usage. Only `with_timeout_async` (backed by `tokio::time::timeout`) gets genuine cancellation, because dropping a suspended `Future` actually stops it at its next `.await` point.

### 66. Correlating log lines with a request ID

Wrap the app with `RequestIdLayer` so every request/response pair carries a stable ID your handlers and logs can reference, and that follows a request across service boundaries when the caller already sends one.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::request_id::RequestIdLayer;

let app = App::new().wrap(RequestIdLayer::new());
```

- If the incoming request already has an `X-Request-Id` header (set by an upstream gateway, load balancer, or calling service), that exact value is kept and echoed back — one ID follows the request across every hop instead of getting a new one at each service.
- Otherwise, a fresh ID (`generate_request_id()`, UUID-v4-shaped but not cryptographically random — fine for correlating logs, not for security tokens) is generated and injected into the request *before* your handler runs.
- Either way, the same value is always set on the response.

Read it in a handler with the `RequestId` extractor, or any of the usual header-reading paths:

```rust
use rust_web_server::extract::{FromRequest, RequestId};
use rust_web_server::request::Request;

fn handler(request: &Request) {
    let id = RequestId::from_request(request).unwrap();
    println!("[{}] handling request", id.as_str());
}
```

Use a different header (e.g. to match an existing convention) with `.header(...)`:

```rust
use rust_web_server::request_id::RequestIdLayer;

let layer = RequestIdLayer::new().header("X-Correlation-Id");
```

`RequestIdLayer` composes with other middleware the same way any layer does — put it outermost (registered first via `.wrap()`, so it wraps everything else) if you want the same ID visible to every other middleware in the stack, including `OtelLayer` or your own access-logging middleware.
