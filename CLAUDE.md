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

`Server::process()` and `process_h1_tls()` (TLS) both support `Expect: 100-continue` (RFC 9110 §10.1.1): after parsing headers and passing the `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` check, a request with `Expect: 100-continue` gets `Server::continue_response()` written immediately, before the body-continuation read loop runs — otherwise the read loop would block waiting for a body the client is itself waiting to be told to send. An `Expect` value other than `100-continue` gets `Server::expectation_failed_response()` (417) without reading the body. HTTP/2 (`h2_handler`) and HTTP/3 (`h3_handler`) read bodies as separate async `DATA` frames rather than one blocking read, so they don't have this deadlock risk and don't implement it.

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

`initialize` negotiates `params.protocolVersion` down to the lower of the client's request and the server's own version (`YYYY-MM-DD` strings compare correctly lexically) rather than always claiming its own, and mints a session id (`Mcp-Session-Id` response header) that records that call's `clientInfo` in `sessions: Arc<Mutex<HashMap<String, StoredClientInfo>>>`. `.tool_with_context(name, description, schema, |ctx: McpContext, args: &str| ...)` registers a tool whose handler additionally receives that session's `McpContext` (`client_name`, `client_version`, `session_id`, `auth_claims` — the last always `None` today); `execute()` reads the echoed `Mcp-Session-Id` header to look it up, while `handle_request(body)` (used directly in most tests) yields an empty `McpContext` unless the test calls `handle_request_with_context(body, ctx)` explicitly. Session records are never evicted.

`.tool_annotated(name, description, schema, annotations, handler)` (MCP 2025-03-26) attaches a `ToolAnnotations` value (`read_only_hint`, `destructive_hint`, `idempotent_hint`, `open_world_hint`, all `Option<bool>`) to a tool — hints, not enforced — serialized as an `annotations` object with camelCase keys (`readOnlyHint`, etc.) in `tools/list` only for tools that have them; its handler is the plain `Fn(&str) -> ...` shape like `.tool()`, not the context-aware shape, so there is no single builder combining annotations with `McpContext`.

`McpContent::image(data, mime_type)` and `McpContent::embedded(uri, text, mime_type)` cover the MCP spec's `image` and `resource` content variants alongside the original `text`/`json`; `to_content_json()` branches on `McpContent::kind` (`"text"` / `"image"` / `"resource"`) to pick the right JSON shape. `image` expects an already-base64-encoded string — no base64 crate is a dependency of this project. `resources/read`'s response format is untouched (hand-built, doesn't go through `to_content_json()`), so a resource handler still can't return image content that way.

`handle_request_with_context` detects a top-level JSON array (`body.trim_start().starts_with('[')`) and hands off to `handle_batch`, supporting JSON-RPC 2.0 batch requests — several calls dispatched from one `POST /mcp` body, joined into one `[...]` response array. `json_rpc::split_array_elements` does the array splitting (depth/string-tracking, like `bracket_extract`); the per-method dispatch table and JSON-RPC response rendering are factored into private `dispatch()`/`format_result()` helpers shared by both the single-request and batch code paths. Notifications in a batch contribute no response entry; an all-notification batch returns `202` with no body; an empty array (`[]`) returns one `Invalid Request` error rather than `[]`; a successful `initialize` inside a batch still mints a session and sets `Mcp-Session-Id` (only the first `initialize` in a batch counts, since one response carries one session id).

`.page_size(n)` (clamped to a minimum of `1`) enables cursor-based pagination for `tools/list`/`resources/list`/`prompts/list`; unset (the default), every item is returned in one response with no `nextCursor`. A shared `fn paginate(&self, items: &[String], body: &str) -> Result<(&[String], Option<String>), (i32, String)>` reads `params.cursor`, decodes it via private `encode_cursor`/`decode_cursor` (base64 of the decimal offset string, using self-contained `base64_encode`/`base64_decode` free functions — no base64 crate dependency, mirroring `websocket::base64_encode`'s same from-scratch approach), slices the items, and returns the page plus an optional `nextCursor`. An invalid/tampered cursor returns `INVALID_PARAMS` (`-32602`); an offset past the end returns an empty page with no `nextCursor`.

`GET /mcp` opens an SSE stream for server → client push, built on the existing `Response::stream_pipe: Option<Box<dyn Read + Send>>` mechanism (already used for reverse-proxy passthrough streaming) rather than new response-writing machinery — `Server::pipe_stream` drives it unmodified. `sse_clients: Arc<Mutex<Vec<SyncSender<Vec<u8>>>>>` holds one bounded (`mpsc::sync_channel(32)`) sender per connected client; `start_sse_stream()` registers a new one and returns a `Response` whose `stream_pipe` is `SseChannelReader` (a `Read` adapter blocking on the receiver, emitting a `: keep-alive` comment every `SSE_KEEPALIVE_INTERVAL` (15s) when idle — this doubles as disconnect detection, since the next real write after a dead peer fails). `.notify(method, params_json)` (public) renders one JSON-RPC notification as an SSE `data:` frame and pushes it to every client via `try_send` (never blocks); a client whose buffer is full is dropped from `sse_clients` exactly like a disconnected one. Only wired up for the plain HTTP/1.1 path — `h2_handler`/`h3_handler` don't drive `stream_pipe` for any response yet. `TestClient` doesn't drive `stream_pipe` either (it inspects the `Response` directly), so SSE tests call `start_sse_stream()`/`notify()` directly and read from the returned reader in-process.

`LogLevel` (8 RFC 5424 severities, `Debug` through `Emergency`, `PartialOrd`/`Ord` derived from declaration order) plus `min_log_level: Arc<Mutex<LogLevel>>` and `.logging_enabled()` (adds `"logging":{}` to `initialize`'s capabilities — advertising only, doesn't gate anything) implement `logging/setLevel` (`dispatch`'s `"logging/setLevel" => self.do_set_log_level(body)` stores the requested level) and `.log(level, logger, data_json)`, which builds a `notifications/message` params object and calls `self.notify(...)` — but only if `level >= *min_log_level.lock().unwrap()` — so it inherits `.notify()`'s backpressure/disconnect handling for free. Default `min_log_level` is `LogLevel::Debug` (nothing filtered) until a client calls `logging/setLevel`.

`tools`/`resources`/`prompts` are stored as `Arc<RwLock<Vec<T>>>` (not a plain `Vec`) so every clone of `McpServer` shares the same live list — this backs dynamic registration: `.register_tool(name, description, schema, handler)` / `.remove_tool(name) -> bool` and the `register_resource`/`remove_resource`, `register_prompt`/`remove_prompt` equivalents take `&self`, unlike the consuming `.tool()`/`.resource()`/`.prompt()` builders, so they can be called from any thread at any time (no separate `McpHandle` type — a cloned `McpServer` already shares the storage). `do_tools_call`/`do_resources_read`/`do_prompts_get` clone the matched handler's `Arc` out from under a short read-lock guard before invoking it, so a slow handler never blocks a concurrent registration. Each successful registration/removal pushes `notifications/{tools,resources,prompts}/list_changed` (no `params`) via `.notify()`; a no-op removal (name not found) pushes nothing. `initialize` advertises `listChanged:true` for all three unconditionally; `resources.subscribe` stays `false` (that's TODO-14, not implemented).

`McpContext` gained `pub progress_token: Option<String>` (the raw JSON form of `params._meta.progressToken` from a `tools/call` request — stored raw, not decoded, since the spec allows `string | number` and this way both round-trip correctly) and a private `sse_clients: Option<Arc<Mutex<Vec<SyncSender<Vec<u8>>>>>>`, set unconditionally in `context_for()` for every request. `do_tools_call` extracts `_meta.progressToken` via two nested `json_rpc::extract_raw` calls (no dotted-path support in the hand-rolled JSON helpers) and overrides `ctx.progress_token` before calling the handler. `McpContext::report_progress(progress, total, message)` pushes `notifications/progress` — reads `self.progress_token` internally (not passed as a parameter, unlike the TODO's own sketch) and silently no-ops if it's `None` or `sse_clients` is `None` (a hand-built context, e.g. via `handle_request_with_context`). `McpServer::notify` and `report_progress` share `render_notification()`/`broadcast_sse_to()` free functions (extracted from the old `notify`/`broadcast_sse` methods) so the JSON-RPC rendering and try_send-and-prune broadcast logic exists in exactly one place.

`.completion(ref_type, ref_name, handler)` (a consuming builder like `.tool()`/`.resource()`/`.prompt()`) registers an argument-autocompletion provider in `completions: Arc<RwLock<Vec<CompletionDef>>>`; `dispatch`'s `"completion/complete" => self.do_completion(body)` looks up the entry matching `ref.name` and `ref.type` with its `"ref/"` prefix stripped (so `ref_type: "tool"` matches wire value `"ref/tool"` — an extension beyond the real spec's `ref/prompt`/`ref/resource`, since this server treats tools as completable too), calls the handler with `argument.name`/`argument.value`, and renders `{"completion":{"values":[...],"hasMore":...,"total":...}}`. No matching registration returns empty `values`, not an error. Results beyond `MAX_COMPLETION_VALUES` (100) are truncated with `hasMore:true` and the untruncated `total`. `initialize` advertises `"completions":{}` automatically once `!self.completions.read().unwrap().is_empty()` — no separate opt-in flag. No dynamic (`&self`) equivalent — completions are builder-only.

Request cancellation (`notifications/cancelled`) is **cooperative**, via `cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>` keyed by a `tools/call`'s raw `id` token — not the async-only `tokio_util::CancellationToken` design originally sketched, since async tool handlers don't exist yet (TODO-17). Private `dispatch_with_cancellation` (wrapping `dispatch` in both `handle_request_with_context` and `handle_batch`) registers a flag before a `tools/call`, attaches it to `McpContext` via a new private `cancellation: Option<Arc<AtomicBool>>` field, and always removes the entry afterward regardless of whether it was checked — no leak risk, unlike `sessions`/`sse_clients`. `notifications/cancelled` is special-cased ahead of the generic notification-swallowing branch in both dispatch paths; it reads `params.requestId` (raw token, `string | number`) and flips the matching flag, silently no-op on an unknown/finished id. `McpContext::is_cancelled(&self) -> bool` is the handler-facing getter, always safe to call, `false` by default.

`resources/subscribe`/`resources/unsubscribe` (`.notify_resource_updated(uri)` is the resource-owner-facing push method) are the first genuinely *targeted* SSE notification in this module — every other one (`.notify()`, `.log()`, `list_changed`) broadcasts to all connected clients. `sse_clients` changed from a flat `Vec<SseSender>` to `Vec<SseClient>` (`struct SseClient { session_id: Option<String>, sender: SseSender }`); `start_sse_stream` now reads `Mcp-Session-Id` off the `GET /mcp` request to tag each connection. `subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>>` maps a resource URI to subscribed session ids; `send_sse_to_sessions` (a session-filtering sibling of `broadcast_sse_to`) does the targeted send. `do_resource_subscribe`/`do_resource_unsubscribe` require `ctx.session_id` (`INVALID_PARAMS` without one — a subscription with no way to correlate to an SSE connection could never fire); unsubscribing the last subscriber for a URI prunes that URI's `subscriptions` entry entirely. `initialize`'s `resources.subscribe` is now unconditionally `true`. No proactive cleanup when an SSE connection disconnects without unsubscribing first — same tradeoff as `sessions`.

`sampling/createMessage` (server-side sampling) reverses the normal direction: `ctx.sample(request: SamplingRequest, timeout: Duration) -> Result<SamplingResponse, String>` (`.tool_with_context()` only) sends a JSON-RPC *request* to the client via `send_sse_to_sessions` and **blocks the calling thread** (not `async fn` — tool handlers themselves are still synchronous even where `.async_tool()` exists, see below) on an `mpsc::Receiver` until the client's `POST /mcp` reply arrives or `timeout` elapses. `SamplingRequest.messages: Vec<PromptMessage>` reuses `PromptMessage` rather than a duplicate `SamplingMessage` type (identical wire shape). The client's reply is a JSON-RPC *response* — no `method` — so `handle_request_with_context`/`handle_batch` now check a method-less body with a recognized `id` against `pending_replies: Arc<Mutex<HashMap<String, mpsc::Sender<Result<String,String>>>>>` (`try_deliver_sampling_response`) before falling back to the pre-existing "Missing method" error. `StoredClientInfo.supports_sampling` (from `params.capabilities.sampling` at `initialize` — a *client*-declared capability, not server-advertised) gates `sample()` failing fast without sending anything; it also fails fast with no session id or no live server, and times out if the client never responds (including simply having no `GET /mcp` connection open).

`ctx.sample`'s request/response mechanics are factored into a private `McpContext::send_and_wait(method, params_json, timeout)` shared with `ctx.list_roots(timeout) -> Result<Vec<McpRoot>, String>` (`roots/list` + `notifications/roots/list_changed`) — both are thin wrappers differing only in method/params sent and `result` parsing (`parse_sampling_response` vs `parse_roots_response`). Unlike sampling, `list_roots` **caches per session** in a new `StoredClientInfo.roots: Option<Vec<McpRoot>>` (`None` = never fetched or invalidated) — only the first call in a session round-trips, later calls return the cache. `notifications/roots/list_changed` (special-cased in both dispatch paths ahead of the generic notification branch, same position as `notifications/cancelled`) clears the cache via `invalidate_roots_cache(&ctx.session_id)`, keyed purely by session id since this notification carries no params. `StoredClientInfo.supports_roots` mirrors `supports_sampling`'s gate.

`.async_tool(name, description, schema, handler)` (`http2` feature) registers a tool whose handler is `Fn(&str) -> impl Future<Output = Result<McpContent, String>>`, stored separately in `async_tools: Arc<RwLock<Vec<AsyncToolDef>>>` (not unified into `ToolDef` via a handler enum — far less invasive than touching every existing sync-tool code path). `tools/call` bridges into the future via `crate::async_bridge::block_on_isolated` — the same mechanism `H2ReverseProxy`/`AsyncAppWithState::execute` use — not `tokio::task::block_in_place`, which panics outside a `multi_thread` runtime. `do_tools_list` merges `tools`/`async_tools` via a shared `render_tool_list_entry` helper; `do_tools_call` checks `tools` first, then `async_tools` under `#[cfg(feature = "http2")]`. `.register_async_tool(...)` is the dynamic equivalent (TODO-9 style); `.remove_tool(name)` checks both collections. No async equivalent of `.tool_with_context()`/`.tool_annotated()` yet.

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
- `RequestBuilder` — `.header(k,v)`, `.body(bytes)`, `.body_text(s)` (sets `Content-Type: text/plain`), `.body_json(s)` (sets `Content-Type: application/json`), `.form(&[(&str,&str)])` (percent-encodes and joins pairs, sets `Content-Type: application/x-www-form-urlencoded` — the body shape OAuth 2.0 token endpoints require; used by `sso::client::OidcClient::exchange_code`), `.timeout_ms(ms)`, `.send() -> Result<Response, HttpClientError>`. Follows redirects automatically, downgrading to GET on 301/302/303, preserving method on 307/308.
- `Response` — `.status()`, `.is_success()` (200–299), `.is_redirect()` (301/302/303/307/308), `.header(name)` (case-insensitive lookup), `.bytes()`, `.text()`, `.json::<T>()` (requires `serde` feature).
- `HttpClientError(String)` — implements `std::error::Error` and `Display`.
- `ParsedUrl` (private) — splits `scheme://host[:port]/path[?query]` without external URL crate.
- `tls_connect()` (private, `#[cfg(any(feature = "http-client", feature = "http2"))]`) — wraps a `TcpStream` in `rustls::StreamOwned<ClientConnection, TcpStream>` using `webpki_roots` CA store.
- `AsyncClient` / `AsyncRequestBuilder` (gated on `#[cfg(feature = "http2")]`) — same API including `.form()`, with `async fn send()`, backed by `tokio::net::TcpStream` and `tokio_rustls::TlsConnector`. Timeouts via `tokio::time::timeout`.
- Body reading strategy: chunked (`Transfer-Encoding: chunked`) → decode via `decode_chunked()`; content-length → read exactly N bytes; otherwise read until EOF.

### SSO (OAuth 2.0 / OIDC client, `sso` feature; Authorization Server, `sso-server` feature)

`src/sso/` — OAuth 2.0 / OIDC client support (`rws` as the relying party, e.g. "Login with Google"). Implies `http-client` for TLS calls to the IdP. All JSON parsing and JWT verification is hand-rolled, matching the crate's no-third-party-HTTP-parsing philosophy elsewhere — the only new deps are `rsa`/`p256` (asymmetric signature verification), `sha2` (hashing), and `rand_core` (PKCE verifier / RSA keygen randomness).

- `jwks::JwksCache` — thread-safe cache of public keys from a JWKS endpoint (`Mutex<Vec<JwkEntry>>`, private). `.fetch()` downloads and replaces the cache via `crate::http_client::Client`; `.verify_jwt(token, &VerifyOptions) -> Result<OidcClaims, SsoError>` lazy-loads keys on first call (empty cache), verifies the RS256/ES256 signature via `try_verify` (matches `kid` from the JWT header against cached keys, falling back to trying every cached key if the JWT has no `kid`), then validates `exp`/`iat`/`iss`/`aud`. **Key rotation is reactive, not scheduled** — there's no background refresh timer (the original design sketch's `.refresh_interval_secs()` doesn't exist); instead, a failed `try_verify` triggers exactly one `fetch()` retry before giving up, so a token signed with a key rotated in after the last successful verification still verifies correctly on the very next call. ES256 signatures are the raw 64-byte `r||s` form (not DER) per the JWS spec, reconstructed via `p256::EncodedPoint::from_affine_coordinates`; RS256 uses `rsa::pkcs1v15::VerifyingKey<Sha256>`.
- `jwks::OidcClaims` — the standard OIDC claims (`sub`, `iss`, `aud: Vec<String>`, `exp`, `iat`, `nonce`, `email`, `email_verified`, `name`, `given_name`, `family_name`, `picture`, `locale`). No `groups`/`extra` fields — this module models only spec-standard claims, not IdP-specific extensions.
- `jwks::VerifyOptions` — `audience`, `issuer`, `leeway_secs` (clock-skew tolerance applied to both `exp` and `iat` checks).
- `discovery::OidcProvider` — endpoint URLs (`authorization_endpoint`, `token_endpoint`, `jwks_uri`, `userinfo_endpoint`, `end_session_endpoint`). `::discover(issuer)` fetches `{issuer}/.well-known/openid-configuration`; hardcoded presets `::google()`, `::microsoft(tenant_id)`, `::github()` (OAuth-only, empty `jwks_uri` — no id_token), `::okta(domain)`, `::auth0(domain)`, `::keycloak(base_url, realm)`.
- `config::OidcConfig` — bundles `provider`, `client_id`, `client_secret`, `redirect_uri`, `scopes`, `post_login_redirect`. Presets mirror `OidcProvider`'s; `::from_env()` reads `RWS_OIDC_*` vars (see `docs/src/content/docs/features/sso.md`).
- `client::OidcClient` — `.authorization_url(pkce, state, nonce)` builds the redirect URL (adds PKCE `code_challenge` only when `provider.jwks_uri` is non-empty, since GitHub's token endpoint rejects PKCE params); `.exchange_code(code, pkce_verifier)` POSTs to `token_endpoint` via `Client::form()` and returns `TokenResponse`; `.fetch_user_info(access_token)` covers GitHub-style providers with no `id_token`.
- `pkce::PkceVerifier` / `PkceChallenge` — `code_verifier` (random 43-char base64url) and `code_challenge = BASE64URL(SHA256(verifier))` (S256 only, per RFC 7636).
- `oidc_auth::OidcAuth` — `Middleware` impl: no session → redirect to `/auth/login`; intercepts `/auth/login` (redirect to IdP), `/auth/callback` (validate `state`, exchange code, verify `id_token` via `JwksCache`, verify `nonce`, store claims in session), `/auth/logout`. `.exclude(path)` bypasses auth for a path prefix. Claims are injected into the `X-Rws-Oidc-Claims` request header (JSON) for downstream handlers; `OidcAuth::claims(request)`/`::sub()`/`::email()` are the read-side helpers.

`server::AuthServer` / `client_store::ClientStore` (`sso-server` feature, implies `sso` + `auth`) — the reverse role: `rws` as its own OAuth 2.0 Authorization Server rather than a client of one. A `Middleware` intercepting `POST /oauth/token` (`client_credentials`, `authorization_code`+PKCE, `refresh_token` grants), `GET /oauth/authorize`, `GET /.well-known/openid-configuration`, `GET /.well-known/jwks.json`. Two deliberate deviations from the original design sketch, both documented at length in `src/sso/server.rs`'s module docs: (1) tokens are signed HS256 via the existing `auth::build_jwt`/`auth::verify_jwt` (not an RSA/EC key loaded from PEM — this crate has no private-key PEM/DER parser), so `/.well-known/jwks.json` always returns `{"keys":[]}` (no public key exists for a symmetric algorithm; resource servers must share `signing_secret` directly); (2) `/oauth/authorize` has no built-in login page — it reads a configurable session key (default `"user_id"`, override via `.subject_session_key()`) from an existing app session via `crate::session::SessionStore` to identify the resource owner, minting a code immediately if present or redirecting to a configurable `login_url` (default `/login`) with `?return_to=...` if absent; building that login page is the embedding application's responsibility. Authorization codes and refresh tokens are opaque random strings held in-memory (`Mutex<HashMap<String, _>>`), not JWTs themselves — codes are single-use (removed from the map on exchange) and short-lived (60s). `client_store::ClientStore` (`.new().add(OAuthClient{...}).get(client_id)`) has no `::from_env()`/`::from_db()` — a list of clients with per-client secrets has no natural flat-env-var encoding, unlike `OidcConfig`'s one-client-per-process shape.

Phase 7 (SAML 2.0 SP) is not implemented — see `spec/SSO.md`.

