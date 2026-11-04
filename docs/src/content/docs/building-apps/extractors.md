---
title: Typed Extractors
description: Parse request bodies, query strings, and headers into typed values using FromRequest.
---

## FromRequest trait

`FromRequest` is defined in `crates/rws-http/src/extract/mod.rs`:

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

### RequestId — correlation ID

The value of the `X-Request-Id` header (set by [`RequestIdLayer`](/building-apps/middleware/#built-in-middleware-layers)). Never fails; `.0` is an empty string when the header is absent.

```rust
use rust_web_server::extract::{RequestId, FromRequest};

let id = RequestId::from_request(request).unwrap();
let id: &str = id.as_str();
```

Two more extractors live outside `extract`: `json::Json<T>` (`serde` feature, see [JSON](/building-apps/json/)) and `validate::Validated<T>` (see [Validation](/building-apps/validation/)).

## Extractors as handler arguments

Since 18.28.0 a function annotated with a route attribute (`#[get("/path")]` & co., `macros` feature) takes extractors as arguments: each runs in order, the first failure answers with its rejection, and the function only sees values that were extracted. Registration and the compile-time path checks are on [Routing](/building-apps/routing/#typed-handlers-get-with-extractors). Everything below is in `rust_web_server::handler` (which also re-exports `Body`, `BodyText`, `RequestHeaders`, `RequestId` and `Json`):

```rust
use rust_web_server::handler::{Form, Path, Query, State};
use rust_web_server::{get, post};
use serde::Deserialize;

#[derive(Deserialize)]
struct Page { page: Option<u32>, tag: Vec<String> }

#[get("/shelves/{shelf}/books")]
fn books(Path(shelf): Path<u32>, Query(p): Query<Page>, db: &Db) -> String {
    format!("shelf {shelf}, page {}, tags {:?}", p.page.unwrap_or(1), p.tag)
}

#[derive(Deserialize)]
struct Signup { email: String, age: u8 }

#[post("/signup")]
fn signup(Form(form): Form<Signup>, State(db): State<Db>) -> (&'static StatusCodeReasonPhrase, String) {
    (STATUS_CODE_REASON_PHRASE.n201_created, form.email)
}
```

| Argument | Gives | Rejection |
|---|---|---|
| `Path<T>` | the route's parameters, percent-decoded: a scalar (`Path<u64>`), a tuple of `FromStr` values, or a `#[derive(FromPath)]` struct | `400` when one doesn't parse |
| `Query<T>` (`serde`) | the query string as a `Deserialize` struct (scalars, `String`, `Option` — empty is `None` — unit enums, `Vec` from a repeated key) | `400` |
| `Form<T>` (`serde`) | an `application/x-www-form-urlencoded` body, same field types | `415` for another `Content-Type`, `400` for a body that isn't UTF-8, `422` when the fields don't fit |
| `Json<T>` (`serde`) | the JSON body | `400` |
| `State<S>` / `&S` | the app's state (`State` holds a clone of its `Arc`) | — |
| `Body`, `BodyText`, `RequestHeaders`, `RequestId` | as above | `BodyText`: `400` |
| `&Request`, `&ConnectionInfo`, `&PathParams` | the raw request, connection and parameters | — |
| any `FromRequest` type (`Validated<T>`, `#[derive(FromRequest)]`, yours) | its value | its own |

Rejections are problem details, like rws's own errors (`error_format = "text"` makes them text). `extract::Query` stays the untyped map; `handler::Query<HashMap<String, String>>` works too.

The return type is anything `IntoResponse`: `Response`, `Json<T>`, `String` / `&'static str` (`200`, `text/plain`), `AppError`, `Result<T, E>` of two such types, or `(STATUS_CODE_REASON_PHRASE.n201_created, value)` to set the status.

### `#[derive(FromPath)]`

For several parameters by name, derive `FromPath` on a struct with one `FromStr` field per parameter, named after it. The route must have exactly those parameters — checked at compile time:

```rust
#[derive(rust_web_server::FromPath)]
struct PostKey { id: u64, slug: String }

#[get("/posts/{id}/{slug}")]
fn post(Path(key): Path<PostKey>) -> String {
    format!("{} {}", key.id, key.slug)
}
```

A type of your own that implements `FromStr` works as a one-tuple, `Path((slug,)): Path<(Slug,)>`, or implement `handler::FromPath` for it. Your own argument type with access to the state implements `handler::FromRequestParts<S>`.

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

## `#[derive(FromRequest)]`

The `macros` feature can generate the boilerplate above for a named-field struct where every field is itself already an extractor — each field's `from_request` runs in declaration order, and the first failure short-circuits with that field's error response:

```toml
# Cargo.toml
rust-web-server = { version = "18", features = ["macros"] }
```

```rust
use rust_web_server::extract::{BodyText, Query};

#[derive(rust_web_server::FromRequest)]
struct Payload {
    body: BodyText,
    params: Query,
}

// let Payload { body, params } = Payload::from_request(request)?;
```

This covers structs built from existing extractors (`Body`, `BodyText`, `Query`, `RequestHeaders`, `RequestId`, or your own custom ones). A field like `BearerToken` above works too, as long as it implements `FromRequest` itself — the derive doesn't know how to pull an arbitrary field from a query parameter or header on its own; write a small custom extractor (as shown above) for that and include it as a field.
