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
| `Router` / `PathParams` | `router` | Standalone dynamic router with `:param` and `*wildcard` path matching |
| `IntoResponse` / `AppError` | `error` | Typed errors that map to HTTP status codes |
| `TestClient` | `test_client` | In-process HTTP test client — no TCP socket required |
| `FromRequest` / `Body` / `BodyText` / `Query` | `extract` | Typed request extractors — parse body or query params, returning a ready error on failure |
| `RateLimiter` | `rate_limit` | Per-IP sliding-window rate limiter; `global()` reads config from env vars |
| `WebSocket` / `Frame` | `websocket` | RFC 6455 WebSocket handshake, frame read/write; SHA-1 and base64 built in |
| `AppWithState<S>` | `state` | State-aware application with built-in dynamic routing; state shared via `Arc<S>`. Use `App::with_state(S)` as the entry point. |
| `Middleware` / `WithMiddleware` | `middleware` | Composable middleware pipeline wrapping any `Application`. Use `App::new().wrap(layer)` or `AppWithState::wrap(layer)`. |
| `RateLimitLayer` | `middleware` | Built-in middleware that enforces the global rate limiter per client IP |
| `AsyncAppWithState<S>` | `async_state` | Like `AppWithState<S>` but handlers are `async fn`; requires `http2` feature. Entry point: `App::with_async_state(S)`. |
| `Sse` / `SseEvent` | `sse` | Build a buffered `text/event-stream` response from a sequence of events. Correct headers set automatically. |
| `SessionStore` / `Session` | `session` | Thread-safe in-memory session store with TTL expiry. Cookie helpers: `session_id_from_request`, `session_cookie`, `destroy_cookie`. |
| `Json<T>` | `json` | Serde-backed JSON extractor (`from_request`) and responder (`into_response`). Requires `features = ["serde"]`. |
| `BasicAuthLayer<F>` | `auth` | HTTP Basic Auth middleware; validates `Authorization: Basic` credentials via a closure. Requires `features = ["auth"]`. |
| `JwtLayer` | `auth` | JWT HS256 middleware; verifies `Authorization: Bearer` tokens with constant-time HMAC-SHA256. Requires `features = ["auth"]`. |
| `build_jwt` / `verify_jwt` / `Claims` | `auth` | Sign and verify HS256 JWTs; `Claims` exposes `sub`, `exp`, and raw JSON payload. |
| `IpFilter` | `ip_filter` | Allow/deny middleware keyed on client IPv4 address or CIDR range. `IpFilter::allow([...])` passes only listed addresses; `IpFilter::deny([...])` blocks them. |
| `routes!` | `macros` | Declarative routing macro — builds `AppWithState`, `AsyncAppWithState`, or `Router` from a `METHOD "path" => handler` table. |
| `#[route]`, `#[get]`, `#[post]`, … | `macros` (proc-macro) | Attribute macros that annotate handler functions with their HTTP method and path. Requires `features = ["macros"]`. |
| `#[derive(FromRequest)]` | `macros` (proc-macro) | Derive `FromRequest` for a named-field struct; calls `from_request` on each field in declaration order, short-circuiting on the first error. Requires `features = ["macros"]`. |
| `Validate` / `ValidationErrors` | `validate` | Field-level validation trait; `ValidationErrors` collects all failures before returning. Implement manually or derive. |
| `Validated<T>` | `validate` | `FromRequest` wrapper — extracts then validates in one step; `400` on extraction failure, `422 Unprocessable Entity` with JSON error body on validation failure. |
| `is_email` / `is_url` | `validate` | Format check helpers used by the derive macro; callable directly. |
| `#[derive(Validate)]` | `macros` (proc-macro) | Derive `Validate` from `#[validate(...)]` field annotations. Validators: `length(min,max)`, `range(min,max)`, `email`, `required`, `url`. Requires `features = ["macros"]`. |

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

- `rws_requests_total` — total HTTP requests handled
- `rws_errors_total` — requests that returned an application error
- `rws_active_connections` — currently open connections

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
