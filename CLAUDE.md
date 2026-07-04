# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build (http3 is the default — includes HTTP/3, HTTP/2, and TLS)
cargo build

# Build HTTP/2 + TLS only (no QUIC/HTTP/3)
cargo build --no-default-features --features http2

# Build HTTP/1.1-only (no TLS, lightest binary)
cargo build --no-default-features --features http1

# Run (falls back to plain HTTP/1.1 if no cert is configured)
cargo run

# Run with HTTPS + HTTP/2 + HTTP/3 active
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem

# Run all tests
cargo test

# Run a single test (replace the test path with the one you want)
cargo test --package rust-web-server --bin rws client_hint::tests::client_hints_header -- --exact
```

MSRV is 1.75. The `--ignore-rust-version` flag from older docs is no longer needed.

## Required for every change

Every code change — new feature, bug fix, refactor — must include all three:

### 1. Tests

- Add tests in a `tests.rs` sibling file (e.g. `src/csrf/tests.rs`) or inline `#[cfg(test)]` block.
- Use `TestClient::new(app)` for integration-style tests; pure unit tests for helpers and pure functions.
- Cover the happy path and at least one failure/edge case per public function or method.
- Run `cargo test` before committing. All tests must pass.

#### Shared-state lock — mandatory for any test that touches `RWS_CONFIG_*` env vars

`cargo test` runs all tests in parallel within one process. `RWS_CONFIG_*` environment variables are process-wide mutable state. Any test that **writes** them — directly or transitively — races with every other test that reads them, causing intermittent failures that are hard to reproduce.

**Rule:** hold `let _g = crate::test_env::lock();` as the very first line of every test that does any of the following (directly or via a called function):

| Trigger | Why it writes env vars |
|---|---|
| `std::env::set_var("RWS_CONFIG_*", …)` | direct write |
| `std::env::remove_var("RWS_CONFIG_*")` | direct remove |
| `override_environment_variables_from_config(…)` | reads a `.toml` file and writes all keys it finds, including `thread_count` |
| `bootstrap()` | calls `override_environment_variables_from_config(None)` which reads the project-root `rws.config.toml` |
| `config_reload::reload()` | calls `override_environment_variables_from_config(None)` |
| `CommandLineArgument::_parse(…)` / `set_environment_variable(…)` | calls `env::set_var` for each parsed arg |
| `metrics::SERVER_READY.store(…)` | mutates a global `AtomicBool` read by `/readyz` tests |

The lock is a `static OnceLock<Mutex<()>>` defined in `src/lib.rs` under `#[cfg(test)]`. It must be held for the **entire** test body — assign it to a named binding (`_g`, not `_`) so it isn't dropped immediately:

```rust
#[test]
fn my_test() {
    let _g = crate::test_env::lock();   // held until end of function
    override_environment_variables_from_config(Some("/src/test/rws.config.toml"));
    // ... rest of test
}
```

**The transitive trap.** The lock is required even when a test doesn't call `set_var` directly. `bootstrap()` looks innocent but it calls `override_environment_variables_from_config(None)`, which finds the project-root `rws.config.toml` (which sets `thread_count = 200`) and writes it into the process environment. A test that calls `bootstrap()` without the lock will silently corrupt `RWS_CONFIG_THREAD_COUNT` for any concurrently running test that just set it to something else.

When adding a new test, ask: *does any function I call eventually call `env::set_var`?* If yes, take the lock.

#### Preferred alternative: `App::with_config` for CORS/CSP tests

Tests that only verify CORS, CSP, or other header behavior (not filesystem-dependent paths) should prefer the lock-free pattern:

```rust
#[test]
fn my_cors_test() {
    // No env writes, no lock needed — runs safely in parallel.
    use crate::server_config::ServerConfig;
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: "https://example.com".to_string(),
        ..ServerConfig::default()
    };
    let app = App::with_config(config);
    let client = crate::test_client::TestClient::new(app);
    // ... assertions
}
```

`App::with_config(config)` pins the app to a fixed `ServerConfig` — no env reads happen during request processing, so no lock is needed even under parallelism. The `test_env::lock()` pattern is still required for tests that call `bootstrap()`, `override_environment_variables_from_config()`, or any function that writes `RWS_CONFIG_*` vars.

### 2. DEVELOPER.md

- **Building blocks table** — add a row for every new public type, function, or middleware: `| Name | Module | What it does |`
- **Use cases** — add a numbered use-case section with a minimal, runnable code example showing the feature in context. Follow the existing `## Use Case #N: Title` heading pattern.

### 3. README.md + llms.txt

- Add a bullet or table row to the relevant "What's in the box" / "Optional features" section of `README.md`.
- Update `llms.txt` — add the new type/function to the relevant section (API surface, middleware table, module index, security checklist, etc.). `llms.txt` is the primary LLM discovery document; keep it current.

### 4. docs/ (Astro site)

- Update or create the relevant page under `docs/src/content/docs/`. Pages are organized by section: `building-apps/`, `features/`, `proxy/`, `database/`, `deployment/`, `reference/`, etc.
- For a new feature, add a new `.md` or `.mdx` file in the appropriate section and register it in the Astro sidebar if needed (`docs/astro.config.mjs`).
- For an existing feature, update the page that covers it — add a new `##` section, update examples, revise the description.
- Keep examples in docs consistent with examples in `DEVELOPER.md`.

Skipping any of these four is not acceptable. Docs and tests ship with the code, not after.

## Architecture

The default build (`http3` feature) uses a tokio async runtime and serves HTTP/3 over QUIC, HTTP/2, and HTTP/1.1 over TLS. The `http1`-only build is a fully synchronous, thread-pool-based server with no async runtime. All HTTP parsing, JSON, CORS, MIME types, range requests, WebSocket, SSE, and routing are implemented from scratch with no third-party HTTP dependencies.

### Request lifecycle

`main.rs` → `Server::setup()` binds the TCP listener and creates the `ThreadPool` → `Server::run()` (plain HTTP/1.1) or `Server::run_tls()` (TLS) accepts connections → dispatches each to the thread pool → `Server::process()` implements an HTTP/1.1 keep-alive loop: reads bytes, calls `Request::parse()`, calls `app.execute()`, applies gzip compression (`compression::apply_gzip()`), records metrics, sets `Connection` header, then writes the response. For large files `response.stream_file` triggers `Server::write_chunked_file()` instead.

Each connection gets a 30-second read timeout. `Server::run_redirect()` optionally listens on a second port and issues `301` redirects to HTTPS.

### Routing / controller dispatch

`App` (in `src/app/mod.rs`) implements the `Application` trait (`src/application/mod.rs`). `Application::execute` returns `Result<Response, String>`. `App::execute` walks a hardcoded list of `if Controller::is_matching(...)` checks and calls the first matching controller's `process()`. Controllers are checked in declaration order.

The `Controller` trait (`src/controller/mod.rs`) has two methods:
- `is_matching(request, connection) -> bool`
- `process(request, response, connection) -> Response`

Three ways to add routes:
1. **Controller pattern** — create a module under `src/app/controller/`, implement `Controller`, register it in `App::execute`.
2. **Router** — build a `Router` inside `Application::execute` and call `router.handle(request, connection)`. Prefer this for new code when you need named path parameters or don't want boilerplate controllers.
3. **State-aware app** — `App::with_state(S)` returns `AppWithState<S>` which has `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()` builder methods accepting `fn(state, request, path_params, connection) -> Response`. `App::with_async_state(S)` gives the same API with `async fn` handlers (requires `http2` feature). Wrap with `.wrap(layer)` to add middleware.

### Application variants

- `App` — zero-config, wraps all built-in controllers. Entry point: `App::new()`.
- `AppWithState<S>` (`src/state/mod.rs`) — state-aware dynamic router; `state: Arc<S>` shared across handlers. Entry point: `App::with_state(S)`.
- `AsyncAppWithState<S>` (`src/async_state/mod.rs`) — same as `AppWithState` but handlers are `async fn`; requires `http2` feature. Entry point: `App::with_async_state(S)`.
- `WithMiddleware<A>` (`src/middleware/mod.rs`) — wraps any `Application` with a middleware stack. Entry point: `app.wrap(layer)`.
- `McpServer` (`src/mcp/mod.rs`) — implements `Application`; serves the MCP Streamable HTTP protocol at `POST /mcp`. Entry point: `app.mcp(name, version)` or `McpServer::new(name, version)`.

### Middleware

`src/middleware/mod.rs` defines the `Middleware` trait:
```rust
trait Middleware: Send + Sync {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String>;
}
```
`WithMiddleware<A>::wrap(layer)` pushes layers onto a `Vec<Box<dyn Middleware>>`; layers are applied in push order (first-pushed is outermost). Any `Application` can be wrapped via `.wrap(layer)`.

Built-in middleware: `RateLimitLayer`, `MetricsLayer`, `CacheLayer`, `OtelLayer`, `RewriteLayer`, `ReverseProxy`, `H2ReverseProxy`, `GrpcProxy`, `BasicAuthLayer`, `JwtLayer`, `ForwardAuthLayer`, `IpFilter`, `RequestIdLayer`, `TimeoutLayer`.

### Configuration

Configuration is layered (lowest → highest priority):
1. Defaults hardcoded in `src/entry_point/mod.rs` (`Config::*_DEFAULT_VALUE`)
2. System environment variables
3. `rws.config.toml` in the working directory
4. Command-line args (`rws.command_line`)

All config is read at startup into process environment variables (`RWS_CONFIG_*`) and then accessed globally via `env::var(...)`. There is no config struct passed around at runtime.

Key config constants (all in `src/entry_point/mod.rs`):
- `RWS_CONFIG_IP`, `RWS_CONFIG_PORT`, `RWS_CONFIG_THREAD_COUNT`
- `RWS_CONFIG_TLS_CERT_FILE`, `RWS_CONFIG_TLS_KEY_FILE`
- `RWS_CONFIG_TLS_CLIENT_CA_FILE` — mTLS CA cert; empty disables client cert verification
- `RWS_CONFIG_HTTP_REDIRECT_PORT` — if set, `Server::run_redirect()` sends `301` to HTTPS on this port
- `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS`, `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS`
- `RWS_CONFIG_LOG_FORMAT` — `"combined"` (default) or `"json"`
- `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` — per-read chunk size (default 10000); bodies larger than this span multiple reads automatically
- `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` — max accepted request body size (default `0`, unlimited); `413 Payload Too Large` before buffering when exceeded, checked in `Server::process`, `process_h1_tls`, `h2_handler`, and `h3_handler`

Hot reload: `SIGHUP` (or `POST /admin/config/reload`) calls `config_reload::reload()` which re-reads CORS rules, rate limits, log format, and request allocation size. On TLS builds, SIGHUP also rebuilds the `TlsAcceptor` from updated certs for all virtual hosts.

### Key types

- `Request` (`src/request/mod.rs`) — method, request_uri, http_version, headers, body (raw bytes)
- `Response` (`src/response/mod.rs`) — status code, headers, body as `Vec<ContentRange>`, and `stream_file: Option<String>` for chunked file streaming
- `Header` (`src/header/mod.rs`) — name/value pair; constants for all standard header names
- `ConnectionInfo` (`src/server/mod.rs`) — client/server IP+port, request allocation size, and `sni_hostname: Option<String>` (SNI hostname from TLS handshake, `None` for plain HTTP), passed into every controller

### HTTP version constants

`src/http/mod.rs` defines `VERSION` (HTTP/0.9 through HTTP/3.0) and `HTTP::version_list()` which is used by `Request::parse_method_and_request_uri_and_http_version_string` to validate the version token on every incoming request.

### Dynamic router

`src/router/mod.rs` provides `Router` — a fluent, path-based router with named parameters and wildcards. Register handlers with `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()`, then call `router.handle(request, connection)` from inside `Application::execute`. Pattern syntax: `:name` for a named segment, `*name` for a trailing wildcard. `PathParams::get(name)` retrieves extracted values.

Call `.with_host("example.com")` before registering routes to restrict a `Router` to requests whose SNI hostname (TLS) or `Host` header (plain HTTP) matches that value — the foundation for virtual-host routing.

### Typed extractors

`src/extract/mod.rs` defines the `FromRequest` trait and built-in extractors:
- `Body` — raw bytes (never fails)
- `BodyText` — UTF-8 body (400 on invalid UTF-8)
- `Query` — parsed query string as `HashMap<String, String>`
- `RequestHeaders` — all request headers with case-insensitive `get(name)`

### Typed errors

`src/error/mod.rs` defines:
- `IntoResponse` trait — implement on your error enum to map it to a `Response`; `Response` itself is the identity implementation
- `AppError` enum — covers `BadRequest`, `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `UnprocessableEntity`, `TooManyRequests`, `Internal`; all implement `IntoResponse`

### Rate limiting

`src/rate_limit/mod.rs` provides `RateLimiter` — a thread-safe sliding-window limiter keyed by a string (typically the client IP). Call `limiter.check(key)` → `true` if within budget, `false` to return 429. `rate_limit::global()` returns a process-wide singleton configured via `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` (default 1000) and `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` (default 60).

### Metrics

`src/metrics/mod.rs` — global counters incremented directly in `Server::process()` and `dispatch_connection()`:
- `record_request()` — total requests
- `record_error()` — total errors
- `connection_open()` / `connection_close()` — current connection gauge
- `SERVER_READY: AtomicBool` — cleared on shutdown; checked by `/readyz`

`MetricsLayer` middleware records per-route `rws_route_requests_total{method,path,status}` counters and `rws_route_duration_seconds{method,path}` histograms. The built-in `GET /metrics` endpoint exposes all metrics in Prometheus text format.

### MCP server

`src/mcp/mod.rs` — `McpServer` implements `Application` and serves the MCP Streamable HTTP protocol (`POST /mcp`, JSON-RPC 2.0). Register tools, resources, and prompts via builder methods: `.tool(name, description, schema, handler)`, `.resource(uri_template, name, description, handler)`, `.prompt(name, description, handler)`. `.require_bearer(token)` gates all requests behind a static Bearer token. `.wrap(app)` falls through non-MCP requests to another `Application`. The binary ships 8 built-in rws-specific tools (`server_config`, `feature_flags`, `server_metrics`, `rate_limit_config`, `check_rate_limit`, `cors_config`, `list_static_files`, `reload_config`).

### Test client

`src/test_client/mod.rs` provides `TestClient<A>` — dispatches requests directly through an `Application` without opening a TCP socket. Use it in unit and integration tests:

```rust
let client = TestClient::new(App::new());
let res = client.get("/healthz").send();
assert_eq!(200, res.status());
```

### Graceful shutdown

HTTP/1.1 (`http1` feature, `ctrlc` dep): a `SIGINT`/`SIGTERM` handler sets an `AtomicBool`; the accept loop exits and the `ThreadPool` drains in-flight requests before the process exits.

HTTP/2 + HTTP/3 (async features): `tokio::signal` handles `SIGINT` and `SIGTERM`; each `run_tls`/`run_quic` loop `select!`s on the signal and breaks cleanly. `SERVER_READY` is cleared before breaking.

### No async (`http1` feature only)

When compiled with `--no-default-features --features http1`, the server uses `std::net::TcpListener` and a hand-rolled `ThreadPool` (`src/thread_pool/mod.rs`). Each connection is handled synchronously on a worker thread. No tokio, no async.

### HTTP/2 feature (`--features http2`)

When built with the `http2` feature, the binary uses a `tokio` runtime and serves TLS via `rustls` (aws-lc-rs crypto backend). ALPN negotiation selects HTTP/2 (`h2` crate) or HTTP/1.1 per connection automatically. New modules:

- `src/tls/mod.rs` — `SniCertResolver` implements `rustls::server::ResolvesServerCert`; `create_tls_acceptor_from_vhosts(vhosts, default_cert, default_key)` builds a multi-domain `TlsAcceptor` that picks the right cert per SNI hostname at handshake time. `create_tls_acceptor(cert, key)` is a backward-compat wrapper for single-cert deployments. When `RWS_CONFIG_TLS_CLIENT_CA_FILE` is set, `load_client_verifier()` builds a `WebPkiClientVerifier` that enforces mTLS on both HTTPS and QUIC listeners.
- `src/virtual_host/mod.rs` — `VirtualHostConfig { domain, cert_file, key_file }` carries per-domain cert configuration.
- `src/h2_handler/mod.rs` — translates `h2::RecvStream` requests into `crate::request::Request`, calls `app.execute()`, translates `Response` back into H2 frames. The `Application` and `Controller` traits are untouched.
- `src/server/mod.rs::Server::run_tls()` — async accept loop; extracts SNI hostname after the TLS handshake (`tls_stream.get_ref().1.server_name()`), routes by ALPN to `h2_handler::handle_connection` or `Server::process_h1_tls`, populates `ConnectionInfo::sni_hostname`.
- `src/server/mod.rs::Server::run_redirect()` — optional second listener; sends `301 Moved Permanently` to HTTPS for every request; activated by `RWS_CONFIG_HTTP_REDIRECT_PORT`.
- HTTP/1.1 TLS responses include `Alt-Svc: h3=":PORT"` (or `h2` when http3 feature is absent) to advertise protocol availability.
- Forbidden HTTP/2 headers (`connection`, `keep-alive`, `transfer-encoding`, `upgrade`, `proxy-connection`, `te`) are stripped from responses before sending.
- SIGHUP: reloads hot config via `config_reload::reload()` AND rebuilds `TlsAcceptor` with fresh certs for all virtual hosts.

### HTTP/3 feature (`--features http3`, default)

When built with the `http3` feature (which implies `http2`), a second listener starts over UDP using QUIC (`quinn` crate). HTTP/3 uses `h3` + `h3-quinn`. New additions:

- `src/tls/mod.rs::create_quinn_server_config_from_vhosts()` — builds a multi-domain `quinn::ServerConfig` via the same `SniCertResolver`; `create_quinn_server_config()` is the single-cert wrapper.
- `src/h3_handler/mod.rs` — accepts QUIC connections, extracts SNI from `conn.handshake_data()?.downcast::<HandshakeData>()`, resolves H3 streams via `RequestResolver::resolve_request()`, calls `app.execute()`, sends H3 responses.
- `src/server/mod.rs::Server::run_quic()` — binds a UDP endpoint on the same port as TCP; skipped silently if no cert is configured.
- `main()` runs `Server::run_tls`, `Server::run_quic`, and `Server::run_redirect` concurrently via `tokio::join!`.

### Proxy modules

- `src/proxy/mod.rs` — `ReverseProxy` (HTTP/1.1 reverse proxy middleware); `H2ReverseProxy` (HTTP/2 upstream proxy, `http2` feature, bridges sync middleware into the tokio runtime via `crate::async_bridge::block_on_isolated` — a scoped OS thread with its own single-threaded runtime, which works under any tokio runtime flavor including `current_thread`); `GrpcProxy` (wraps `H2ReverseProxy`, filters on `Content-Type: application/grpc*`). `Backend::parse()` strips `h2://` and `http://` prefixes.
- `src/async_bridge/mod.rs` (`http2` feature) — `block_on_isolated(f)`: runs an async closure to completion regardless of whether the calling thread is already inside a tokio runtime. Used by both `H2ReverseProxy` and `AsyncAppWithState::execute` to bridge their sync trait methods (`Middleware`/`Application`) into async code without `tokio::task::block_in_place`'s `multi_thread`-only requirement.
- `src/tcp_proxy/mod.rs` — `TcpProxy` standalone L4 TCP proxy; `bind(addr)` blocks and accepts connections; `relay(client)` spawns two threads doing `std::io::copy` for bidirectional byte relay; round-robin backend selection via `AtomicUsize`.
- `src/udp_proxy/mod.rs` — `UdpProxy` standalone UDP datagram proxy; per-datagram thread model; ephemeral socket per datagram connects to the backend; `set_read_timeout()` controls reply wait; round-robin backend selection.
- `src/ws_proxy/mod.rs` — `WsProxy` standalone WebSocket proxy; reads the HTTP upgrade request from the client, connects to the backend, exchanges upgrade handshake, then relays raw bytes bidirectionally in a two-thread loop.

### Dependency injection

`src/di/mod.rs` — `Container` is a type-keyed service store backed by `HashMap<TypeId, Box<dyn Any + Send + Sync>>`. Services are stored as `Arc<T>` or `Arc<dyn Trait>` (both are `Sized`, `'static`, `Any + Send + Sync` and can be stored in the map). Registration: `register::<T>(value)` wraps in `Arc<T>`; `provide::<T: ?Sized>(Arc<T>)` stores trait objects directly. Both keyed by `TypeId::of::<T>()`. Named services use a second map keyed by `(TypeId, String)`. Resolution: `get::<T>()` (works for both concrete and `dyn Trait`) and `get_named::<T>(name)`. `into_arc()` seals the container for sharing. No external dependencies.

### Config-driven proxy server

`src/proxy_config/` — turns `rws.config.toml` into a live proxy stack at startup; activated when the config file contains `[[route]]` or `[[upstream]]` sections.

- `mod.rs` — all config types (`ProxyConfig`, `UpstreamConfig`, `RouteConfig`, `ActionConfig`, `MiddlewareConfig`, `AuthConfig`, etc.); `ProxyConfig::is_proxy_mode()` scans the file; `ProxyConfig::load()` and `ProxyConfig::from_str()` parse it. `ConfigDrivenApp { routes: Arc<Vec<CompiledRoute>>, fallback: App }` implements `Application + Clone` (first-match router). `RouteMatcher` evaluates host, path prefix/exact, method, content-type prefix. `DynamicProxy` picks a live backend from `Arc<RwLock<Vec<String>>>` with an atomic round-robin counter. `RedirectAdapter` and `RespondAdapter` handle redirect/fixed-response actions. `PerRouteRateLimit` and `BearerAuthMiddleware` implement `Middleware`. `route.middleware.auth` accepts `AuthConfig::Bearer { token_env }`, `AuthConfig::Jwt { secret_env }` (wraps `auth::JwtLayer`, requires the `auth` feature), or `AuthConfig::Basic { htpasswd_file }` (wraps `auth::BasicAuthLayer::from_htpasswd_file`, requires the `auth` feature) — `builder.rs::apply_middleware()` returns a config error if the feature isn't compiled in.
- `parser.rs` — hand-rolled TOML parser producing `SectionMap` (`HashMap<String, Vec<(String, String)>>`). Tracks `[[array]]` tables with counters (`upstream[0]`, `route[0].match`), expands inline tables and arrays of strings.
- `health.rs` — `start_health_checker()` spawns a daemon thread per upstream; sends `GET {path} HTTP/1.1` with connect+read timeouts, tracks consecutive pass/fail, writes updated live-backend list via `RwLock`.
- `builder.rs` — `build_from_file()` / `build(config)`: creates upstream live-backend pools, starts health-checker threads, compiles routes (applies middleware stack via `apply_middleware()`), spawns L4/WS proxy threads. `ArcApp` adapter wraps `Arc<dyn Application>` so `WithMiddleware::new()` can take ownership.
- `main()` (all three feature variants) checks `ProxyConfig::is_proxy_mode()` before calling `build_app()`; if true, calls `build_from_file()` and passes the resulting `ConfigDrivenApp` to `Server::run` / `run_tls` / `run_quic`.

### Rewrite middleware

- `src/rewrite/mod.rs` — `RewriteLayer` implements `Middleware`; clones `Request` and applies `RequestRule` variants (header set/remove, URI set/strip-prefix/add-prefix) before dispatch, then applies `ResponseRule` variants (header set/remove, status override, body byte find-and-replace) on the way back. Private `replace_bytes()` does linear non-overlapping scan.

### Authentication middleware (`auth` feature)

`src/auth/mod.rs` — gated behind the `auth` Cargo feature (adds `hmac` + `sha2`, RustCrypto). All base64/JWT logic is hand-rolled — no third-party JWT or base64 crate.

- `BasicAuthLayer<F>` — validates `Authorization: Basic` against a `Fn(&str, &str) -> bool` closure; `BasicAuthLayer::from_htpasswd_file(path)` loads an htpasswd-style file once at construction (supports plain-text and rws's own `{SHA256}` scheme — not Apache's real `{SHA}`/`$apr1$`/bcrypt hashes). Issues `401` with a `WWW-Authenticate` challenge when the header is missing/malformed, `401` without a challenge when validation fails.
- `JwtLayer` — verifies `Authorization: Bearer <jwt>` signed HS256; rejects expired (`exp`) tokens. `build_jwt(claims_json, secret)` and `verify_jwt(token, secret) -> Option<Claims>` are public helpers for issuing/inspecting tokens outside the middleware (e.g. a login handler). `extract_bearer_token(request)` pulls the raw token string.
- `src/auth/forward.rs` — `ForwardAuthLayer` (Traefik/nginx-style forward-auth): forwards every request header as a `GET` to an external auth URL via `http_client::Client`. 2xx → request proceeds, with any `.copy_header(name)`-listed response header replacing the same-named request header (e.g. an auth service resolving a cookie to `X-User-Id`). Any other status → the auth service's response (status/headers/body) is passed through to the client verbatim, preserving `WWW-Authenticate`/`Location`/custom bodies. Unreachable auth service → `502 Bad Gateway` (fails closed). `.timeout_ms()` defaults to 5000ms.

### Request timeouts

`src/timeout/mod.rs` — per-route timeouts layered on top of the single global 30s connection read timeout. Rust cannot forcibly preempt a running synchronous handler, so the sync-side helpers (`with_timeout`, `with_timeout_state` for `Router`/`AppWithState` handlers, and `TimeoutLayer` for wrapping a whole `Application`) run the wrapped work on a background thread and bound how long they *wait*: past the deadline the caller gets `504 Gateway Timeout` immediately, but the spawned thread keeps running to completion with its result discarded. Only `with_timeout_async` (requires `http2`, for `AsyncAppWithState`) achieves genuine cancellation — dropping the `Future` via `tokio::time::timeout` actually stops it at its next `.await`.

### Request ID / correlation ID middleware

`src/request_id/mod.rs` — `RequestIdLayer` ensures every request/response pair carries a stable ID in a header (default `X-Request-Id`, override via `.header(name)`). If the incoming request already has the header (set by an upstream gateway), that value is echoed back unchanged so one ID follows a request across hops; otherwise `generate_request_id()` mints a UUID-v4-*shaped* (not spec-compliant, not cryptographically random) ID from a monotonic counter + timestamp splitmix64 finalizer. Works without OpenTelemetry configured, unlike `otel`'s span-based tracing.

### OpenAPI generation (`openapi` feature)

`src/openapi/mod.rs` — generates a minimal OpenAPI 3.0.3 JSON document directly from `Router::route_entries()` (paths, methods, path parameters only — no request/response body schemas, since Rust has no runtime type reflection). `build_spec(&OpenApiConfig, &[RouteInfo])` merges routes sharing a path into one `paths` entry and converts `:name`/`*name` segments to `{name}`. `swagger_ui_html(spec_url)` returns a self-contained Swagger UI page loading `swagger-ui-dist` from a CDN. Wired in via `AppWithState`/`AsyncAppWithState::openapi(OpenApiConfig::new(title, version))`, which registers `GET /openapi.json` and `GET /docs`.

### Model layer

`src/model/` — JPA/Hibernate-style ORM. Enabled by feature flags `model-sqlite`, `model-postgres`, or `model-mysql` (one per compilation unit). No third-party ORM dependencies.

- `mod.rs` — `Value` enum (backend-independent SQL value: `Null`, `Bool`, `Int`, `Float`, `Text`, `Bytes`); `ModelRow` (named column bag with typed `get::<T>(col)`); `Model` trait (generated by `#[derive(Model)]`); `FromColumn` / `ToColumn` impls for `i16`, `i32`, `i64`, `u32`, `u64`, `f32`, `f64`, `bool`, `String`, `Option<T>`, `&str`.
- `connection.rs` — `DbConfig` (host, port, user, password, database, pool_size; `from_env()` reads `RWS_DB_*` vars); `DbConnection` with cfg-gated backend field (`rusqlite::Connection` / `postgres::Client` / `mysql::Conn`). Methods: `execute`, `query_rows`, `begin`, `commit`, `rollback`, `transaction(closure)`, `query::<T>`, `query_raw`, `migrate`, `migration_status`.
- `pool.rs` — `DbPool` (pre-creates `pool_size` connections; thread-safe `Mutex<Vec<DbConnection>>`); `PooledConnection<'_>` (checked out connection; `Deref`/`DerefMut` to `DbConnection`; returned to pool on `Drop`).
- `repository.rs` — `Repository<T, ID>` trait; `ModelRepository<'a, T, ID>` impl for `Repository<T, i64>`. `save` chooses INSERT (pk==0 or auto-increment) vs UPDATE. SQLite uses `last_insert_rowid()`; PostgreSQL uses `RETURNING id`; MySQL uses `last_insert_id()`.
- `query.rs` — `QueryBuilder<'a, T>` with free-function SQL builders to avoid borrow conflicts. `where_eq` injects `__placeholder__` tokens replaced with `?` (SQLite/MySQL) or `$N` (PostgreSQL). `delete`, `update`, `fetch_all`, `fetch_one`, `count`.
- `migration.rs` — reads `*.sql` from a directory in lexicographic order; creates `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if absent; wraps each unapplied file in a `BEGIN`/`COMMIT` transaction.
- `relation.rs` — `HasMany<T>`, `HasOne<O>`, `BelongsTo<O>` — explicit-load helpers with a `.load(&mut conn)` method. No lazy loading, no hidden N+1 queries.
- `tests.rs` — `#[cfg(all(test, feature = "model-sqlite"))]` integration tests using SQLite `:memory:`. 14 tests covering all 8 phases.

`rws-macros/src/lib.rs` — `#[proc_macro_derive(Model, attributes(table, column, primary_key))]` generates `impl Model for Struct` plus `Struct::repository(&mut conn)` and `Struct::query(&mut conn)` helpers. Attributes: `#[table(name = "…")]` (struct-level); `#[primary_key]` / `#[primary_key(auto_increment)]`; `#[column(name = "…")]`; `#[ignore]` (uses `Default::default()` in `from_row`, excluded from `to_values`).

### HTTP client

`src/http_client/mod.rs` — synchronous outbound HTTP/1.1 client. Always compiled in; no feature flag needed for plain HTTP. HTTPS requires `any(feature = "http-client", feature = "http2")`.

- `Client` — builder-pattern client; `Client::new()` sets `timeout_ms: 30_000` and `max_redirects: 10`. Convenience methods `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()`, `.head()`, `.request(method, url)` each return a `RequestBuilder`.
- `RequestBuilder` — `.header(k,v)`, `.body(bytes)`, `.body_text(s)` (sets `Content-Type: text/plain`), `.body_json(s)` (sets `Content-Type: application/json`), `.timeout_ms(ms)`, `.send() -> Result<Response, HttpClientError>`. Follows redirects automatically, downgrading to GET on 301/302/303, preserving method on 307/308.
- `Response` — `.status()`, `.is_success()` (200–299), `.is_redirect()` (301/302/303/307/308), `.header(name)` (case-insensitive lookup), `.bytes()`, `.text()`, `.json::<T>()` (requires `serde` feature).
- `HttpClientError(String)` — implements `std::error::Error` and `Display`.
- `ParsedUrl` (private) — splits `scheme://host[:port]/path[?query]` without external URL crate.
- `tls_connect()` (private, `#[cfg(any(feature = "http-client", feature = "http2"))]`) — wraps a `TcpStream` in `rustls::StreamOwned<ClientConnection, TcpStream>` using `webpki_roots` CA store.
- `AsyncClient` / `AsyncRequestBuilder` (gated on `#[cfg(feature = "http2")]`) — same API with `async fn send()`, backed by `tokio::net::TcpStream` and `tokio_rustls::TlsConnector`. Timeouts via `tokio::time::timeout`.
- Body reading strategy: chunked (`Transfer-Encoding: chunked`) → decode via `decode_chunked()`; content-length → read exactly N bytes; otherwise read until EOF.

