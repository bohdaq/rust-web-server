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

### 5. Async handlers

The HTTP/1.1 path blocks one OS thread per connection. With 200 threads (the default) and any I/O waiting — a database query, an external API call — the pool saturates at 200 concurrent users. The HTTP/2 and HTTP/3 paths use tokio, but handlers have no async interface and cannot `await` anything.

**What is needed:** Controllers that can return a `Future`, and a tokio-backed executor for the HTTP/1.1 path as well, eliminating the fixed thread-count ceiling.

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

## Summary

| # | Gap | Status |
|---|-----|--------|
| 1 | Shared application state | ✅ Done (v17.9.0) |
| 2 | Dynamic routing with path parameters | ✅ Done (v17.9.0) |
| 3 | Middleware pipeline | ✅ Done (v17.9.0) |
| 4 | HTTP/1.1 keep-alive | ✅ Done (v17.4.0) |
| 5 | Async handlers | Pending |
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
