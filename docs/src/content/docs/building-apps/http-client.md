---
title: HTTP Client
description: Call external APIs from handlers with the built-in synchronous Client and the async AsyncClient.
---

`rust_web_server::http_client` is a small outbound HTTP/1.1 client with no third-party HTTP dependency. It follows redirects (up to 10 by default; 301/302/303 become `GET`, 307/308 keep the method) and decodes chunked bodies.

| Build | What works |
|---|---|
| any build | `Client` over plain `http://` |
| `http-client` feature, or `http2`/`http3` (the default) | `Client` over `https://` (rustls + WebPKI roots) |
| `http2`/`http3` | `AsyncClient` (async `send()`) |

```toml
[dependencies]
# Only needed for HTTPS in an http1-only build; the default features already include it.
rust-web-server = { version = "17", default-features = false, features = ["http1", "http-client"] }
```

:::caution
In a build without `http-client`, `http2` or `http3`, an `https://` URL is not encrypted — the request cannot succeed. Enable one of those features before calling HTTPS endpoints.
:::

## Calling an API from a handler

```rust
use rust_web_server::http_client::{Client, HttpClientError};

fn fetch_user(id: u64) -> Result<String, HttpClientError> {
    let client = Client::new();
    let resp = client
        .get(&format!("https://api.example.com/users/{id}"))
        .header("Authorization", "Bearer tok_…")
        .timeout_ms(5_000)
        .send()?;

    if resp.is_success() {
        resp.text()
    } else {
        Err(HttpClientError(format!("upstream returned {}", resp.status())))
    }
}
```

`Client::new()` uses a 30-second timeout and follows up to 10 redirects; change them with `Client::new().timeout_ms(ms)` and `.max_redirects(n)`, or per request with `RequestBuilder::timeout_ms`. `send()` blocks the calling thread — in the async servers a synchronous handler already runs on the blocking pool, so that is fine; inside an `async` handler use `AsyncClient` instead.

## Request bodies

```rust
use rust_web_server::http_client::Client;

// JSON — sets Content-Type: application/json
let resp = Client::new()
    .post("https://api.example.com/charges")
    .body_json(r#"{"amount":1000,"currency":"usd"}"#)
    .header("Authorization", "Bearer sk_…")
    .send()?;
assert!(resp.is_success());

// Form — percent-encodes each pair, sets Content-Type: application/x-www-form-urlencoded
let resp = Client::new()
    .post("https://oauth2.googleapis.com/token")
    .form(&[
        ("grant_type", "authorization_code"),
        ("code", "abc123"),
        ("client_id", "my-client-id"),
        ("client_secret", "my-client-secret"),
    ])
    .send()?;
let json = resp.text()?;
```

`.body_text(s)` sends `text/plain`; `.body(bytes)` sends raw bytes with whatever `Content-Type` you set via `.header`. `.form()` is the body shape OAuth 2.0 token endpoints require — [SSO](/features/sso/) uses it for the code exchange.

## Reading the response

| Method | Returns |
|---|---|
| `.status()` | `u16` |
| `.is_success()` | `true` for 200–299 |
| `.is_redirect()` | `true` for 301/302/303/307/308 (returned once `max_redirects` is reached — `max_redirects(0)` disables following) |
| `.header(name)` | `Option<&str>`, case-insensitive |
| `.headers()` | `&[(String, String)]` |
| `.bytes()` | `&[u8]` |
| `.text()` | `Result<String, HttpClientError>` (UTF-8) |
| `.json::<T>()` | `Result<T, HttpClientError>` — requires the `serde` feature |

`HttpClientError(pub String)` implements `std::error::Error` and `Display`.

## Async client

With the `http2` feature (included in the default build), `AsyncClient` has the same builder API — `get`/`post`/`put`/`patch`/`delete`/`request`, `.header`, `.body*`, `.form`, `.timeout_ms` — with an `async fn send()`:

```rust
use rust_web_server::http_client::{AsyncClient, HttpClientError};

async fn fetch_async(url: &str) -> Result<String, HttpClientError> {
    AsyncClient::new().get(url).send().await?.text()
}
```

See [Async Handlers](/building-apps/async-handlers/#full-example--async-http-call) for a complete handler that forwards an upstream response.
