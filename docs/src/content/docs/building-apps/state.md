---
title: Shared State
description: Pass databases, caches, and config to handlers using App::with_state and Arc-wrapped types.
---

## App::with_state

`App::with_state(S)` creates an `AppWithState<S>` that stores your state behind an `Arc<S>` and shares it across all handlers and threads. The bound is `S: Send + Sync + 'static`.

```rust
use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

struct Config {
    version: &'static str,
}

let app = App::with_state(Config { version: "1.0.0" })
    .get("/version", |_req, _params, _conn, cfg| {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![
            Range::get_content_range(
                cfg.version.as_bytes().to_vec(),
                MimeType::TEXT_PLAIN.to_string(),
            )
        ];
        r
    });
```

## Handler signature

```rust
fn handler(
    request: &Request,
    params: &PathParams,
    connection: &ConnectionInfo,
    state: &S,
) -> Response
```

Handlers receive an immutable `&S` reference. The state type `S` itself is never cloned per-request — only the `Arc<S>` wrapping it is cloned once at registration time.

## routes! macro with state

The `routes!` macro works with any builder that has the `.get()` / `.post()` etc. methods, including `AppWithState`:

```rust
use rust_web_server::app::App;
use rust_web_server::routes;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::core::New;

struct AppState {
    db_url: String,
}

fn get_items(_req: &Request, _p: &PathParams, _c: &ConnectionInfo, s: &AppState) -> Response {
    let _ = &s.db_url; // use db_url to open a connection
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

let app = routes! {
    App::with_state(AppState { db_url: "postgres://localhost/mydb".to_string() }),
    GET "/items" => get_items,
};
```

## Mutable shared data

Because handlers receive `&S` (not `&mut S`), you need interior mutability for data that changes after startup. Two common patterns:

### Arc<Mutex<T>> — exclusive write access

```rust
use std::sync::{Arc, Mutex};
use rust_web_server::app::App;

struct Counter {
    hits: Arc<Mutex<u64>>,
}

let app = App::with_state(Counter { hits: Arc::new(Mutex::new(0)) })
    .get("/count", |_req, _params, _conn, state| {
        let mut hits = state.hits.lock().unwrap();
        *hits += 1;
        // build response with *hits ...
        todo!()
    });
```

### Arc<RwLock<T>> — concurrent reads, exclusive writes

Prefer `RwLock` when reads vastly outnumber writes (e.g., a shared cache or config snapshot):

```rust
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use rust_web_server::app::App;

struct Cache {
    store: Arc<RwLock<HashMap<String, String>>>,
}

let app = App::with_state(Cache { store: Arc::new(RwLock::new(HashMap::new())) })
    .get("/cache/:key", |_req, params, _conn, state| {
        let key = params.get("key").unwrap_or("");
        let guard = state.store.read().unwrap();
        let value = guard.get(key).cloned().unwrap_or_default();
        // build response with value ...
        todo!()
    });
```

## Accessing state from within state

If you store `Arc`-wrapped sub-components inside `S`, you can clone the inner `Arc` and spawn background tasks without coupling the lifetime to the request:

```rust
use std::sync::{Arc, Mutex};

struct JobQueue(Arc<Mutex<Vec<String>>>);

// Inside a handler:
// let queue = Arc::clone(&state.0);
// std::thread::spawn(move || { queue.lock().unwrap().push(job); });
```

## AsyncAppWithState — async handlers

When compiled with the `http2` feature, `App::with_async_state(S)` returns an `AsyncAppWithState<S>` whose handlers are `async fn`:

```rust
use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::core::New;

struct Db { url: String }

let app = AsyncAppWithState::new(Db { url: "postgres://localhost/mydb".to_string() })
    .get("/users/:id", |_req, params, _conn, state| async move {
        let id = params.get("id").unwrap_or("unknown");
        // await a real async DB call here
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r
    });
```

:::note[Feature requirement]
`AsyncAppWithState` requires `--features http2` (or the default `http3` feature). It is not available in the `http1`-only build because that build has no tokio runtime.
:::

The async bridge automatically detects whether it is running inside an existing tokio runtime (HTTP/2 / HTTP/3 path) or in the synchronous thread-pool (HTTP/1.1 path) and adapts accordingly.

## Unmatched requests

Routes are tried in registration order. Any request that does not match a registered route falls through to the built-in `App` controller chain (static files, `/healthz`, `/readyz`, `/metrics`, `404 Not Found`).
