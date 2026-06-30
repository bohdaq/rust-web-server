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
