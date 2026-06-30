[Read Me](README.md) > Framework Roadmap

# Framework Roadmap

What is needed to evolve `rust-web-server` from an HTTP toolkit into a framework suitable for serious application development. Items are ordered by impact — the first three unlock everything else.

---

## Blockers — cannot build a real application without these

### ✅ 1. Shared application state — _Done (v17.9.0)_

`AppWithState<S>` in `src/state/mod.rs` wraps any `S: Send + Sync` behind an `Arc` and exposes route registration with state access:

```rust
let app = AppWithState::new(AppState { db: pool, config })
    .get("/users/:id", |_req, params, _conn, state| {
        let user = state.db.find(params.get("id").unwrap()).unwrap();
        // ... build response
    });
```

Handlers receive `(&Request, &PathParams, &ConnectionInfo, &S)`. Unmatched routes fall through to the built-in [`App`] controller chain. The `Arc<S>` is cloned once per route registration, not per request.

---

### 2. Dynamic routing with path parameters

Routes are hardcoded if/else chains in `App::execute`. There is no way to define `/users/:id` and receive `id` as a value — every handler must manually parse the URI string. A REST API with 20 endpoints means 20 manual string operations, and no route is declarative or inspectable.

**Target API:**
```rust
app.get("/users/:id", UserController::show);
app.post("/users",    UserController::create);
app.delete("/users/:id", UserController::destroy);
```

The router extracts named segments (`:id`) and wildcard segments (`*path`) and makes them available in the handler as typed values.

> ✅ **Done (v17.9.0):** `AppWithState<S>` (see Item 1) integrates `Router`-style `:param` / `*wildcard` matching with shared state. Standalone `Router` (v17.6.0) remains available for stateless dispatch inside custom `Application` implementations.

---

### ✅ 3. Middleware pipeline — _Done (v17.9.0)_

`Middleware` trait and `WithMiddleware<A>` in `src/middleware/mod.rs`. Wraps any `Application`:

```rust
let app = WithMiddleware::new(App::new())
    .wrap(AuthMiddleware::new(secret))
    .wrap(RateLimitMiddleware::new(100))
    .wrap(RequestLogger);
```

`Middleware::handle` receives `next: &dyn Application` — call `next.execute` to continue the chain or return early to short-circuit. Layers run in registration order on the request path and in reverse on the response path. Composes cleanly with `AppWithState`:

```rust
let app = WithMiddleware::new(AppWithState::new(state).get(...))
    .wrap(LoggingMiddleware);
```

---

### ✅ 4. HTTP/1.1 keep-alive (persistent connections) — _Done (v17.4.0)_

Every request requires a new TCP handshake. A browser loading a page with 10 assets makes 10 TCP connections. `Server::process` reads one request then closes. The fix is to loop over requests on the same stream until `Connection: close` is received or the read times out.

---

## Major gaps — severely limits real-world use

### ✅ 5. Async handlers — _Done (v17.11.0)_

`AsyncAppWithState<S>` in `src/async_state/mod.rs` (requires `http2` feature) gives handlers an `async fn` signature so they can `await` database queries, HTTP clients, or any other async I/O:

```rust
let app = AsyncAppWithState::new(db_pool)
    .get("/users/:id", |_req, params, _conn, state| async move {
        let id = params.get("id").unwrap();
        let user = state.find_user(id).await?;
        // ... build response
    });
```

Handler signature: `Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut` where `Fut: Future<Output = Response> + Send + 'static`. Handlers receive owned values so the future is `'static` and can be moved freely. Full `:param` / `*wildcard` path matching is included. Unmatched routes fall through to the built-in `App` controller chain.

Entry point: `App::with_async_state(state)` (requires the `http2` Cargo feature).

---

### ✅ 6. Typed request extractors — _Done (v17.7.0)_

`FromRequest` trait in `src/extract/mod.rs`. Built-in extractors:
- `Body` — raw bytes (never fails)
- `BodyText` — UTF-8 string (returns 400 on invalid UTF-8)
- `Query` — parsed query parameters as `HashMap<String, String>`
- `RequestHeaders` — all request headers with case-insensitive `get`

Implement `FromRequest` on your own type for custom extraction logic.

---

### ✅ 7. Duplicate dispatch logic — _Done (v17.6.0)_

`App::execute` and `App::handle_request` were nearly identical if/else chains over the same controllers. Adding one route required editing both. Fixed: `App::handle_request` now delegates to `App::execute` with a synthetic `ConnectionInfo`, eliminating the duplicate dispatch code.

---

### ✅ 8. Streaming responses / chunked transfer encoding — _Done (v17.4.0)_

Every response body is fully assembled in memory before the first byte is sent. A 500 MB file allocates 500 MB of RAM and holds it for the entire write. Both `Transfer-Encoding: chunked` (HTTP/1.1) and the native stream framing in HTTP/2 and HTTP/3 need to be wired to an iterator or async stream that the controller produces incrementally.

---

### ✅ 9. Typed error handling — _Done (v17.6.0)_

`Application::execute` returns `Result<Response, String>`. Production code needs typed errors that carry their own HTTP status code, so a handler can return `Err(AppError::NotFound)` and the framework maps it to a 404 without the handler building the response manually.

`IntoResponse` trait and `AppError` enum are in `src/error/mod.rs`. `AppError` covers 400, 401, 403, 404, 409, 422, and 500. Implement `IntoResponse` on your own error type for custom mappings.

---

## Secondary gaps — painful in practice

### ✅ 10. Cookies — _Done (v17.4.0)_

`CookieJar` parses the `Cookie` request header. `SetCookie` builds `Set-Cookie` response values with all RFC 6265 attributes.

---

### ✅ 11. Response compression — _Done (v17.4.0)_

Automatic gzip compression for text responses when the client sends `Accept-Encoding: gzip`.

---

### ✅ 12. `ConnectionInfo` peer address type — _Done (v17.8.0)_

`ConnectionInfo::peer_addr() -> Option<SocketAddr>` and `Address::to_socket_addr() -> Option<SocketAddr>` helpers added as non-breaking additions. The raw `ip: String` / `port: i32` fields are preserved for backward compatibility; the helpers parse them on demand.

---

### ✅ 13. Graceful shutdown — _Done (v17.7.0)_

`Server::run` (HTTP/1.1 thread pool path, `http1` feature) now installs a Ctrl+C/SIGTERM handler via the `ctrlc` crate. On signal: the accept loop exits, `SERVER_READY` is cleared (causing `/readyz` to return 503), and `ThreadPool::join()` drains all in-flight connections before returning. The async paths (`run_tls`, `run_quic`) have handled graceful shutdown since v17.5.0.

---

### ✅ 14. No test client — _Done (v17.6.0)_

`TestClient<A>` in `src/test_client/mod.rs` dispatches requests in-process through any `Application` without opening a TCP socket.

```rust
let client = TestClient::new(App::new());
let res = client.get("/healthz").send();
assert_eq!(200, res.status());
```

---

### ✅ 15. WebSocket support — _Done (v17.8.0)_

`src/websocket/mod.rs` provides RFC 6455-compliant WebSocket protocol primitives:
- `WebSocket::is_upgrade_request(&request)` — detects Upgrade/Connection/Key headers
- `WebSocket::handshake_response(&request)` — builds the `101 Switching Protocols` response (SHA-1 accept key, base64 encoded)
- `WebSocket::read_frame(stream)` — reads one frame, handles client-to-server masking
- `WebSocket::write_frame(stream, frame)` — sends a server-to-client unmasked frame
- `Frame` enum: `Text`, `Binary`, `Ping`, `Pong`, `Close`, `Continuation`
- Convenience methods: `send_text`, `send_close`, `send_pong`

Real-time features (chat, live updates, collaborative editing) are now possible. Because WebSocket requires raw stream access after the 101 response, the handler must drive its own accept loop rather than returning from a `Controller::process` call.

---

### ✅ 16. HTTP → HTTPS redirect — _Done (v17.4.0)_

`RWS_CONFIG_HTTP_REDIRECT_PORT` binds a plain-HTTP listener that issues `301 Moved Permanently` to the HTTPS equivalent URL.

---

## Next — high-impact additions

### ✅ 17. Server-Sent Events (SSE) — _Done (v17.12.0)_

`Sse` builder and `SseEvent` in `src/sse/mod.rs` produce a correctly formatted `text/event-stream` response body from a sequence of events. Headers set automatically: `Content-Type: text/event-stream`, `Cache-Control: no-cache`, `X-Accel-Buffering: no`.

```rust
use rust_web_server::sse::{Sse, SseEvent};

let response = Sse::new()
    .event("connected", "ready")
    .push(SseEvent::data(r#"{"count":1}"#).id("1").event_type("update"))
    .push(SseEvent::data(r#"{"count":2}"#).id("2").event_type("update"))
    .retry(5000)
    .comment("keep-alive")
    .into_response();
```

`SseEvent` supports `id`, `event_type`, `retry`, and multi-line `data` (produces one `data:` line per source line, which clients join with `\n`). The response body is fully buffered before sending — suitable for pre-known event sequences. For live streaming where events arrive over time, write the SSE headers and raw event lines directly to the TCP stream in a custom accept loop (same pattern as WebSocket).

---

### ✅ 18. Session management — _Done (v17.13.0)_

`SessionStore`, `Session`, and cookie helpers in `src/session/mod.rs`. Place one `SessionStore` in your application state; it is cheap to clone (all clones share the same `Arc<Mutex<…>>` backing map).

```rust
use rust_web_server::app::App;
use rust_web_server::session::{self, SessionStore};
use rust_web_server::header::Header;

struct State { sessions: SessionStore }

let app = App::with_state(State { sessions: SessionStore::new(3600) })
    .post("/login", |req, _params, _conn, state| {
        let mut sess = state.sessions.create();
        sess.set("user_id", "42");
        state.sessions.save(&sess);
        // set cookie on response …
        let cookie = session::session_cookie(&sess.id, "sid", 3600);
        // response.headers.push(Header { name: "Set-Cookie".to_string(), value: cookie });
        // …
    })
    .get("/profile", |req, _params, _conn, state| {
        let sid = session::session_id_from_request(&req, "sid")?;
        let sess = state.sessions.load(&sid)?;
        let user_id = sess.get("user_id").unwrap_or("guest");
        // …
    });
```

API summary:
- `SessionStore::new(ttl_secs)` — create a store; sessions expire after `ttl_secs`
- `store.create()` → `Session` — generate ID, insert empty session
- `store.create_with_id(id)` → `Session` — caller-supplied ID (CSPRNG)
- `store.load(id)` → `Option<Session>` — returns `None` if unknown or expired
- `store.save(&session)` — persist mutations back to the store
- `store.destroy(id)` — delete a session
- `store.purge_expired()` — reclaim memory (call periodically)
- `session_id_from_request(&req, cookie_name)` → `Option<String>`
- `session_cookie(id, name, ttl_secs)` → `Set-Cookie` value (`HttpOnly`, `SameSite=Lax`)
- `destroy_cookie(name)` → `Set-Cookie` with `Max-Age=0`

---

### ✅ 19. Serde JSON integration — _Done (v17.14.0)_

`Json<T>` extractor and responder in `src/json/extractor.rs`, gated on the `serde` Cargo feature (adds `serde` + `serde_json` deps). Enable with `features = ["serde"]` in `Cargo.toml`.

```toml
# Cargo.toml
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
        let Json(payload) = match Json::<CreateUser>::from_request(&req) {
            Ok(j)  => j,
            Err(r) => return r,  // 400 on bad JSON
        };
        Json(UserResponse { id: 1, name: payload.name }).into_response()
    });
```

- `Json::<T>::from_request(&req)` → `Result<Json<T>, Response>` (400 on parse error)
- `Json(value).into_response()` → `200 OK` with `Content-Type: application/json`
- Implements `FromRequest` so it works with the typed extractor pattern
- `Deref<Target = T>` for transparent field access

---

### ✅ 20. Built-in auth middleware (JWT + Basic) — _Done (v17.15.0)_

`BasicAuthLayer<F>` and `JwtLayer` in `src/auth/mod.rs`, gated on the `auth` Cargo feature (adds `hmac` + `sha2` from RustCrypto).

```toml
rust-web-server = { version = "17", features = ["auth"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::auth::{BasicAuthLayer, JwtLayer, build_jwt, verify_jwt};
use rust_web_server::core::New;

// HTTP Basic Auth — 401 + WWW-Authenticate challenge on missing/wrong credentials
let app = App::new()
    .wrap(BasicAuthLayer::new(|user, pass| user == "admin" && pass == "s3cret"));

// JWT HS256 — 401 on missing, tampered, wrong-algorithm, or expired tokens
let app = App::new()
    .wrap(JwtLayer::new(b"my-signing-secret"));

// Issue tokens from a login handler:
let token = build_jwt(r#"{"sub":"42","exp":9999999999}"#, b"my-signing-secret");
```

- `BasicAuthLayer::new(fn)` — validates `Authorization: Basic <base64>`; RFC 7617-compliant (passwords with `:` work)
- `JwtLayer::new(secret)` — verifies `Authorization: Bearer <token>` (HS256, constant-time)
- `build_jwt(claims_json, secret)` — produces a signed HS256 token; useful for login endpoints and tests
- `verify_jwt(token, secret)` → `Option<Claims>` — access `claims.sub`, `claims.exp`, `claims.raw` in handlers
- `extract_bearer_token(&req)` — extracts the raw token string from the Authorization header

---

### 21. Automatic TLS (ACME / Let's Encrypt)

Obtaining and renewing TLS certificates is manual today — the operator must run `certbot`, write the paths into config, and handle renewal restarts. ACME would automate issuance and zero-downtime renewal directly inside the server process.

**Target API:**
```rust
cargo run -- --acme-domain=example.com --acme-email=admin@example.com
```

---

## Developer experience

### 22. Declarative routing macros ✅ Done — v17.17.0

**`routes!` macro** (main crate, zero extra deps) builds any `AppWithState`, `AsyncAppWithState`, or `Router` from a declarative table:

```rust
use rust_web_server::routes;

let app = routes! {
    App::with_state(db),
    GET  "/users"     => list_users,
    GET  "/users/:id" => get_user,
    POST "/users"     => create_user,
};
```

**Proc-macro attributes** (`rws-macros` subcrate, `features = ["macros"]`) annotate handlers with their route for documentation and tooling:

```rust
use rust_web_server::route;  // re-exported from rws-macros

#[route(GET, "/users/:id")]
fn get_user(req: &Request, params: &PathParams, conn: &ConnectionInfo, state: &Db) -> Response {
    let id = params.get("id").unwrap_or("0");
    // ...
}
```

Shorthand method attributes: `#[get]`, `#[post]`, `#[put]`, `#[patch]`, `#[delete]`.

---

### 23. `derive(FromRequest)` ✅ Done — v17.18.0

`#[derive(FromRequest)]` in `rws-macros` (re-exported as `rust_web_server::FromRequest`).
Generates a `FromRequest` impl that calls `from_request` on each named field in declaration
order; the first failure short-circuits. Requires `features = ["macros"]`.

```rust
#[derive(Debug, rust_web_server::FromRequest)]
struct Payload {
    body: BodyText,
    query: Query,
}
```

---

### 24. Request validation helpers ✅ Done — v17.19.0

`Validate` trait, `ValidationErrors`, `Validated<T>` extractor, and `#[derive(Validate)]`
proc-macro in `src/validate/mod.rs` (re-exported as `rust_web_server::Validate`, requires
`features = ["macros"]` for the derive).

```rust
#[derive(rust_web_server::Validate)]
struct CreateUser {
    #[validate(length(min = 1, max = 50))]
    name: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 0, max = 150))]
    age: u8,
}

// In a handler — extract and validate in one step:
let Validated(user) = match Validated::<CreateUser>::from_request(req) {
    Ok(v)    => v,
    Err(res) => return res,  // 400 (extraction) or 422 (validation) with JSON errors
};
```

Supported validators: `length(min, max)`, `range(min, max)`, `email`, `required`, `url`.
All failures are collected before returning so the caller sees every invalid field at once.

---

## Security

### 25. IP allowlist / denylist ✅ Done — v17.16.0

`IpFilter` middleware in `src/ip_filter/mod.rs`. Accepts exact IPv4 addresses and CIDR ranges.
IPv6 client addresses are unmatched — blocked in allow mode, passed in deny mode.

```rust
use rust_web_server::ip_filter::IpFilter;

let app = App::new()
    .wrap(IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16"]))
    .wrap(IpFilter::deny(["1.2.3.4"]));
```

---

## Infrastructure

### 26. OpenTelemetry distributed tracing

There is no trace context propagation. Requests cannot be correlated across services, and there is no way to measure handler latency with span-level granularity compatible with Jaeger, Tempo, or Honeycomb.

**Target API:**
```rust
let app = App::new()
    .wrap(OtelLayer::new(tracer).propagate_b3().propagate_w3c());
```

---

### 27. Per-route metrics

`/metrics` currently exports only server-wide counters. Production services need per-route request counts and latency histograms to identify slow endpoints and set SLO alerts.

**Target outcome:** `/metrics` includes `rws_route_requests_total{method,path,status}` and `rws_route_duration_seconds{method,path}` histograms.

---

### 28. Response caching

Every request hits the handler regardless of whether the response could be served from an in-memory or shared cache. A cache middleware would short-circuit the handler for `GET` responses within their TTL.

**Target API:**
```rust
let app = App::new()
    .wrap(CacheLayer::memory(1000).ttl(60).vary_by_header("Accept"));
```

---

### 29. Hot config reload

Configuration changes (thread count, rate-limit thresholds, TLS cert rotation) require a full server restart today. A `SIGHUP` handler that re-reads `rws.config.toml` and applies non-binding changes in-place would eliminate downtime for routine tuning.

---

### 30. Reverse proxy / load balancing

There is no way to proxy requests to upstream services. A reverse-proxy handler would let `rws` sit in front of multiple backends, enabling blue-green deploys, A/B routing, and sidecar patterns without an external Nginx or Envoy.

**Target API:**
```rust
let app = App::new()
    .wrap(ReverseProxy::new(["http://backend-1:8080", "http://backend-2:8080"])
        .strategy(LoadBalancing::RoundRobin)
        .health_check("/healthz"));
```

---

### 31. MCP (Model Context Protocol) server

AI coding agents and LLM tool-callers need a standardized interface to interact with application APIs. An `McpController` would expose tools, resources, and prompts over the MCP protocol, making any `rws` application instantly reachable from Claude, Cursor, and other MCP-aware clients.

**Target API:**
```rust
let app = App::new()
    .mcp_tool("list_users", list_users_handler)
    .mcp_resource("user://{id}", get_user_resource)
    .mcp_prompt("summarize", summarize_prompt);
```

---

## Summary

| # | Item | Status |
|---|------|--------|
| 1 | Shared application state | ✅ Done (v17.9.0) |
| 2 | Dynamic routing with path parameters | ✅ Done (v17.9.0) |
| 3 | Middleware pipeline | ✅ Done (v17.9.0) |
| 4 | HTTP/1.1 keep-alive | ✅ Done (v17.4.0) |
| 5 | Async handlers | ✅ Done (v17.11.0) |
| 6 | Typed request extractors | ✅ Done (v17.7.0) |
| 7 | Duplicate dispatch logic | ✅ Done (v17.6.0) |
| 8 | Streaming responses | ✅ Done (v17.4.0) |
| 9 | Typed error handling | ✅ Done (v17.6.0) |
| 10 | Cookies | ✅ Done (v17.4.0) |
| 11 | Response compression | ✅ Done (v17.4.0) |
| 12 | `ConnectionInfo` uses `String` not `SocketAddr` | ✅ Done (v17.8.0) |
| 13 | Graceful shutdown | ✅ Done (v17.7.0) |
| 14 | No test client | ✅ Done (v17.6.0) |
| 15 | WebSocket support | ✅ Done (v17.8.0) |
| 16 | HTTP → HTTPS redirect | ✅ Done (v17.4.0) |
| 17 | Server-Sent Events (SSE) | ✅ Done (v17.12.0) |
| 18 | Session management | ✅ Done (v17.13.0) |
| 19 | Serde JSON integration | ✅ Done (v17.14.0) |
| 20 | Built-in auth middleware (JWT + Basic) | ✅ Done (v17.15.0) |
| 21 | Automatic TLS (ACME / Let's Encrypt) | Pending |
| 22 | Declarative routing macros | ✅ Done (v17.17.0) |
| 23 | `derive(FromRequest)` | ✅ Done (v17.18.0) |
| 24 | Request validation helpers | ✅ Done (v17.19.0) |
| 25 | IP allowlist / denylist | ✅ Done (v17.16.0) |
| 26 | OpenTelemetry distributed tracing | Pending |
| 27 | Per-route metrics | Pending |
| 28 | Response caching | Pending |
| 29 | Hot config reload | Pending |
| 30 | Reverse proxy / load balancing | Pending |
| 31 | MCP server controller | Pending |
