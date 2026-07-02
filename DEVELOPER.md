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
| `Router` / `PathParams` | `router` | Standalone dynamic router with `:param` and `*wildcard` path matching; `.with_host(name)` restricts a router to one virtual host |
| `VirtualHostConfig` | `virtual_host` | Per-domain cert configuration `{ domain, cert_file, key_file }` for multi-domain SNI routing |
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
| `ReverseProxy` | `proxy` | Middleware that forwards requests to HTTP backends with round-robin load balancing and automatic failover. Returns `502` when all backends fail. |
| `RewriteLayer` | `rewrite` | Composable request/response rewriting middleware: request header add/replace/remove, URI set/strip-prefix/add-prefix, response header add/replace/remove, status override, body byte find-and-replace. |
| `LoadBalancing` | `proxy` | Enum selecting the balancing strategy (`RoundRobin`). Passed to `ReverseProxy::strategy()`. |
| `MetricsLayer` | `metrics` | Middleware that records per-route request counts and latency histograms. Adds `rws_route_requests_total{method,path,status}` and `rws_route_duration_seconds{method,path}` to `/metrics`. |
| `CacheLayer` | `cache` | In-memory TTL response cache middleware for GET requests. Builder: `.ttl(secs)`, `.vary_by_header(name)`. Injects `Age` on hits; respects `Cache-Control: no-store/private`. |
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
| `H2ReverseProxy` | `proxy` | HTTP/2 upstream reverse proxy middleware. Round-robin backend selection; `h2://` scheme prefix supported. Requires `http2` feature. |
| `GrpcProxy` | `proxy` | gRPC reverse proxy middleware — filters on `Content-Type: application/grpc*` and forwards over HTTP/2. Requires `http2` feature. |
| `TcpProxy` | `tcp_proxy` | Standalone L4 TCP proxy. Accepts connections on a local address and relays bytes bidirectionally to round-robin backends. |
| `UdpProxy` | `udp_proxy` | Standalone UDP proxy (request-reply model). Forwards each datagram to a backend and returns the reply to the original client. |
| `WsProxy` | `ws_proxy` | Standalone WebSocket proxy. Listens on a local address, upgrades client connections, and relays WebSocket frames bidirectionally to backends. |
| `WeightedBackend` / `CanaryLayer` | `canary` | Weighted traffic-splitting proxy middleware; each backend has a `weight` — distribution is proportional. Useful for canary releases and A/B testing. |
| `CircuitBreaker` | `circuit_breaker` | Per-backend circuit breaker (Closed→Open→HalfOpen state machine). `global()` returns a process-wide singleton. Configurable failure threshold and recovery window. |
| `RetryLayer` | `circuit_breaker` | Middleware that retries requests on configurable status codes (default: 502, 503, 504) up to `max_retries` times. |
| `BackendPool` / `DiscoverySource` | `service_discovery` | Dynamic backend pool updated by a background thread. Sources: `Static`, `EnvPrefix` (env vars), `File` (one host:port per line), `Dns` (A-record lookup). |
| `IngressRule` / `KubernetesIngressWatcher` / `IngressRouter` | `ingress` | Kubernetes Ingress watcher: polls the K8s API, parses Ingress rules, and routes requests to the correct upstream service. `IngressRouter` implements `Application`. |
| `Scheduler` / `CronSchedule` | `scheduler` | `@Scheduled`-equivalent background task runner. Three modes: `.every(Duration, fn)` (fixed rate), `.after(Duration, fn)` (fixed delay), `.cron("sec min hour day month weekday", fn)`. Full cron syntax: `*`, exact, `*/step`, `N-M`, comma list. |
| `TeraEngine` | `template` (requires `tera` feature) | Jinja2/Django HTML template engine. `from_dir(dir)` loads disk templates; `from_raw(&[(name, src)])` for inline templates. Global singleton via `template::init(dir)` / `template::render(name, &ctx)`. |
| `#[derive(Config)]` / `FromEnvStr` | `config_binding` (requires `macros` feature for derive) | Typed env-var binding. Generates `load() -> Result<Self, String>`. `#[config(env = "KEY", default = "v")]` per field; `Option<T>` for optional; struct-level `#[config(prefix = "APP_")]`. Implement `FromEnvStr` for custom types. |
| `ProxyConfig` / `ConfigDrivenApp` / `build_from_file` | `proxy_config` | Config-driven proxy server. `ProxyConfig::is_proxy_mode()` detects `[[route]]` / `[[upstream]]` sections in `rws.config.toml`; `build_from_file()` returns a `ConfigDrivenApp` (first-match router over `Arc<Vec<CompiledRoute>>`) plus L4/WS proxy thread handles. Per-route middleware: `PerRouteRateLimit`, `BearerAuthMiddleware`, `RewriteLayer`, `CacheLayer`, `IpFilter`. `DynamicProxy` performs health-aware round-robin proxying. |
| `Container` | `di` | Type-keyed dependency injection container. `register::<T>(service)` stores concrete types; `provide::<dyn Trait>(Arc::new(...))` stores trait objects; both keyed by `TypeId`. Named services via `register_named` / `provide_named` / `get_named`. Share across handlers with `container.into_arc()` as `AppWithState` state. |

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

`WsProxy` listens for HTTP upgrade requests, performs the WebSocket handshake with the client, connects to a backend, and relays frames bidirectionally in a two-thread relay loop.

```rust
use rust_web_server::ws_proxy::WsProxy;

WsProxy::new(["ws-backend:8080"])
    .connect_timeout_ms(500)
    .read_timeout_ms(30_000)
    .bind("0.0.0.0:9000")
    .expect("WS proxy failed");
```

`bind()` blocks indefinitely. The proxy does raw byte relay after the handshake, so any WebSocket subprotocol passes through transparently.

---

### 42. HTTP/2 reverse proxy

`H2ReverseProxy` forwards requests to HTTP/2 backends. It works as a `Middleware` in the normal stack; `block_in_place` bridges the sync handler into the tokio runtime. Requires the `http2` feature.

```rust
#[cfg(feature = "http2")]
{
    use rust_web_server::app::App;
    use rust_web_server::core::New;
    use rust_web_server::proxy::H2ReverseProxy;

    let app = App::new()
        .wrap(H2ReverseProxy::new(["h2://backend1:8443", "h2://backend2:8443"])
            .path_prefix("/api")
            .connect_timeout_ms(1000)
            .read_timeout_ms(5000));
}
```

Requests whose URI does not start with `path_prefix` pass through to the next middleware. `X-Forwarded-For` and `Via` are injected automatically.

---

### 43. gRPC proxy

`GrpcProxy` wraps `H2ReverseProxy` and filters on `Content-Type: application/grpc*`. All gRPC traffic is forwarded over HTTP/2; non-gRPC requests fall through to the next handler. Requires the `http2` feature.

```rust
#[cfg(feature = "http2")]
{
    use rust_web_server::app::App;
    use rust_web_server::core::New;
    use rust_web_server::proxy::GrpcProxy;

    let app = App::new()
        .wrap(GrpcProxy::new(["grpc-service:50051"])
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
| `static` | Built-in static file controller |
| `redirect` | 301/302 with a `Location` header; `$path` interpolated |
| `respond` | Fixed status + body (catch-all 404, maintenance page) |
| `mcp` | Built-in MCP Streamable HTTP server |

**Per-route middleware keys**

| Key | Example |
|---|---|
| `rate_limit` | `{ max_requests = 500, window_secs = 60 }` |
| `cache` | `{ ttl_secs = 3600, vary_by = ["Accept-Encoding"] }` |
| `auth` | `{ type = "bearer", token_env = "API_TOKEN" }` |
| `ip_allow` / `ip_deny` | `["192.168.1.0/24", "10.0.0.1"]` |
| `rewrite.request[]` | `[{ type = "header_set", name = "X-Real-IP", value = "$client_ip" }]` |
| `rewrite.response[]` | `[{ type = "header_remove", name = "Server" }]` |

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

`Container` is a type-keyed service store. Register services at startup; resolve them in handlers via `Arc<Container>` passed as `AppWithState` state.

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

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::di::Container;
use rust_web_server::routes;

fn get_user(
    req: &Request,
    params: &PathParams,
    _conn: &ConnectionInfo,
    state: &Arc<Container>,
) -> Response {
    let repo = state.get::<dyn UserRepository>().unwrap();
    // use repo.find(...)
    Response::new()
}

let mut container = Container::new();
container.provide::<dyn UserRepository>(Arc::new(PgUserRepository));
// register more services...

let app = routes! {
    App::with_state(container.into_arc()),
    GET "/users/:id" => get_user,
};
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
