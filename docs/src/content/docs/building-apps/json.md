---
title: JSON
description: Parse JSON request bodies and produce JSON responses with the built-in parser or the serde-backed Json<T> extractor.
---

`rust-web-server` ships two JSON layers:

1. **Built-in parser** (`rust_web_server::json::object::JSON`) — a zero-dependency, hand-rolled JSON parser that handles strings, numbers, booleans, null, nested objects, and arrays. No third-party crates required.
2. **`Json<T>` extractor / responder** (`rust_web_server::json::Json`) — backed by `serde_json`; requires `features = ["serde"]`. Use this for typed request/response structs.

## `Json<T>` extractor and responder (recommended)

Enable the `serde` feature in `Cargo.toml`:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
```

### Deserializing a request body

```rust
use serde::Deserialize;
use rust_web_server::json::Json;
use rust_web_server::extract::FromRequest;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

#[derive(Deserialize)]
struct CreatePost {
    title:   String,
    body:    String,
    tags:    Vec<String>,
}

fn create_post(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let Json(payload) = match Json::<CreatePost>::from_request(req) {
        Ok(j)  => j,
        Err(r) => return r,  // 400 Bad Request on parse failure
    };

    println!("New post: {}", payload.title);

    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}
```

`Json::<T>::from_request` calls `serde_json::from_slice` on `request.body` and returns a `400 Bad Request` response (with a human-readable error message as the body) on any parse failure.

`Json<T>` also implements `FromRequest` directly, so it works with `#[derive(FromRequest)]`:

```rust
use rust_web_server::extract::FromRequest;

#[derive(rust_web_server::FromRequest)]
struct Payload {
    body: Json<CreatePost>,
}
```

### Serializing a response

```rust
use serde::Serialize;
use rust_web_server::json::Json;
use rust_web_server::response::Response;

#[derive(Serialize)]
struct PostResponse {
    id:    u64,
    title: String,
    slug:  String,
}

fn get_post(
    _req: &Request,
    params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let post = PostResponse {
        id:    42,
        title: "Hello".to_string(),
        slug:  "hello".to_string(),
    };

    Json(post).into_response()
    // Sets Content-Type: application/json and status 200 OK
}
```

`Json<T>::into_response` uses `serde_json::to_vec`. If serialisation fails (extremely rare for well-formed `Serialize` implementations), it returns `500 Internal Server Error`.

### Dereferencing

`Json<T>` implements `Deref` and `DerefMut` to `T`, so you can access fields without unwrapping:

```rust
let Json(payload) = Json::<CreatePost>::from_request(req)?;
// or
let json = Json::<CreatePost>::from_request(req)?;
println!("{}", json.title);  // via Deref
```

## Setting `Content-Type: application/json` manually

When building a response by hand, set the content type through `Range::get_content_range`:

```rust
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::core::New;

let json_body = br#"{"status":"ok"}"#.to_vec();

let mut r = Response::new();
r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
r.content_range_list = vec![
    Range::get_content_range(json_body, MimeType::APPLICATION_JSON.to_string())
];
```

## Built-in dynamic parser (`JSON` / `ToJSON` / `FromJSON`)

For cases where you cannot use serde or need to introspect JSON without a fixed schema, the built-in `JSON` struct in `rust_web_server::json::object` parses JSON into a property list.

```rust
use rust_web_server::json::object::JSON;

let raw = r#"{"name":"Alice","age":30,"active":true}"#.to_string();

let properties = JSON::parse_as_properties(raw).unwrap();
for (prop, value) in &properties {
    match prop.property_type.as_str() {
        "String" => println!("{} = {:?}", prop.property_name, value.string),
        "i128"   => println!("{} = {:?}", prop.property_name, value.i128),
        "bool"   => println!("{} = {:?}", prop.property_name, value.bool),
        _        => {}
    }
}
```

Each parsed entry is a `(JSONProperty, JSONValue)` pair.

`JSONProperty` has:
- `property_name: String`
- `property_type: String` — one of `"String"`, `"bool"`, `"i128"`, `"f64"`, `"object"`, `"array"`, `"null"`

`JSONValue` has optional fields for each type: `.string`, `.bool`, `.i128`, `.f64`, `.object`, `.array`, `.null`.

To serialise a property list back to a JSON string:

```rust
use rust_web_server::json::object::JSON;

let json_string = JSON::to_json_string(properties);
```

The `ToJSON` and `FromJSON` traits (also in `rust_web_server::json::object`) provide an interface for implementing dynamic JSON serialisation on your own types without serde.

## Error handling summary

| Scenario | Status | Trigger |
|---|---|---|
| Malformed JSON | `400 Bad Request` | `Json::<T>::from_request` parse failure |
| Serialisation failure | `500 Internal Server Error` | `Json<T>::into_response` `to_vec` error (rare) |

:::note[Choosing between the two approaches]
Use `Json<T>` with `serde` for any production code where the JSON schema is known at compile time. Use the built-in `JSON` parser only when you need to handle arbitrary or schema-less JSON, or when you cannot add the `serde` dependency.
:::
