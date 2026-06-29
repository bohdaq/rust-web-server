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

`ConnectionInfo` is passed into every controller and carries the peer address.

```rust
fn process(_request: &Request, mut response: Response, connection: &ConnectionInfo) -> Response {
    println!("request from {}:{}", connection.peer_addr.ip(), connection.peer_addr.port());
    // ...
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
