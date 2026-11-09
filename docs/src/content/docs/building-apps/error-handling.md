---
title: Error Handling
description: Map Rust errors to HTTP responses using IntoResponse and the built-in AppError enum.
---

## IntoResponse trait

`IntoResponse` is defined in `crates/rws-http/src/error/mod.rs`:

```rust
pub trait IntoResponse {
    fn into_response(self) -> Response;
}
```

Implement this on your error enum so handlers can return `Result<Response, MyError>` and convert the error to an HTTP response with `.unwrap_or_else(|e| e.into_response())`.

`Response` itself implements `IntoResponse` as the identity conversion, so it can be used wherever `impl IntoResponse` is expected.

## AppError enum

`AppError` is a built-in typed error that covers the most common HTTP failure cases:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum AppError {
    BadRequest(String),         // 400
    Unauthorized,               // 401
    Forbidden,                  // 403
    NotFound(String),           // 404
    Conflict(String),           // 409
    UnprocessableEntity(String), // 422
    TooManyRequests,            // 429
    PayloadTooLarge(String),    // 413
    Internal(String),           // 500
}
```

Each variant's `into_response()` produces a plain-text body from the attached message, with the appropriate status code and a standard header set.

## Returning Result from handlers

Handlers registered on `AppWithState` must return `Response`, not `Result`. Use `.unwrap_or_else` to fold the error into the response at the call site:

```rust
use rust_web_server::error::{AppError, IntoResponse};
use rust_web_server::response::Response;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

fn find_user(id: u64) -> Result<Response, AppError> {
    if id == 0 {
        return Err(AppError::NotFound("user not found".to_string()));
    }
    // ... real lookup
    Err(AppError::Internal("db connection failed".to_string()))
}

fn get_user(
    _req: &Request,
    params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let id: u64 = params
        .get("id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    find_user(id).unwrap_or_else(|e| e.into_response())
}
```

## Custom error type

Implement `IntoResponse` on your own error enum when `AppError` does not map cleanly to your domain:

```rust
use rust_web_server::error::IntoResponse;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::header::Header;
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

#[derive(Debug)]
pub enum ApiError {
    Validation(String),
    DatabaseError(String),
    NotAuthenticated,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Validation(msg)    => (STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, msg.as_str()),
            ApiError::DatabaseError(msg) => (STATUS_CODE_REASON_PHRASE.n500_internal_server_error, msg.as_str()),
            ApiError::NotAuthenticated   => (STATUS_CODE_REASON_PHRASE.n401_unauthorized, "Not authenticated"),
        };

        // Build a minimal fake request so Header::get_header_list has something to work from.
        use rust_web_server::request::Request;
        let dummy = Request {
            method: "GET".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![],
            body: Default::default(),
        };
        let headers = Header::get_header_list(&dummy);
        let body = Range::get_content_range(
            message.as_bytes().to_vec(),
            MimeType::TEXT_PLAIN.to_string(),
        );
        Response::get_response(status, Some(headers), Some(vec![body]))
    }
}
```

Then use it in handlers exactly like `AppError`:

```rust
fn handler(...) -> Response {
    do_something().unwrap_or_else(|e: ApiError| e.into_response())
}
```

## Error pages

A handler that panics, or an app that returns `Err`, is answered by the server with a bare `500`. `ErrorPages` decides what the client sees instead, by [`RWS_ENV`](/configuration/env-vars/#environment-kind) (18.35.0):

```rust
use rust_web_server::error_pages::ErrorPages;

let app = App::with_state(db).get("/", home).wrap(ErrorPages::from_env());
```

| `RWS_ENV` | A browser (`Accept: text/html`) | Anything else |
|---|---|---|
| `dev`, `test` | A page with the panic message, where it happened, the request (cookie and authorization headers hidden) and the backtrace | Problem details whose `detail` is the message, plus `location` and `backtrace` (an array of lines) |
| `prod` (or unset) | `500` problem details without a detail | the same |

Either way the details go to stderr. `ErrorPages::detailed(bool)` sets it regardless of `RWS_ENV`. With details on, every request goes through the layer on the blocking pool (async handlers too) so their panics can be caught; with them off, requests pass straight through. The backtrace is captured by a panic hook the layer adds in front of the existing one (once per process); a panic in an async handler shows its message, and its backtrace when it ran on the same thread.

## AppError variant reference

| Variant | Status | Body |
|---|---|---|
| `BadRequest(msg)` | 400 | `msg` |
| `Unauthorized` | 401 | `"Unauthorized"` |
| `Forbidden` | 403 | `"Forbidden"` |
| `NotFound(msg)` | 404 | `msg` |
| `Conflict(msg)` | 409 | `msg` |
| `UnprocessableEntity(msg)` | 422 | `msg` |
| `TooManyRequests` | 429 | `"Too Many Requests"` |
| `PayloadTooLarge(msg)` | 413 | `msg` |
| `Internal(msg)` | 500 | `msg` |

:::note[Content-Type]
`AppError::into_response()` always sets `Content-Type: text/plain`. To return JSON error bodies, implement your own `IntoResponse` and pass `MimeType::APPLICATION_JSON` to `Range::get_content_range`.
:::
