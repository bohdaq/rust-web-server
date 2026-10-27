---
title: OpenAPI / Swagger
description: Generate an OpenAPI 3.1 document, with request and response schemas from your serde types, and a browsable Swagger UI.
---

Requires the `openapi` feature:

```toml
[dependencies]
rust-web-server = { version = "18", features = ["openapi"] }
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

- **`GET /openapi.json`** — the generated OpenAPI 3.1 document (`Content-Type: application/json`)
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

## Request and response schemas

Describe an operation with an `Operation` and the generated document gets its request body, responses and query parameters, with schemas from your types. They come from `JsonSchema` — `#[derive(JsonSchema)]` (`macros` feature), the derivation [MCP tools](/mcp/agent-tools/) use — so the schema follows `#[serde(rename_all)]` and friends and describes the JSON your handlers really read and write. Rust has no runtime reflection, so the types are named where the operation is described rather than read off the handler:

```rust
use rust_web_server::app::App;
use rust_web_server::openapi::{OpenApiConfig, Operation};
use rust_web_server::JsonSchema;
use serde::{Deserialize, Serialize};

/// A stocked item.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Item { sku: String, on_hand: u32 }

/// The body of a stock change.
#[derive(Deserialize, JsonSchema)]
struct StockDelta { delta: i64 }

#[derive(Serialize, JsonSchema)]
struct ApiError { error: String }

let app = App::with_state(inventory)
    .get("/items/:sku", get_item)
    .post("/items/:sku/adjust", adjust)
    .openapi(
        OpenApiConfig::new("Inventory", "1.0.0")
            .operation("GET", "/items/:sku", Operation::new("Look up one item")
                .response::<Item>(200, "The item")
                .response::<ApiError>(404, "No item with that SKU"))
            .operation("POST", "/items/:sku/adjust", Operation::new("Add or remove stock")
                .operation_id("adjustStock")
                .tag("items")
                .body::<StockDelta>()
                .response::<Item>(200, "The item after the change")
                .status(409, "Stock would go below zero")),
    );
```

| `Operation` method | In the document |
|---|---|
| `new(summary)`, `.description(text)`, `.operation_id(id)`, `.tag(name)`, `.deprecated()` | the operation's summary, description, `operationId`, `tags`, `deprecated` |
| `.body::<T>()` | a required `application/json` `requestBody` with `T`'s schema; `.body_schema(content_type, schema, required)` for anything else |
| `.response::<T>(status, description)` | a response with an `application/json` body of `T`; `.response_schema(status, description, content_type, schema)` for other types |
| `.status(status, description)` | a response without a body (an error, `204`) |
| `.query::<T>()` | one `in: query` parameter per property of struct `T`, required as `T` requires it; `.query_param(name, schema, required)` for one |
| `.path_param::<T>(name)` | the path parameter's schema (default: a string); `.path_param_schema(name, schema, description)` |

Operations are keyed by method and the route pattern as registered (`/items/:sku`, not `/items/{sku}`); describing one twice keeps the last. A route with no `Operation` still appears, with a generic `200 OK`. An `Operation` for a pattern no route has is documented anyway — for routes served outside the router. Schemas are inline (no `components`); `JsonSchema` emits JSON Schema 2020-12, which is why the document is OpenAPI 3.1 (`Option<T>` becomes `anyOf` with `null`, which 3.0 can't express).

[`examples/mcp_inventory.rs`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_inventory.rs) describes its REST API this way; CI checks its document with an OpenAPI 3.1 validator (`ci/e2e/openapi.sh`, `openapi-spec-validator`).

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
