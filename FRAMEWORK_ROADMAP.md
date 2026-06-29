[Read Me](README.md) > Framework Roadmap

# Framework Roadmap

What is needed to evolve `rust-web-server` from an HTTP toolkit into a framework suitable for serious application development. Items are ordered by impact — the first three unlock everything else.

---

## Blockers — cannot build a real application without these

### 1. Shared application state

`App` is a zero-sized `Copy` struct. `Controller` methods are all static `fn` with no `&self`. There is no way to inject a database pool, config struct, cache, or any shared resource into a handler.

**Target API:**
```rust
let app = App::new().with_state(AppState {
    db: Pool::connect(&database_url).await?,
    config: Config::from_env(),
});
```

Every controller would receive `&AppState` (or a typed extract from it) alongside `&Request`.

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

> ✅ **Partially done (v17.6.0):** `Router` in `src/router/mod.rs` provides standalone dynamic routing with `:param` and `*wildcard` extraction. Handlers receive `&PathParams`. Integration with the built-in `App::execute` is pending (item 1 — shared state — is the prerequisite for wiring it cleanly).

---

### ✅ 3. Middleware pipeline — _Deferred: requires architectural change_

There is no pipeline. Cross-cutting concerns — authentication, rate limiting, request tracing, header injection — require editing `App::execute` directly. There is no way to compose behavior without touching core dispatch code.

**Target API:**
```rust
app.wrap(AuthMiddleware::new(secret))
   .wrap(RateLimiter::new(100))
   .wrap(RequestLogger::new());
```

Each middleware wraps the next, forming a chain. A request flows inward through the chain before reaching the handler, and the response flows back outward.

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

### 12. `ConnectionInfo` peer address type

`ConnectionInfo.client.ip` is a `String` and `.client.port` is an `i32` instead of a `std::net::SocketAddr`. Users who need the actual address for logging or rate limiting must re-parse strings. The field should be a `SocketAddr`.

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

### 15. No WebSocket support

No `Upgrade: websocket` handling. Real-time features (chat, live updates, collaborative editing) are not possible.

---

### ✅ 16. HTTP → HTTPS redirect — _Done (v17.4.0)_

`RWS_CONFIG_HTTP_REDIRECT_PORT` binds a plain-HTTP listener that issues `301 Moved Permanently` to the HTTPS equivalent URL.

---

## Summary

| # | Gap | Status |
|---|-----|--------|
| 1 | Shared application state | Pending |
| 2 | Dynamic routing with path parameters | ✅ Partial (standalone `Router`) |
| 3 | Middleware pipeline | Pending |
| 4 | HTTP/1.1 keep-alive | ✅ Done (v17.4.0) |
| 5 | Async handlers | Pending |
| 6 | Typed request extractors | ✅ Done (v17.7.0) |
| 7 | Duplicate dispatch logic | ✅ Done (v17.6.0) |
| 8 | Streaming responses | ✅ Done (v17.4.0) |
| 9 | Typed error handling | ✅ Done (v17.6.0) |
| 10 | Cookies | ✅ Done (v17.4.0) |
| 11 | Response compression | ✅ Done (v17.4.0) |
| 12 | `ConnectionInfo` uses `String` not `SocketAddr` | Pending |
| 13 | Graceful shutdown | ✅ Done (v17.7.0) |
| 14 | No test client | ✅ Done (v17.6.0) |
| 15 | No WebSocket | Pending |
| 16 | HTTP → HTTPS redirect | ✅ Done (v17.4.0) |
