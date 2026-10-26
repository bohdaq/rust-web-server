---
title: Request & Response
description: The Request and Response types, status code constants, MIME type constants, and how to build response bodies.
---

## Request

`Request` is defined in `src/request/mod.rs`:

```rust
pub struct Request {
    pub method: String,       // "GET", "POST", etc.
    pub request_uri: String,  // "/search?q=rust"
    pub http_version: String, // "HTTP/1.1"
    pub headers: Vec<Header>,
    pub body: Vec<u8>,        // raw bytes
}
```

### METHOD constants

Compare `request.method` against the `METHOD` constant instead of string literals:

```rust
use rust_web_server::request::{METHOD, Request};

fn is_get(request: &Request) -> bool {
    request.method == METHOD.get
}
```

Available fields: `METHOD.get`, `METHOD.head`, `METHOD.post`, `METHOD.put`, `METHOD.delete`, `METHOD.connect`, `METHOD.options`, `METHOD.trace`, `METHOD.patch`.

### Helper methods

```rust
// Parse the query string into a HashMap<String, String>
let query: Option<HashMap<String, String>> = request.get_query().unwrap();

// Extract just the path, stripping the query string
let path: String = request.get_path().unwrap();

// Get a specific header (case-insensitive)
let content_type: Option<&Header> = request.get_header("Content-Type".to_string());

// Extract the Host header's domain component
let domain: Option<String> = request.get_domain().unwrap();
```

`request.header(name)` is the allocation-free variant of `get_header`: it takes a `&str` and also matches case-insensitively.

```rust
use rust_web_server::header::Header;

match request.header(Header::_AUTHORIZATION) {
    Some(h) => println!("Authorization: {}", h.value),
    None    => { /* not present */ }
}
```

### Client address

`ConnectionInfo`, passed to every controller and handler, carries the peer address. `peer_addr()` returns it as a `std::net::SocketAddr`; the raw `client.ip` / `client.port` fields are also public:

```rust
fn process(_request: &Request, response: Response, connection: &ConnectionInfo) -> Response {
    if let Some(addr) = connection.peer_addr() {
        println!("request from {}", addr);   // e.g. "127.0.0.1:54321"
    }
    println!("ip={} port={}", connection.client.ip, connection.client.port);
    response
}
```

This is the TCP peer. Behind a load balancer it is the balancer's address; `X-Forwarded-For` is not consulted.

## Response

`Response` is defined in `src/response/mod.rs`:

```rust
pub struct Response {
    pub http_version: String,             // "HTTP/1.1"
    pub status_code: i16,                 // 200, 404, etc.
    pub reason_phrase: String,            // "OK", "Not Found", etc.
    pub headers: Vec<Header>,
    pub content_range_list: Vec<ContentRange>, // response body parts
    pub stream_file: Option<String>,      // path of a file streamed from disk
    pub stream_pipe: Option<Box<dyn std::io::Read + Send>>, // reader piped chunked; wins over both
}
```

### Building a response with get_response

The most concise way to build a response:

```rust
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::header::Header;
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

let body = Range::get_content_range(
    b"{\"ok\":true}".to_vec(),
    MimeType::APPLICATION_JSON.to_string(),
);

let response = Response::get_response(
    STATUS_CODE_REASON_PHRASE.n200_ok,
    Some(Header::get_header_list(request)), // standard response headers
    Some(vec![body]),
);
```

`Response::get_response` signature:

```rust
pub fn get_response(
    status_code_reason_phrase: &StatusCodeReasonPhrase,
    boxed_header_list: Option<Vec<Header>>,
    boxed_content_range_list: Option<Vec<ContentRange>>,
) -> Response
```

Pass `None` for headers to get an empty header list. Pass `None` for the body list to get an empty body (useful for `204 No Content`). A `1xx`, `204` or `304` response is always sent without a body or `Content-Length`, whatever the handler put in it (`Response::drop_content_for_status`; since 17.113.2). A `HEAD` response keeps the `Content-Length` of the `GET` it stands for, without the body.

### One-line JSON / text responses

For the common case — a JSON or plain-text body with no extra headers — `Response::json`/`Response::text` skip `get_response`/`Range::get_content_range` entirely:

```rust
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

let response = Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, b"{\"ok\":true}".to_vec());
let error_response = Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid input");
```

`Response::json` takes already-serialized bytes, so it has no dependency on the `serde` feature — build `bytes` with `serde_json::to_vec(&value)?` or hand-rolled JSON. See [JSON](/building-apps/json/) for the `serde`-backed `Json<T>` extractor/responder, which serializes a typed value for you in one call.

### Redirects

Set a 3xx status and a `Location` header:

```rust
use rust_web_server::header::Header;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
    response.status_code = *STATUS_CODE_REASON_PHRASE.n301_moved_permanently.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n301_moved_permanently.reason_phrase.to_string();
    response.headers.push(Header::new(Header::_LOCATION, "https://example.com/new-path"));
    response
}
```

`n302_found`, `n303_see_other`, `n307_temporary_redirect` and `n308_permanent_redirect` work the same way.

### Streaming large files

For large files, set `stream_file` instead of loading bytes into `content_range_list` (the static file server does this above 256 KiB, `StaticResourceController::STREAM_THRESHOLD`). Memory stays constant: the body goes out as-is when you set a `Content-Length` header, otherwise with `Transfer-Encoding: chunked`. To stream from any `Read` source, set `stream_pipe` instead:

```rust
response.stream_file = Some("/var/data/large-export.csv".to_string());
```

## STATUS_CODE_REASON_PHRASE constants

The `STATUS_CODE_REASON_PHRASE` constant provides typed access to every standard HTTP status code:

```rust
use rust_web_server::response::STATUS_CODE_REASON_PHRASE;

STATUS_CODE_REASON_PHRASE.n200_ok
STATUS_CODE_REASON_PHRASE.n201_created
STATUS_CODE_REASON_PHRASE.n204_no_content
STATUS_CODE_REASON_PHRASE.n400_bad_request
STATUS_CODE_REASON_PHRASE.n401_unauthorized
STATUS_CODE_REASON_PHRASE.n403_forbidden
STATUS_CODE_REASON_PHRASE.n404_not_found
STATUS_CODE_REASON_PHRASE.n409_conflict
STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity
STATUS_CODE_REASON_PHRASE.n429_too_many_requests
STATUS_CODE_REASON_PHRASE.n500_internal_server_error
// ... and all other 1xx–5xx codes
```

Each entry has `.status_code: &'static i16` and `.reason_phrase: &'static str`. Set them on a response:

```rust
response.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
response.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
```

## Header constants

`Header` in `src/header/mod.rs` exposes string constants for all standard HTTP header names:

```rust
use rust_web_server::header::Header;

Header::_CONTENT_TYPE        // "Content-Type"
Header::_CONTENT_LENGTH      // "Content-Length"
Header::_CONTENT_RANGE       // "Content-Range"
Header::_AUTHORIZATION       // "Authorization"
Header::_CACHE_CONTROL       // "Cache-Control"
Header::_LOCATION            // "Location"
Header::_SET_COOKIE          // "Set-Cookie"
Header::_FORWARDED           // "Forwarded"
// ... all standard request and response headers
```

Create a header value pair:

```rust
use rust_web_server::header::Header;

// name and value are Cow<'static, str> (18.0.0): a &'static str is borrowed,
// a String (e.g. format!(..)) is owned. Read them as &str via deref.
let location = Header::new(Header::_LOCATION, "/new-path");
response.headers.push(location);
```

### get_header_list

`Header::get_header_list(request)` returns the standard set of response headers (CORS, `Date`, cache control, security headers; client hints only with `RWS_CONFIG_CLIENT_HINTS=true`) pre-populated from the request context. Call it at the start of a controller's `process` method when building a response from scratch:

```rust
let mut header_list = Header::get_header_list(request);
header_list.push(Header::new(Header::_CACHE_CONTROL, "no-store"));
```

## MimeType constants

`MimeType` in `src/mime_type/mod.rs` provides string constants for common content types:

```rust
use rust_web_server::mime_type::MimeType;

MimeType::APPLICATION_JSON          // "application/json"
MimeType::APPLICATION_OCTET_STREAM  // "application/octet-stream"
MimeType::TEXT_PLAIN                // "text/plain"
MimeType::TEXT_HTML                 // "text/html"
MimeType::TEXT_CSS                  // "text/css"
MimeType::TEXT_JAVASCRIPT           // "text/javascript"
// ... and many more
```

Use `MimeType::detect_mime_type(filepath)` to derive the MIME type from a file extension at runtime:

```rust
let mime = MimeType::detect_mime_type("/assets/app.wasm");   // "application/wasm"
let mime = MimeType::detect_mime_type("/styles/main.css");   // "text/css"
let mime = MimeType::detect_mime_type("/data/feed.json");    // "application/json"
```

## Range::get_content_range

Wrap any `Vec<u8>` in a `ContentRange` for use in `response.content_range_list`:

```rust
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

let body = Range::get_content_range(
    serde_json::to_vec(&payload).unwrap(),
    MimeType::APPLICATION_JSON.to_string(),
);
response.content_range_list = vec![body];
```

For multipart range responses (e.g. `Accept-Ranges` support), push multiple `ContentRange` values into the list. The framework serialises them as `multipart/byteranges` automatically, with a `Content-Length` and a boundary that occurs in none of the parts (`Range::multipart_boundary`; since 17.113.2).
