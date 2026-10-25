---
title: Building Apps Overview
description: Three ways to add routes to rust-web-server and how a request flows from the TCP socket to your handler.
---

## Request lifecycle

Every request travels the same path regardless of which routing style you use:

```
main.rs
  └─ Server::setup()        — bind TCP listener
       └─ Server::run_tls()  — accept loop (plain HTTP via run_plain when no certificate);
            │                  TLS handshake; ALPN picks HTTP/2 or HTTP/1.1
            └─ one task per connection — keep-alive loop; Request::parse()
                 └─ app.execute()  — your routing logic returns Result<Response, String>,
                 │                   run on tokio's blocking thread pool (or inline, see below)
                 └─ compression, metrics, Connection header, write response
```

The connection handler calls `app.execute(request, connection)` where `app` is any value that implements the `Application` trait. `execute` is synchronous and may block (read files, call a database or upstream): the server runs it on tokio's blocking thread pool, so a slow handler never stalls other connections. Requests for which `Application::runs_inline` returns `true` (the built-in `App`'s GET/HEAD — health, metrics, static files) skip that handoff and run directly on the event loop; see [Middleware](/building-apps/middleware/#running-on-the-event-loop-runs_inline). (The `http1`-only build has no async runtime; there each connection is served by `Server::process()` on a fixed-size worker thread pool.) The three routing styles differ only in how you implement or compose that `Application`.

`Server::setup()` reads the IP, port and thread count from the layered configuration (defaults → env vars → `rws.config.toml` → CLI args; see [Configuration](/configuration/overview/)) and returns the bound listener plus a thread pool. The async builds then pass your app to `Server::run_tls` (see [Quick Start](/getting-started/quick-start/)); the `http1`-only build uses the blocking `Server::run`:

```rust
use rust_web_server::server::Server;

fn main() {
    let (listener, pool) = match Server::setup() {
        Ok(v) => v,
        Err(e) => { eprintln!("{}", e); return; }
    };
    let app = MyApp::new();          // any type implementing Application + Clone
    Server::run(listener, pool, app);
}
```

## Three ways to add routes

### 1. Controller trait

The lowest-level option. Implement two static methods on a unit struct and register the struct in `App::execute`.

```rust
use rust_web_server::controller::Controller;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::ConnectionInfo;

pub struct PingController;

impl Controller for PingController {
    fn is_matching(request: &Request, _conn: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/ping"
    }

    fn process(_req: &Request, mut response: Response, _conn: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(b"pong".to_vec(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    }
}
```

Controllers are checked in declaration order inside `App::execute`; the first `is_matching` that returns `true` wins.

Use this style when you need precise control over matching logic that path patterns cannot express, or when you are extending the built-in controller chain.

### 2. Router

A fluent, path-based router with named parameters and wildcards. Call `router.handle(request, connection)` from inside any `Application::execute` implementation.

```rust
use rust_web_server::router::{Router, PathParams};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

let router = Router::new()
    .get("/hello", |_req, _params, _conn| {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![
            Range::get_content_range(b"hello".to_vec(), MimeType::TEXT_PLAIN.to_string())
        ];
        r
    })
    .get("/users/:id", |_req, params, _conn| {
        let id = params.get("id").unwrap_or("unknown");
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![
            Range::get_content_range(
                format!("user {}", id).into_bytes(),
                MimeType::TEXT_PLAIN.to_string(),
            )
        ];
        r
    });
```

Use `Router` directly when you need to compose multiple routers (for example, one per virtual host) inside a custom `Application::execute`.

### 3. App::with_state (recommended for new code)

The highest-level option. `App::with_state(S)` creates an `AppWithState<S>` that carries your state in an `Arc<S>` and exposes `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()` builder methods. Handlers receive `&S` directly.

```rust
use rust_web_server::app::App;
use rust_web_server::routes;

struct Db { url: String }

let app = routes! {
    App::with_state(Db { url: "postgres://localhost/mydb".to_string() }),
    GET  "/users"     => list_users,
    POST "/users"     => create_user,
    GET  "/users/:id" => get_user,
};
```

Unmatched requests fall through to the built-in `App` controller chain (static files, `/healthz`, `/readyz`, `/metrics`, 404).

## When to use which approach

| Situation | Recommended style |
|---|---|
| New CRUD endpoints with shared state | `App::with_state` + `routes!` |
| Virtual-host routing or path conditions | `Router::with_host` |
| Extend or override built-in behaviour | `Controller` trait |
| Add cross-cutting concerns | Middleware via `.wrap(layer)` |

## Application variants at a glance

- `App` — zero-config built-in app; `App::new()`.
- `AppWithState<S>` — state-aware sync router; `App::with_state(S)`.
- `AsyncAppWithState<S>` — same but handlers are `async fn`; requires the `http2` feature.
- `WithMiddleware<A>` — wraps any `Application` with a middleware stack; `app.wrap(layer)`.
- `McpServer` — serves the MCP Streamable HTTP protocol; `app.mcp(name, version)`.
