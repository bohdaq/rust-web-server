---
title: Routing
description: Path-based routing with named parameters, wildcards, and virtual-host scoping using Router and the routes! macro.
---

## routes! macro

The `routes!` macro is the recommended way to declare routes for new code. It builds an `AppWithState<S>` (or any builder that exposes `.get()`, `.post()`, etc.) from a declarative table.

```rust
use rust_web_server::app::App;
use rust_web_server::routes;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::core::New;

struct Db;

fn list_users(_req: &Request, _params: &PathParams, _conn: &ConnectionInfo, _db: &Db) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

fn create_user(_req: &Request, _params: &PathParams, _conn: &ConnectionInfo, _db: &Db) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}

let app = routes! {
    App::with_state(Db),
    GET  "/users"     => list_users,
    POST "/users"     => create_user,
};
```

Syntax:

```text
routes! {
    <builder_expression>,
    METHOD "pattern" => handler_or_closure,
    ...
}
```

Valid methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE` (all caps). A trailing comma after the last route is optional.

## Router — direct usage

Use `Router` directly when you need to compose multiple routers or call `handle` inside a custom `Application::execute`.

```rust
use rust_web_server::router::{Router, PathParams};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::core::New;

let router = Router::new()
    .get("/users/:id", |_req, params, _conn| {
        let id = params.get("id").unwrap_or("unknown");
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(
            format!("user {}", id).into_bytes(),
            MimeType::TEXT_PLAIN.to_string(),
        )];
        r
    })
    .delete("/users/:id", |_req, params, _conn| {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n204_no_content.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n204_no_content.reason_phrase.to_string();
        r
    });

// Inside Application::execute:
// if let Some(response) = router.handle(request, connection) {
//     return Ok(response);
// }
```

`router.handle(request, connection)` returns `Some(Response)` on the first match, `None` if no route matches. Routes are tried in registration order.

## Pattern syntax

| Syntax | Description | Example |
|---|---|---|
| `/literal` | Exact path segment | `/users` |
| `/:name` | Named segment — captures one segment | `/users/:id` |
| `/*name` | Wildcard — captures everything after the prefix | `/files/*path` |

Rules:
- A wildcard (`*name`) must be the **last** segment in a pattern.
- The query string is stripped before matching; only the path is used.
- Patterns are matched segment-by-segment; `/users` does not match `/users/42`.

```rust
// /users/42/posts/7 → id="42", post_id="7"
router.get("/users/:id/posts/:post_id", handler);

// /files/a/b/c.txt → path="a/b/c.txt"
router.get("/files/*path", handler);
```

## PathParams extraction

Inside a handler, call `params.get("name")` to retrieve a named segment value. It returns `Option<&str>`.

```rust
.get("/articles/:slug", |_req, params, _conn| {
    let slug = params.get("slug").unwrap_or("unknown");
    // use slug ...
    Response::new()
})
```

## Virtual-host routing

Call `.with_host("example.com")` before registering routes to restrict a router to requests whose SNI hostname (TLS) or `Host` header (plain HTTP) matches that value.

```rust
let api_router = Router::new()
    .with_host("api.example.com")
    .get("/status", api_status_handler);

let www_router = Router::new()
    .with_host("www.example.com")
    .get("/", home_handler);

// In Application::execute:
// api_router.handle(request, connection)
//     .or_else(|| www_router.handle(request, connection))
```

`handle()` returns `None` immediately when `.with_host()` is set and the incoming request's hostname does not match, so composing multiple host-scoped routers is safe and efficient.

## Relationship to `App`'s built-in routes

`App`'s built-in controllers — static file serving, `/healthz`/`/readyz`/`/metrics`, form handling, the 404 fallback — deliberately don't go through `Router`. That set is small, static, and known at compile time, so a fixed if-chain is simpler and just as fast as a segment matcher would be there. `Router` is for **your** routes, with dynamic path params and wildcards a fixed if-chain can't express cleanly.

`AppWithState` and `AsyncAppWithState` already compose the two: they hold a `Router` internally, try it first, and fall through to `App`'s built-in controller chain for anything the router doesn't match — so static files, health checks, and metrics keep working automatically even in a fully custom, stateful app.

:::caution[Coming Soon]
Attribute macros (`#[get("/path")]`, `#[post("/path")]`, etc.) via the `macros` feature flag are planned but not yet implemented. Use the `routes!` macro or builder-style `.get()` / `.post()` calls in the meantime.
:::
