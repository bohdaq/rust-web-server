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

---

### 3. Middleware pipeline

There is no pipeline. Cross-cutting concerns — authentication, rate limiting, request tracing, header injection — require editing `App::execute` directly. There is no way to compose behavior without touching core dispatch code.

**Target API:**
```rust
app.wrap(AuthMiddleware::new(secret))
   .wrap(RateLimiter::new(100))
   .wrap(RequestLogger::new());
```

Each middleware wraps the next, forming a chain. A request flows inward through the chain before reaching the handler, and the response flows back outward.

---

### 4. HTTP/1.1 keep-alive (persistent connections)

Every request requires a new TCP handshake. A browser loading a page with 10 assets makes 10 TCP connections. `Server::process` reads one request then closes. The fix is to loop over requests on the same stream until `Connection: close` is received or the read times out.

This is also listed in [ROADMAP.md](ROADMAP.md) as Priority 1.

---

## Major gaps — severely limits real-world use

### 5. Async handlers

The HTTP/1.1 path blocks one OS thread per connection. With 200 threads (the default) and any I/O waiting — a database query, an external API call — the pool saturates at 200 concurrent users. The HTTP/2 and HTTP/3 paths use tokio, but handlers have no async interface and cannot `await` anything.

**What is needed:** Controllers that can return a `Future`, and a tokio-backed executor for the HTTP/1.1 path as well, eliminating the fixed thread-count ceiling.

---

### 6. Typed request extractors

Every handler must manually call `request.body`, `String::from_utf8`, deserialize JSON, and extract fields. This is 5–10 lines of boilerplate before any business logic runs.

**Target API:**
```rust
fn create_user(body: Json<CreateUserRequest>, state: &AppState) -> Response {
    // body.name, body.email are already typed and validated
}
```

The framework deserializes and validates automatically; the handler only sees typed values.

---

### 7. Duplicate dispatch logic

`App::execute` and `App::handle_request` are nearly identical if/else chains over the same controllers. Adding one route requires editing both. This is an internal design issue that makes the framework brittle to extend and maintains two sources of truth for routing.

**Fix:** A single dispatch path used by both the server and test helpers.

---

### 8. Streaming responses / chunked transfer encoding

Every response body is fully assembled in memory before the first byte is sent. A 500 MB file allocates 500 MB of RAM and holds it for the entire write. Both `Transfer-Encoding: chunked` (HTTP/1.1) and the native stream framing in HTTP/2 and HTTP/3 need to be wired to an iterator or async stream that the controller produces incrementally.

This is also listed in [ROADMAP.md](ROADMAP.md) as Priority 1.

---

### 9. Typed error handling

`Application::execute` returns `Result<Response, String>`. Production code needs typed errors that carry their own HTTP status code, so a handler can return `Err(AppError::NotFound)` and the framework maps it to a 404 without the handler building the response manually.

**Target API:**
```rust
enum AppError {
    NotFound(String),
    Unauthorized,
    Internal(Box<dyn std::error::Error>),
}

impl IntoResponse for AppError { ... }
```

---

## Secondary gaps — painful in practice

### 10. Cookies

`Set-Cookie` header constant is defined; no parse or serialize implementation exists. Sessions, authentication tokens, and user preferences all require cookies. Needs a `Cookie` / `CookieJar` type with signed/encrypted variants.

### 11. Response compression

`Accept-Encoding` is never acted on. Text payloads (HTML, JSON, CSS, JS) are sent uncompressed. A gzip/brotli/zstd layer would reduce typical response sizes by 60–80 %. This is also listed in [ROADMAP.md](ROADMAP.md) as Priority 1.

### 12. `ConnectionInfo` peer address type

`ConnectionInfo.client.ip` is a `String` and `.client.port` is an `i32` instead of a `std::net::SocketAddr`. Users who need the actual address for logging or rate limiting must re-parse strings. The field should be a `SocketAddr`.

### 13. Incomplete graceful shutdown

`Server::run` (plain HTTP/1.1 thread pool) has no shutdown path — Ctrl+C kills the process mid-request. The thread pool does not drain in-flight work. The async paths (`run_tls`, `run_quic`) stop accepting new connections on Ctrl+C but do not wait for handlers to finish.

### 14. No test client

No structured way to fire test requests without a real TCP socket. `App::handle_request` is useful for unit tests but bypasses the server layer and has no ergonomic API for constructing requests with headers, query parameters, and typed bodies.

**Target API:**
```rust
let client = TestClient::new(app);
let res = client.get("/users/42")
    .header("Authorization", "Bearer token")
    .send();
assert_eq!(res.status(), 200);
```

### 15. No WebSocket support

No `Upgrade: websocket` handling. Real-time features (chat, live updates, collaborative editing) are not possible.

### 16. HTTP → HTTPS redirect

No plain-HTTP listener that issues `301 Moved Permanently` to the HTTPS equivalent. Deploying on port 80 and 443 simultaneously requires a separate proxy. Also listed in [ROADMAP.md](ROADMAP.md) as Priority 2.

---

## Summary

| # | Gap | Severity |
|---|-----|----------|
| 1 | Shared application state | Blocker |
| 2 | Dynamic routing with path parameters | Blocker |
| 3 | Middleware pipeline | Blocker |
| 4 | HTTP/1.1 keep-alive | Blocker |
| 5 | Async handlers | Major |
| 6 | Typed request extractors | Major |
| 7 | Duplicate dispatch logic | Major |
| 8 | Streaming responses | Major |
| 9 | Typed error handling | Major |
| 10 | Cookies | Secondary |
| 11 | Response compression | Secondary |
| 12 | `ConnectionInfo` uses `String` not `SocketAddr` | Secondary |
| 13 | Incomplete graceful shutdown | Secondary |
| 14 | No test client | Secondary |
| 15 | No WebSocket | Secondary |
| 16 | HTTP → HTTPS redirect | Secondary |
