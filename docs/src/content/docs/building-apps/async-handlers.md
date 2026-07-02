---
title: Async Handlers
description: Write async route handlers using AsyncAppWithState for non-blocking I/O with databases, HTTP clients, and other async libraries.
---

`AsyncAppWithState<S>` is the async counterpart to `AppWithState<S>`. Its handlers are `async fn` closures that can `await` database queries, outbound HTTP calls, timers, or any other async I/O.

:::caution[Feature requirement]
`AsyncAppWithState` requires the `http2` feature (or the default `http3` feature). It is **not** available in `--no-default-features --features http1` builds.

```toml
[dependencies]
rust-web-server = { version = "17" }  # default features include http2 + http3
```
:::

## Creating an `AsyncAppWithState`

Call `AsyncAppWithState::new(state)` with your shared state value, then chain `.get()`, `.post()`, etc. to register handlers.

```rust
use std::sync::Arc;
use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::router::PathParams;
use rust_web_server::request::Request;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::core::New;

struct AppState {
    db_url: String,
}

let app = AsyncAppWithState::new(AppState { db_url: "postgres://…".to_string() })
    .get("/greet/:name", |_req, params, _conn, state| async move {
        let name = params.get("name").unwrap_or("world");
        let body = format!("Hello, {}! DB: {}", name, state.db_url);
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![
            Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())
        ];
        r
    });
```

## Handler signature

Async handlers take four owned parameters so the returned future is `'static`:

```rust
async fn handler(
    req:    Request,           // owned — not a reference
    params: PathParams,
    conn:   ConnectionInfo,
    state:  Arc<S>,
) -> Response
```

The state is passed as `Arc<S>` (a cheap clone), keeping all handlers behind shared ownership.

## Named handler functions

You can name your handlers instead of using inline closures. This is easier to test and keeps long business logic out of the router call-site:

```rust
use std::sync::Arc;
use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

struct Db {
    url: String,
}

async fn get_user(
    _req: Request,
    params: PathParams,
    _conn: ConnectionInfo,
    state: Arc<Db>,
) -> Response {
    let user_id = params.get("id").unwrap_or("0");

    // Await an async database call here
    // let user = sqlx::query!(...).fetch_one(&pool).await?;

    let body = format!("{{\"id\":{}}}", user_id);
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

let app = AsyncAppWithState::new(Db { url: "sqlite::memory:".to_string() })
    .get("/users/:id", get_user);
```

## Supported HTTP methods

| Builder method | HTTP method |
|---|---|
| `.get(pattern, handler)` | GET |
| `.post(pattern, handler)` | POST |
| `.put(pattern, handler)` | PUT |
| `.patch(pattern, handler)` | PATCH |
| `.delete(pattern, handler)` | DELETE |

Path pattern syntax is identical to the synchronous router: `:name` matches a single segment, `*name` matches the rest of the path.

## How the runtime bridge works

`AsyncAppWithState` implements the synchronous `Application` trait so it fits into `Server::run_tls()` / `Server::run_quic()` alongside sync middleware. When `execute` is called:

- **Inside an existing tokio runtime** (HTTP/2 / HTTP/3 handlers): the async future runs in a scoped OS thread with its own single-threaded runtime to avoid blocking the event loop.
- **Outside any runtime** (HTTP/1.1 thread-pool): a temporary single-threaded tokio runtime is created, the future is driven to completion, and the runtime is dropped.

In both cases, the call site sees a synchronous `Result<Response, String>`. You do not need to think about this bridge when writing handlers.

## Unmatched routes fall through

Routes that do not match any registered pattern are forwarded to the built-in `App` controller chain (static files, health checks, metrics, etc.). You only need to register handlers for your application routes.

## When to use async handlers

Use `AsyncAppWithState` when your handlers need to:

- Query an async database driver (e.g. `sqlx`, `tokio-postgres`)
- Make outbound HTTP calls via `reqwest` or `AsyncClient`
- Wait on async channels or timers
- Call any `async fn` from a third-party library

For handlers that only do synchronous work (reading headers, JSON parsing, string formatting), the synchronous `App::with_state` is sufficient and avoids the overhead of the runtime bridge.

## Full example — async HTTP call

```rust
use std::sync::Arc;
use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::http_client::AsyncClient;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

struct Config {
    upstream_url: String,
}

async fn proxy_data(
    _req: Request,
    _params: PathParams,
    _conn: ConnectionInfo,
    state: Arc<Config>,
) -> Response {
    let result = AsyncClient::new()
        .get(&state.upstream_url)
        .send()
        .await;

    match result {
        Ok(upstream_resp) => {
            let body = upstream_resp.bytes().to_vec();
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            r.content_range_list = vec![
                Range::get_content_range(body, MimeType::APPLICATION_JSON.to_string())
            ];
            r
        }
        Err(_) => {
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
            r
        }
    }
}

let app = AsyncAppWithState::new(Config {
    upstream_url: "https://api.example.com/data".to_string(),
})
.get("/data", proxy_data);
```
