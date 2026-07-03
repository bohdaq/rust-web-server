---
title: Per-Route Timeouts
description: Give individual routes their own deadline instead of one global read timeout for everything.
---

A single global read timeout (30s per connection, or `RWS_CONFIG_*`) applies to every route by default. A file-upload endpoint may legitimately need 120s while a health check should fail fast at 500ms. `rust_web_server::timeout` wraps individual handlers with their own budget — no new Cargo feature required, no new dependencies.

## `Router` / stateless handlers

```rust
use rust_web_server::router::Router;
use rust_web_server::timeout::with_timeout;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

let router = Router::new()
    .get("/healthz", with_timeout(Duration::from_millis(500), |_req, _params, _conn| Response::new()))
    .post("/upload", with_timeout(Duration::from_secs(120), |_req, _params, _conn| Response::new()));
```

If the wrapped handler doesn't return within `duration`, the caller gets `504 Gateway Timeout` immediately instead of waiting further.

## `AppWithState<S>`

```rust
use rust_web_server::app::App;
use rust_web_server::timeout::with_timeout_state;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

#[derive(Clone)]
struct Db; // holds e.g. an Arc<DbPool> internally — cheap to clone

let app = App::with_state(Db)
    .get("/healthz", with_timeout_state(Duration::from_millis(500), |_req, _params, _conn, _db| Response::new()))
    .post("/upload", with_timeout_state(Duration::from_secs(120), |_req, _params, _conn, _db| Response::new()));
```

:::note[Why `S: Clone`?]
The handler signature only gives you `&S` (a borrow), but the timeout wrapper runs the call on a background thread, which needs its own owned copy. Most `AppWithState` state types already hold their real data behind `Arc` fields internally and are cheap to `#[derive(Clone)]`. If yours genuinely can't be `Clone`, wrap the whole app with [`TimeoutLayer`](#wrapping-a-whole-application) instead, or switch that route to `AsyncAppWithState` — see below.
:::

## `AsyncAppWithState<S>` (requires `http2`)

```rust
use rust_web_server::app::App;
use rust_web_server::timeout::with_timeout_async;
use rust_web_server::response::Response;
use rust_web_server::core::New;
use std::time::Duration;

struct Db;

let app = App::with_async_state(Db)
    .post("/upload", with_timeout_async(Duration::from_secs(120), |_req, _params, _conn, _db| async {
        Response::new()
    }));
```

No `Clone` bound needed — `AsyncAppWithState` already passes state as an owned `Arc<S>`. This variant is also the only one with **genuine cancellation**: it's backed by `tokio::time::timeout`, and dropping a suspended `Future` actually stops its execution at the next `.await` point.

## Wrapping a whole `Application`

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::timeout::TimeoutLayer;
use std::time::Duration;

let app = TimeoutLayer::new(App::new(), Duration::from_secs(5));
```

Use `TimeoutLayer` when you want one blanket deadline around an entire app rather than different deadlines per route. `TimeoutLayer::from_arc(Arc<dyn Application + ...>, duration)` wraps an already-shared `Arc` — this is how the config-driven proxy applies a route's `timeout_ms` internally.

## Config-driven proxy

Add `timeout_ms` as a flat key under `[route.middleware]` — it bounds that route's *total* time, including its other middleware:

```toml
[[route]]
name = "slow-upload"

[route.match]
path = "/upload"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "backend"

[route.middleware]
timeout_ms = 120000
```

`0` or an absent `timeout_ms` means no timeout — the route behaves as it did before this feature existed.

## The honest limitation

Rust cannot forcibly stop a thread that's already running. `with_timeout`, `with_timeout_state`, and `TimeoutLayer` all run the wrapped work on a background thread and race it against `duration` using a channel:

- If the work finishes first, its result is returned normally.
- If the deadline passes first, the caller gets `504 Gateway Timeout` **immediately** — the calling thread does not wait for the slow work to finish.
- The slow work itself, however, is **not stopped**. It keeps running to completion on its background thread; its eventual result is simply discarded when it tries to send it back.

This means these timeouts bound the **client's wait time**, not the handler's actual CPU/memory/connection usage. For a slow database query or a hung TCP connection to a backend, the underlying resource is still consumed until that call naturally finishes (or its own lower-level timeout — e.g. a DB driver's own query timeout, or `DynamicProxy`'s `connect_timeout_ms`/`read_timeout_ms` — cuts it off first).

`with_timeout_async` is the one exception: because tokio futures are cooperatively scheduled, dropping a `Future` that's suspended at an `.await` point genuinely stops it from making further progress. If you need the underlying work to actually stop at the deadline — not just the client-facing response — use `AsyncAppWithState` and `with_timeout_async`.
