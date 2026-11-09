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
| `/:name` or `/{name}` | Named segment — captures one segment | `/users/:id`, `/users/{id}` |
| `/*name` or `/{*name}` | Wildcard — captures everything after the prefix | `/files/*path`, `/files/{*path}` |
| `/{{text}}` | Literal braces | `/raw/{{id}}` matches `/raw/{id}` |

The braced forms (since 18.7.0) are what axum 0.8 and OpenAPI use; the two syntaxes mean the same thing and can be mixed. A brace parameter takes a whole segment — `/file.{ext}` is a literal, not a parameter.

Rules:
- A wildcard (`*name`, `{*name}`) must be the **last** segment in a pattern.
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

## Typed handlers: `#[get]` with extractors

Since 18.28.0 a handler can take [extractors](/building-apps/extractors/#extractors-as-handler-arguments) as arguments and return anything that implements `IntoResponse`; the route attribute (`macros` feature) generates the adapter and the app takes the method and pattern from it:

```toml
# Cargo.toml
rust-web-server = { version = "18", features = ["macros", "serde"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::error::AppError;
use rust_web_server::handler::{Json, Path, State};
use rust_web_server::{get, post};

struct Db { /* pool, … */ }

#[get("/posts/{id}")]
fn show_post(Path(id): Path<u64>, State(db): State<Db>) -> Result<Json<Post>, AppError> {
    db.find(id).map(Json).ok_or_else(|| AppError::NotFound(format!("no post {id}")))
}

#[post("/posts")]
fn create_post(Json(new): Json<NewPost>, db: &Db) -> Result<Json<Post>, AppError> {
    db.insert(new).map(Json)
}

let app = App::with_state(Db { /* … */ })
    .route(show_post)
    .route(create_post);
```

- `#[get]`, `#[post]`, `#[put]`, `#[patch]`, `#[delete]` and `#[route(METHOD, "/path")]` turn the function into a unit struct of the same name implementing `handler::Handler<S>` (and `AsyncHandler<S>`). `.route(h)` registers it on `AppWithState`, on `Router` (handlers without state arguments) and on `AsyncAppWithState`, where `async fn` handlers go too. The function as written stays callable as `show_post::call(Path(1), State(db))`, e.g. in unit tests.
- The generated `handle` runs the extractors in argument order and calls the function: the same work as a handler that calls them by hand, behind the same route dispatch, with no extra allocation for a classic handler's path.
- The first extractor that fails answers the request with its rejection (`400`, `415`, `422`, … as problem details); the function doesn't run.
- `show_post::METHOD` and `show_post::PATTERN` (trait `handler::RouteSpec`) are the attribute's arguments.

**Checked at compile time** — the route's parameters against the `Path` argument:

| Mistake | Error |
|---|---|
| `#[get("/posts/{id}")] fn f(Path(post_id): Path<u64>)` | `` `post_id` is not a parameter of route `GET /posts/{id}` `` |
| `#[get("/posts/{id}")] fn f(Path((id, slug)): Path<(u64, String)>)` | the tuple takes 2, the route has 1 |
| `#[get("/posts/{id}/{slug}")] fn f(Path(id): Path<u64>)` | a scalar takes one parameter |
| `#[post("/posts/{id}/comments")] fn f(State(db): State<Db>)` | `{id}` isn't taken by any `Path` (or `&PathParams`) argument |
| `#[get("/posts")] fn f(Path(id): Path<u64>)` | the route has no parameters |
| a `#[derive(FromPath)]` struct whose fields aren't exactly the route's parameters | a `const` assertion naming the route |

A name bound inside `Path(…)` names the route parameter it takes (as in Rocket), so a tuple's bindings may come in any order: `Path((slug, id))` for `/posts/{id}/{slug}`. A plain binding (`p: Path<(u64, String)>`) or `_` takes them in route order. `/posts/:id` and `/posts/{id}` are the same route.

**Classic handlers are unchanged.** On a function with the classic signature — `(&Request, &PathParams, &ConnectionInfo)` plus the state for `AppWithState` (or `AsyncAppWithState`'s owned form) — the attribute only adds a `Route: METHOD /path` doc comment, as before 18.28.0, and you register it with `.get(pattern, handler)` or `routes!`.

## Route conflicts

A route registered after one with the same method that matches every path it does never runs: routes are tried in order and the first match wins. `AppWithState`, `AsyncAppWithState` and `Router` report these when the route is added — one line on stderr per distinct conflict — and list them with `route_conflicts()`:

```rust
let app = App::with_state(db)
    .get("/users/:id", show_user)
    .get("/users/me", me)          // shadowed: /users/:id answers /users/me
    .get("/users/{uid}", other);   // duplicate: same pattern as /users/:id

for conflict in app.route_conflicts() {
    eprintln!("{conflict}");
}
// rws: warning: route GET /users/me never runs: GET /users/:id, registered before it, matches every path it does
```

Each `RouteConflict` has the `method`, the `pattern` that never runs, the `earlier` one that answers instead, and a `kind` (`Duplicate`: the same pattern, parameter names aside; `Shadowed`: the earlier one has a parameter or wildcard where it has a literal). It's a warning only — the app is built and serves as before; register the specific route first (`/users/me`, then `/users/:id`). Patterns that only overlap (`/a/:x` and `/:y/b`) aren't reported. A test can assert `app.route_conflicts().is_empty()`.

## Groups and guards

`group(prefix, |g| …)` registers routes under a shared path prefix. They share guards — checks that run before the handler (`before_action`) — and, on `AppWithState`, middleware (18.35.0):

```rust
use rust_web_server::app::App;
use rust_web_server::flash::Redirect;
use rust_web_server::error::IntoResponse;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;

fn signed_in(request: &Request, _: &ConnectionInfo, _: &Db) -> Result<(), Response> {
    match current_user(request) {
        Some(_) => Ok(()),
        None => Err(Redirect::to("/login").alert("Sign in first").into_response()),
    }
}

let app = App::with_state(db)
    .get("/", home)
    .group("/admin", |admin| {
        admin
            .guard(signed_in)
            .wrap(RateLimitLayer)            // only these routes (AppWithState)
            .get("/", dashboard)
            .route(export_csv)               // a typed #[get("/export")] handler: /admin/export
            .group("/billing", |billing| billing.guard(is_owner).get("/", invoices))
    });
```

A guard is `Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response>`; `Err(response)` answers the request. Guards apply to every route of the group, whichever order they were added in, outer groups' first. Groups nest. On `AsyncAppWithState` (`group` takes async handlers) groups have guards but no `wrap`: wrap the whole app instead.

## Resources

`resource(path, |r| …)` registers the conventional routes of a collection, each with a name for [`url_for`](#named-routes-and-url_for):

| Method | Path | Builder | Name |
|---|---|---|---|
| GET | `/posts` | `index` | `posts.index` |
| GET | `/posts/new` | `new_form` | `posts.new` |
| POST | `/posts` | `create` | `posts.create` |
| GET | `/posts/{id}` | `show` | `posts.show` |
| GET | `/posts/{id}/edit` | `edit` | `posts.edit` |
| PUT, PATCH | `/posts/{id}` | `update` | `posts.update` |
| DELETE | `/posts/{id}` | `destroy` | `posts.destroy` |

```rust
let app = App::with_state(db).resource("/posts", |posts| {
    posts
        .index(list_posts)
        .new_form(new_post_form)
        .create(create_post)
        .show(show_post)       // params.get("id")
        .destroy(delete_post)
        .guard(signed_in)
        .extra(|g| g.post("/{id}/publish", publish_post))
});
```

Only the actions you give are registered, and `new` always comes before `{id}`. `.name("articles")` changes the name prefix (default: the path's last segment); `extra` adds routes under the resource's path with its guards. A group has `resource` too, so `/admin/posts` is `admin.resource("/posts", …)`. Handlers have the classic signature (`AsyncAppWithState`: the async one); typed `#[get]` handlers go in a group with `.route(…)`.

## Named routes and `url_for`

`.name("…")` after a route names it; `router::url_for(name, params)` builds its path, percent-encoding the values. Parameters the pattern doesn't have become the query string:

```rust
use rust_web_server::router::url_for;

let app = App::with_state(db).get("/posts/{id}", show_post).name("post");

assert_eq!(Some("/posts/7?tab=comments".to_string()), url_for("post", &[("id", "7"), ("tab", "comments")]));
assert_eq!(None, url_for("post", &[]));                      // a parameter is missing
```

Names are process-wide (registering one again replaces it) and `named_routes()` lists them. Templates call it as `url_for(name="post", id=7)` ([view helpers](/building-apps/templates/#view-helpers)); `Redirect::to_route(name, params)` redirects to one ([flash](/building-apps/forms-uploads/#flash-messages-and-redirects)).

See the [API reference](/reference/api/) for the full list of public types.
