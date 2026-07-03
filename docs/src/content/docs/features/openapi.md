---
title: OpenAPI / Swagger
description: Generate an OpenAPI 3.0 spec and browsable Swagger UI directly from your registered routes.
---

Requires the `openapi` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["openapi"] }
```

No API consumers reading source code or hand-maintained spec files that drift out of sync — the spec is built from the same route registrations your app already has.

## Quick start

```rust
use rust_web_server::app::App;
use rust_web_server::openapi::OpenApiConfig;
use rust_web_server::response::Response;
use rust_web_server::core::New;

struct Db;

let app = App::with_state(Db)
    .get("/users", |_req, _params, _conn, _db| Response::new())
    .get("/users/:id", |_req, _params, _conn, _db| Response::new())
    .post("/users", |_req, _params, _conn, _db| Response::new())
    // Call .openapi() last — it snapshots the routes registered so far.
    .openapi(OpenApiConfig::new("My API", "1.0.0").description("Example API"));
```

This adds two routes on top of whatever you already registered:

- **`GET /openapi.json`** — the generated OpenAPI 3.0.3 document (`Content-Type: application/json`)
- **`GET /docs`** — Swagger UI, loaded from the `unpkg.com/swagger-ui-dist` CDN, pointed at `/openapi.json`

`AsyncAppWithState::openapi(config)` (requires `http2`) works identically for apps with `async fn` handlers.

:::caution[Call `.openapi()` last]
The spec is built once, at the point `.openapi()` is called, from whatever routes exist at that moment. Routes registered *after* `.openapi()` still work normally — they're just not in the generated spec.
:::

## What's in the generated spec

`OpenApiConfig::new(title, version)` sets `info.title` and `info.version`; `.description(...)` is optional. Path parameters are converted automatically:

| Router pattern | OpenAPI path | `parameters` entry |
|---|---|---|
| `/users` | `/users` | none |
| `/users/:id` | `/users/{id}` | `{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }` |
| `/files/*path` | `/files/{path}` | same shape, best-effort (OpenAPI has no native "rest of path" wildcard) |

Multiple HTTP methods on the same path are merged into a single `paths` entry, matching the OpenAPI structure — e.g. `GET /users` and `POST /users` both appear under `"paths": { "/users": { "get": {...}, "post": {...} } }`.

## Scope: paths and methods, not request/response bodies

Every generated operation has a generic `200 OK` response and no request or response body schema:

```json
{
  "get": {
    "summary": "GET /users/{id}",
    "responses": { "200": { "description": "OK" } },
    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }]
  }
}
```

This is a deliberate scope boundary, not an oversight: Rust has no runtime type reflection, so producing a JSON Schema for a handler's request body type or a `#[derive(Validate)]` struct would require deriving schema metadata at the macro level — a substantially larger, separate feature. If you need full body schemas today:

- Post-process the generated JSON string to inject `requestBody`/response schemas for specific operations, or
- Build the spec yourself with [`build_spec`](#using-build_spec-directly) from a hand-written `Vec<RouteInfo>`, or
- Maintain a small hand-written OpenAPI fragment for the handful of routes where body shape matters most, and merge it with the generated output.

## Using `build_spec` directly

The lower-level `build_spec` function takes a route list and a config, and returns the spec as a `String` — useful for inspecting the output, writing it to a file at build time, or merging it with hand-written schema fragments:

```rust
use rust_web_server::openapi::{build_spec, OpenApiConfig};
use rust_web_server::router::RouteInfo;

let routes = vec![
    RouteInfo { method: "GET".to_string(), pattern: "/users/:id".to_string() },
];
let spec_json = build_spec(&OpenApiConfig::new("My API", "1.0.0"), &routes);
```

`AppWithState::route_entries()` / `AsyncAppWithState::route_entries()` / `Router::route_entries()` all return the `Vec<RouteInfo>` that `.openapi()` uses internally, if you want the same route snapshot for something else.
