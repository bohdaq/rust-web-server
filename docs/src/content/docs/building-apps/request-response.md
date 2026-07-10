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

## Response

`Response` is defined in `src/response/mod.rs`:

```rust
pub struct Response {
    pub http_version: String,             // "HTTP/1.1"
    pub status_code: i16,                 // 200, 404, etc.
    pub reason_phrase: String,            // "OK", "Not Found", etc.
    pub headers: Vec<Header>,
    pub content_range_list: Vec<ContentRange>, // response body parts
    pub stream_file: Option<String>,      // absolute path for chunked file streaming
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

Pass `None` for headers to get an empty header list. Pass `None` for the body list to get an empty body (useful for `204 No Content`).

### One-line JSON / text responses

For the common case — a JSON or plain-text body with no extra headers — `Response::json`/`Response::text` skip `get_response`/`Range::get_content_range` entirely:

```rust
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

let response = Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, b"{\"ok\":true}".to_vec());
let error_response = Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid input");
```

`Response::json` takes already-serialized bytes, so it has no dependency on the `serde` feature — build `bytes` with `serde_json::to_vec(&value)?` or hand-rolled JSON. See [JSON](/building-apps/json/) for the `serde`-backed `Json<T>` extractor/responder, which serializes a typed value for you in one call.

### Streaming large files

For files larger than ~8 MB, set `stream_file` instead of loading bytes into `content_range_list`. The server uses `Transfer-Encoding: chunked` and a constant memory footprint:

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
Header::_X_FORWARDED_FOR     // "X-Forwarded-For"
// ... all standard request and response headers
```

Create a header value pair:

```rust
use rust_web_server::header::Header;

let location = Header {
    name: Header::_LOCATION.to_string(),
    value: "/new-path".to_string(),
};
response.headers.push(location);
```

### get_header_list

`Header::get_header_list(request)` returns the standard set of response headers (CORS, cache control, security headers, client hints) pre-populated from the request context. Call it at the start of a controller's `process` method when building a response from scratch:

```rust
let mut header_list = Header::get_header_list(request);
header_list.push(Header {
    name: Header::_CACHE_CONTROL.to_string(),
    value: "no-store".to_string(),
});
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

Use `MimeType::detect_mime_type(filepath)` to derive the MIME type from a file extension at runtime.

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

For multipart range responses (e.g. `Accept-Ranges` support), push multiple `ContentRange` values into the list. The framework serialises them as `multipart/byteranges` automatically.
