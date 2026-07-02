---
title: Typed Extractors
description: Parse request bodies, query strings, and headers into typed values using FromRequest.
---

## FromRequest trait

`FromRequest` is defined in `src/extract/mod.rs`:

```rust
pub trait FromRequest: Sized {
    fn from_request(request: &Request) -> Result<Self, Response>;
}
```

Call `T::from_request(request)` to extract a typed value from the incoming request. On failure the implementation returns a ready-to-send `Response` (usually `400 Bad Request`) so handlers can early-return without duplicating error-building code.

## Built-in extractors

### Body — raw bytes

Clones the raw request body into a `Vec<u8>`. Never fails; an empty body produces an empty `Vec`.

```rust
use rust_web_server::extract::{Body, FromRequest};

fn handler(request: &Request) -> Response {
    let Body(bytes) = Body::from_request(request).unwrap();
    // bytes is Vec<u8>
    todo!()
}
```

`Body` also exposes `.into_bytes(self) -> Vec<u8>` for consuming the wrapper.

### BodyText — UTF-8 decoded body

Returns `400 Bad Request` if the body bytes are not valid UTF-8.

```rust
use rust_web_server::extract::{BodyText, FromRequest};
use rust_web_server::error::{AppError, IntoResponse};

fn handler(request: &Request) -> Response {
    let text = match BodyText::from_request(request) {
        Ok(BodyText(s)) => s,
        Err(err_response) => return err_response,
    };
    // text is String
    todo!()
}
```

`BodyText` exposes `.as_str(&self) -> &str` for borrowing.

### Query — parsed query string

Parses the query string from `request.request_uri` into a `HashMap<String, String>`. Never fails; a URI with no query string produces an empty map.

```rust
use rust_web_server::extract::{Query, FromRequest};

fn handler(request: &Request) -> Response {
    let q = Query::from_request(request).unwrap();
    let page = q.get("page").map(String::as_str).unwrap_or("1");
    let limit = q.get("limit").map(String::as_str).unwrap_or("20");
    // ...
    todo!()
}
```

`Query` exposes `.get(&self, key: &str) -> Option<&String>`.

### RequestHeaders — all request headers

Clones the entire header list. Never fails.

```rust
use rust_web_server::extract::{RequestHeaders, FromRequest};

fn handler(request: &Request) -> Response {
    let headers = RequestHeaders::from_request(request).unwrap();
    let auth = headers.get("Authorization"); // case-insensitive lookup
    // ...
    todo!()
}
```

`RequestHeaders::get(&self, name: &str) -> Option<&str>` performs a case-insensitive lookup and returns the value of the first matching header.

## Combining extractors

Extractors are ordinary function calls; combine as many as you need:

```rust
use rust_web_server::extract::{BodyText, Query, RequestHeaders, FromRequest};

fn create_item(request: &Request, _params: &PathParams, _conn: &ConnectionInfo, _state: &()) -> Response {
    // Early-return on bad body
    let BodyText(body) = match BodyText::from_request(request) {
        Ok(b) => b,
        Err(r) => return r,
    };

    let query = Query::from_request(request).unwrap();
    let headers = RequestHeaders::from_request(request).unwrap();

    let dry_run = query.get("dry_run").map(|v| v == "true").unwrap_or(false);
    let trace_id = headers.get("X-Trace-Id").unwrap_or("none");

    // process body, dry_run, trace_id ...
    todo!()
}
```

## Writing a custom extractor

Implement `FromRequest` for any type that can be derived from a `Request`:

```rust
use rust_web_server::extract::FromRequest;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::header::Header;
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

/// Extracts a Bearer token from the Authorization header.
pub struct BearerToken(pub String);

impl FromRequest for BearerToken {
    fn from_request(request: &Request) -> Result<Self, Response> {
        let auth = request
            .get_header("Authorization".to_string())
            .and_then(|h| h.value.strip_prefix("Bearer ").map(str::to_string));

        match auth {
            Some(token) => Ok(BearerToken(token)),
            None => {
                let header_list = Header::get_header_list(request);
                let body = Range::get_content_range(
                    b"Missing or invalid Authorization header".to_vec(),
                    MimeType::TEXT_PLAIN.to_string(),
                );
                Err(Response::get_response(
                    STATUS_CODE_REASON_PHRASE.n401_unauthorized,
                    Some(header_list),
                    Some(vec![body]),
                ))
            }
        }
    }
}

// Usage:
// let BearerToken(token) = BearerToken::from_request(request)?;
```

:::caution[Coming Soon]
`#[derive(FromRequest)]` for named-field structs (reads each field from a corresponding query parameter or header) is planned via the `macros` feature flag but is not yet implemented. Use the manual implementation pattern above in the meantime.
:::
