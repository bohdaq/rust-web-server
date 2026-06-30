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

### 18. Session management

No session layer exists. Stateful applications must implement their own cookie-based session ID generation, storage lookup, and expiry — typically 100+ lines of boilerplate per project.

**Target API:**
```rust
let app = App::with_state(store)
    .wrap(SessionLayer::new(RedisStore::new(url)).cookie("sid").ttl(3600));

// In a handler:
let session = Session::from_request(&req)?;
session.set("user_id", user.id);
```

---

### 19. Serde JSON integration

The built-in `json` module requires manual property access and has no serialization support. Every JSON handler must construct strings by hand, which is error-prone and verbose.

**Target API:**
```rust
use rust_web_server::json::Json;

// Deserialize:
let body: MyRequest = Json::from_request(&req)?;

// Serialize:
Json(MyResponse { ok: true, count: 42 }).into_response()
```

---

### 20. Built-in auth middleware

JWT verification and HTTP Basic Auth are common enough to ship as first-party middleware rather than leaving each consumer to implement them correctly (timing-safe comparison, algorithm confusion attacks, etc.).

**Target API:**
```rust
// JWT
let app = App::new()
    .wrap(JwtLayer::new(secret).algorithm(Algorithm::HS256).claim("sub"));

// Basic Auth
let app = App::new()
    .wrap(BasicAuthLayer::new(|user, pass| user == "admin" && pass == secret));
```

---

### 21. Automatic TLS (ACME / Let's Encrypt)

Obtaining and renewing TLS certificates is manual today — the operator must run `certbot`, write the paths into config, and handle renewal restarts. ACME would automate issuance and zero-downtime renewal directly inside the server process.

**Target API:**
```rust
cargo run -- --acme-domain=example.com --acme-email=admin@example.com
```

---

## Developer experience

### 22. Declarative routing macros

The current `App::execute` registration table works but requires a separate struct per route and explicit wiring. A proc-macro attribute would eliminate the boilerplate for the common case.

**Target API:**
```rust
#[route(GET, "/users/:id")]
async fn get_user(req: Request, params: PathParams, conn: ConnectionInfo, state: Arc<Db>) -> Response {
    // ...
}
```

---

### 23. `derive(FromRequest)`

Implementing `FromRequest` for a custom extractor today requires a manual `impl FromRequest for MyType` block. A derive macro would generate it from struct field types.

**Target API:**
```rust
#[derive(FromRequest)]
struct AuthPayload {
    #[from_header("Authorization")]
    token: BearerToken,
    #[from_query("locale")]
    locale: Option<String>,
}
```

---

### 24. Request validation helpers

Field-level validation (required, min/max length, regex, numeric range) is manual today. A validation layer would run checks before the handler and return structured 422 error bodies automatically.

**Target API:**
```rust
#[derive(Validate, FromRequest)]
struct CreateUser {
    #[validate(length(min = 1, max = 50))]
    name: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 0, max = 150))]
    age: u8,
}
```

---

## Security

### 25. IP allowlist / denylist

No request filtering by client IP exists. Blocking known-bad ranges or restricting admin endpoints to an internal CIDR requires a custom `Middleware` implementation today.

**Target API:**
```rust
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
| 18 | Session management | Pending |
| 19 | Serde JSON integration | Pending |
| 20 | Built-in auth middleware (JWT + Basic) | Pending |
| 21 | Automatic TLS (ACME / Let's Encrypt) | Pending |
| 22 | Declarative routing macros | Pending |
| 23 | `derive(FromRequest)` | Pending |
| 24 | Request validation helpers | Pending |
| 25 | IP allowlist / denylist | Pending |
| 26 | OpenTelemetry distributed tracing | Pending |
| 27 | Per-route metrics | Pending |
| 28 | Response caching | Pending |
| 29 | Hot config reload | Pending |
| 30 | Reverse proxy / load balancing | Pending |
| 31 | MCP server controller | Pending |
