# Making rws Feel Like Spring Boot

This document tracks what is missing to make `rws` usable as a full application framework, comparable to Spring Boot in developer experience.

---

## Must-have for a framework feel

### 1. Declarative routing via macros

Currently every route requires manual `is_matching`/`process` wiring in `App::execute`. A proc-macro like `#[route(GET, "/users/:id")]` that auto-registers controllers would eliminate the boilerplate entirely.

### 2. Automatic JSON binding

No serde integration exists. Every controller manually parses request bodies and serializes responses. A generic `Json<T>` extractor and return type (backed by `serde_json`) is table-stakes for a modern framework.

### 3. Path and query parameter extraction

No `:id` path variables, no typed query param structs. You currently parse `request_uri` manually. This is one of the most repetitive pain points across controllers.

### 4. Middleware / filter chain

No composable before/after hooks. Spring's `HandlerInterceptor` / filter chain enables auth, rate limiting, request tracing, etc. without touching controllers. Today you would have to copy logic across every controller.

### 5. Centralized error handling

No equivalent of `@ControllerAdvice` / `@ExceptionHandler`. Errors bubble up as raw `Err(String)`. A typed error type and a global handler that maps errors to HTTP responses would clean up every controller.

---

## Important

### 6. Session management

Cookies exist but there is no server-side session store (in-memory or Redis-backed). Spring Session is a core building block for stateful apps.

### 7. Request validation

No equivalent of Bean Validation (`@NotNull`, `@Size`, etc.). Validated field extraction with automatic 400 responses is expected in any framework.

### 8. Typed configuration binding

Config is read via `env::var` strings scattered through the code. A `#[derive(Config)]` macro that binds env vars / TOML keys to a typed struct (like Spring's `@ConfigurationProperties`) makes configuration safe and discoverable.

---

## Nice to have

### 9. HTML template engine

Tera or Minijinja integration for server-side rendering.

### 10. WebSocket support

No upgrade path from HTTP/1.1 today.

### 11. Database layer

No connection pool or query builder. `sqlx` would be the natural fit — async, compile-time checked queries, supports PostgreSQL / MySQL / SQLite.

### 12. Scheduler

No `@Scheduled` equivalent for background tasks (cron-style or fixed-rate).

### 13. Test utilities

No `MockMvc`-style in-process HTTP test client. Currently tests call controller functions directly, bypassing the full request lifecycle.

---

## What Rust makes hard

**Dependency injection** — Spring's IoC container relies on reflection, which Rust does not have. The closest Rust approaches are trait objects + `Arc<dyn Trait>` passed explicitly, or compile-time DI via macros (`shaku`, `inject`). It is doable but looks nothing like Spring's `@Autowired`.

---

## Suggested implementation order

| Priority | Item | Why first |
|---|---|---|
| 1 | Proc-macro routing (`#[route]`) | Biggest DX win; enables everything else |
| 2 | `Json<T>` extractor + response | Eliminates manual serde in every controller |
| 3 | Path / query parameter extraction | Removes the most repetitive controller boilerplate |
| 4 | Middleware chain | Enables auth, logging, rate limiting without code duplication |
| 5 | Centralized error handler | Makes error handling consistent and DRY |
| 6 | Typed config binding | Eliminates scattered `env::var` calls |

Items 1–3 alone would make `rws` feel like a real framework to most users.
