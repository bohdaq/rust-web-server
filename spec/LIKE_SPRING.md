# Making rws Feel Like Spring Boot

This document tracks what is missing to make `rws` usable as a full application framework, comparable to Spring Boot in developer experience.

---

## Must-have for a framework feel

### 1. Declarative routing via macros ✅ Done

`#[route(GET, "/path")]`, `#[get("/path")]`, `#[post("/path")]`, etc. (proc-macro, `macros` feature). The `routes!` declarative macro builds an `AppWithState` or `Router` from a method-path-handler table. `AppWithState<S>` has `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()` fluent builder methods.

### 2. Automatic JSON binding ✅ Done

`Json<T>` typed extractor and responder backed by `serde_json` (`serde` feature). `Json::from_request` deserializes the body; `Json::into_response` serializes and sets `Content-Type: application/json`.

### 3. Path and query parameter extraction ✅ Done

`Router` with `:name` path parameters and `*wildcard` trailing segments. `PathParams::get(name)` retrieves extracted values. `Query` extractor parses the query string into `HashMap<String, String>`. `#[derive(FromRequest)]` derives extraction for named-field structs.

### 4. Middleware / filter chain ✅ Done

`Middleware` trait with `handle(request, connection, next)` — composable before/after hooks. `App::new().wrap(layer)` or `AppWithState::wrap(layer)` pushes layers onto a stack. Built-in layers: `RateLimitLayer`, `MetricsLayer`, `CacheLayer`, `OtelLayer`, `RewriteLayer`, `ReverseProxy`, `H2ReverseProxy`, `GrpcProxy`, `CanaryLayer`, `RetryLayer`, `BasicAuthLayer`, `JwtLayer`, `IpFilter`, `BlocklistLayer`, `MaintenanceLayer`, `LogLayer`.

### 5. Centralized error handling ✅ Done

`IntoResponse` trait — implement on your error enum to map it to a `Response`. `AppError` enum covers `BadRequest`, `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `UnprocessableEntity`, `TooManyRequests`, `Internal`; all implement `IntoResponse`. Return `Result<Response, impl IntoResponse>` from any handler.

---

## Important

### 6. Session management ✅ Done

`SessionStore` — thread-safe in-memory session store with TTL expiry. `Session` is the per-request value type. Cookie helpers: `session_id_from_request`, `session_cookie`, `destroy_cookie`.

### 7. Request validation ✅ Done

`Validate` trait + `ValidationErrors` collector. `Validated<T>` wrapper: extract then validate in one step — `400` on extraction failure, `422 Unprocessable Entity` with JSON error body on validation failure. `#[derive(Validate)]` with `#[validate(length(min, max))]`, `range`, `email`, `required`, `url` annotations (`macros` feature).

### 8. Typed configuration binding ✅ Done (v17.35.0)

`#[derive(Config)]` (`macros` feature) generates `fn load() -> Result<Self, String>` that reads environment variables and parses them into strongly-typed struct fields. Equivalent to Spring's `@ConfigurationProperties`.

```rust
use rust_web_server::Config;

#[derive(Config)]
#[config(prefix = "APP_")]
struct AppConfig {
    #[config(env = "PORT", default = "8080")]
    port: u16,

    #[config(env = "DATABASE_URL")]
    database_url: String,      // required — Err if absent

    #[config(env = "DEBUG")]
    debug: Option<bool>,       // None if absent or empty
}

let cfg = AppConfig::load().expect("invalid config");
```

Runtime support in `src/config_binding/` — `FromEnvStr` trait with impls for all Rust scalar types (`String`, `bool`, `u8`–`u128`, `i8`–`i128`, `f32`, `f64`, `usize`, `isize`). Implement `FromEnvStr` for custom types. Three helper functions (`load_required`, `load_with_default`, `load_optional`) are also available directly.

Field derivation rules:

| Annotation | Env var absent | Env var present |
|---|---|---|
| `#[config(env = "KEY", default = "v")]` | use `"v"` | parse to field type |
| `#[config(env = "KEY")]` non-`Option` | `Err` | parse to field type |
| `#[config(env = "KEY")]` `Option<T>` | `Ok(None)` | parse, wrap in `Some` |
| no `#[config]` — auto-key from field name | same as non-`Option` rules | — |

---

## Nice to have

### 9. HTML template engine ✅ Done (v17.34.0)

`TeraEngine` and module-level helpers in `src/template/` (`features = ["tera"]`). Jinja2/Django syntax via the [Tera](https://keats.github.io/tera/) crate — variables, loops, conditionals, template inheritance, filters, macros.

```rust
use rust_web_server::template::{self, Context};

// At startup:
template::init("templates").unwrap();

// In a handler:
let mut ctx = Context::new();
ctx.insert("title", "Home");
ctx.insert("items", &["Rust", "rws", "Tera"]);
let response = template::render("index.html", &ctx)?;
```

Three construction modes: `TeraEngine::from_dir(dir)` for disk templates, `TeraEngine::from_glob(pattern)` for custom glob, `TeraEngine::from_raw(&[(name, src)])` for inline/test templates. Global singleton via `template::init(dir)` / `template::init_from_env()` / `template::global()`. Template directory configured via `RWS_CONFIG_TEMPLATE_DIR` (default: `"templates"`).

### 10. WebSocket support ✅ Done (v17.8.0)

`WebSocket` and `Frame` types in `src/websocket/`. RFC 6455 handshake, frame encode/decode, SHA-1 + base64 built in, no extra dependency. `WsProxy` provides standalone WebSocket proxying.

### 11. Database layer

No connection pool or query builder. `sqlx` would be the natural fit — async, compile-time checked queries, supports PostgreSQL / MySQL / SQLite.

### 12. Scheduler ✅ Done (v17.33.0)

`Scheduler` (`src/scheduler/`) — `@Scheduled`-equivalent background task runner. Three scheduling modes:
- `.every(Duration, fn)` — fixed rate (interval measured from task start)
- `.after(Duration, fn)` — fixed delay (interval measured from task end)
- `.cron("sec min hour day month weekday", fn)` — 6-field cron expression

Full cron field syntax: `*`, exact value, `*/step`, `N-M` range, `N,M,P` list, and combinations. `.initial_delay(Duration)` delays the first run of the most recently registered task. `.start()` spawns one background thread per task and returns immediately.

### 13. Test utilities ✅ Done

`TestClient<A>` — dispatches requests directly through an `Application` without opening a TCP socket. Builder API: `.get(path)`, `.post(path)`, `.with_header(name, value)`, `.with_body(bytes)`, `.send()` → `Response`. Used in unit and integration tests throughout the codebase.

---

## What Rust makes hard

**Dependency injection** — Spring's IoC container relies on reflection, which Rust does not have. The rws approach is a `Container` (`src/di/`) that stores services keyed by `TypeId`. It covers the most common cases: concrete types, trait objects, and named instances. Use `container.into_arc()` with `App::with_state` to share services across handlers. See [Use Case #53 in DEVELOPER.md](../DEVELOPER.md) for full examples.

```rust
use std::sync::Arc;
use rust_web_server::di::Container;

// Concrete service
let mut c = Container::new();
c.register(Config { port: 8080 });
let cfg = c.get::<Config>().unwrap();

// Trait object — swap implementation without changing handlers
c.provide::<dyn UserRepository>(Arc::new(PgUserRepository::new()));
let repo = c.get::<dyn UserRepository>().unwrap();

// Use as AppWithState
let app = routes! { App::with_state(c.into_arc()), GET "/v" => handler, };
```

---

## Suggested implementation order

| Priority | Item | Status |
|---|---|---|
| 1 | Proc-macro routing (`#[route]`) | ✅ Done |
| 2 | `Json<T>` extractor + response | ✅ Done |
| 3 | Path / query parameter extraction | ✅ Done |
| 4 | Middleware chain | ✅ Done |
| 5 | Centralized error handler (`IntoResponse`) | ✅ Done |
| 6 | Session management | ✅ Done |
| 7 | Request validation (`#[derive(Validate)]`) | ✅ Done |
| 8 | WebSocket support | ✅ Done |
| 9 | Test utilities (`TestClient`) | ✅ Done |
| 10 | Scheduler (`@Scheduled` equivalent) | ✅ Done (v17.33.0) |
| 11 | Typed configuration binding (`#[derive(Config)]`) | ✅ Done (v17.35.0) |
| 12 | HTML template engine | ✅ Done (v17.34.0) |
| 13 | Database layer (`sqlx`) | ❌ Not yet |
