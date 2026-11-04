---
name: rws
description: Build web apps, JSON APIs, MCP servers, reverse proxies and static sites with rust-web-server (the `rust-web-server` crate / `rws` binary). Use when a project depends on rust-web-server, has an rws.config.toml, or the user asks for rws.
---

# rust-web-server (rws)

rws is a web server (one binary: static files, automatic HTTPS, reverse proxy,
`rws.config.toml`) and a Rust crate for building apps on the same engine. It
does **not** use hyper, Tower or axum: its own `App`, `Request`, `Response`,
`Middleware`. Don't write axum code for it.

Docs for the release a project uses: `https://rws8.tech/<major.minor>/llms-full.txt`
(e.g. `/18.7/`), the newest at `https://rws8.tech/llms-full.txt`; pages at
https://rws8.tech. Check the version in `Cargo.toml` before relying on an API.

## Start a project

Copy a starter (each builds and passes its tests, and has its own `AGENTS.md`):

```sh
cargo generate --git https://github.com/bohdaq/rust-web-server starters/api   # JSON API + OpenAPI
cargo generate --git https://github.com/bohdaq/rust-web-server starters/mcp   # MCP server
cargo generate --git https://github.com/bohdaq/rust-web-server starters/fullstack   # lit frontend + API + PostgreSQL, deploys to kube-dc
```

Or by hand: `rust-web-server = { version = "18", features = [...] }` and
`tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`.
Features you'll want: `serde` (JSON), `macros` (`#[derive(JsonSchema)]`,
`#[derive(Model)]`), `openapi`, `auth`, `sso`, `model-sqlite`/`-postgres`/`-mysql`,
`tower` (tonic, tower-http).

## Project layout

The conventional layout (every starter uses it; `rws.app.toml` at the root
names the app and any path that differs — absent means conventional; the
dev MCP `layout` tool reports it). Put new code where it says:

| Path | Holds |
|---|---|
| `src/main.rs` | wiring only: `Server::setup`, `app().dev_mcp_with(dev_tools())`, `Server::run_tls` |
| `src/lib.rs` | `pub fn app()` (routes + middleware), the state type, `dev_tools()` |
| `src/routes/<resource>.rs` | typed handlers + `pub fn register(app) -> app` |
| `src/models/<table>.rs` | `#[derive(Model)]` types |
| `src/jobs/`, `templates/`, `static/` | jobs, Tera templates, static files |
| `migrations/` | `NNNN_name.sql` + `NNNN_name.down.sql` |
| `tests/` | integration tests: `TestClient::new(my_app::app())` |

## The shape of an app

```rust
// src/routes/items.rs
use rust_web_server::error::AppError;
use rust_web_server::get;
use rust_web_server::handler::{Json, Path};
use rust_web_server::state::AppWithState;

use crate::Store;

#[get("/items/{id}")] // {id} is checked against Path(id) at compile time
pub fn show(Path(id): Path<u64>, store: &Store) -> Result<Json<String>, AppError> {
    let items = store.items.lock().unwrap();
    items.get(&id).cloned().map(Json).ok_or_else(|| AppError::NotFound(format!("no item {id}")))
}

pub fn register(app: AppWithState<Store>) -> AppWithState<Store> {
    app.route(show)
}

// src/lib.rs
pub mod routes;

use std::{collections::HashMap, sync::{Arc, Mutex}};
use rust_web_server::{dev_mcp::DevTools, prelude::*, state::AppWithState};

#[derive(Clone, Default)]
pub struct Store {
    pub items: Arc<Mutex<HashMap<u64, String>>>,
}

pub fn app() -> AppWithState<Store> {
    routes::items::register(App::with_state(Store::default()))
}

pub fn dev_tools() -> DevTools {
    DevTools::new().app_dir(env!("CARGO_MANIFEST_DIR"))
}

// src/main.rs
use rust_web_server::{dev_mcp::DevMcpExt, server::Server};

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, my_app::app().dev_mcp_with(my_app::dev_tools())).await;
}
```

- Typed handlers (18.28, `macros` + `serde` features): extractors as
  arguments (`rust_web_server::handler::{Path, Query, Form, Json, State}`,
  `&S`, `&Request`), any `IntoResponse` back (`Json(v)`, `(status, v)`,
  `Result<_, AppError>` — errors are problem details), registered with
  `.route(f)`. `async fn` ones go on `App::with_async_state(s)` (take
  `State(s): State<S>`). `app.route_conflicts()` lists routes an earlier
  one hides — assert it's empty in a test.
- Patterns: `/items/{id}` or `/items/:id` (one segment), `/files/{*path}` or
  `/files/*path` (the rest). `params.get("id")`.
- Classic handlers, `.get(pattern, |request, params, connection, state| …)`, are **synchronous** (`Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response`)
  and run on a blocking pool, so blocking I/O is fine. `App::with_async_state`
  takes `async fn` handlers (for `DbPool`, `AsyncClient`).
- State is cloned per server worker: shared mutable data goes behind
  `Arc<Mutex<…>>` inside the state.
- Unrouted requests fall through to the built-ins: static files from the
  working directory, `/healthz`, `/readyz`, `/metrics`.

## Recipes

| Task | How |
|---|---|
| JSON in | `serde_json::from_slice::<T>(&request.body)`; answer 400 on `Err` |
| JSON out | `Response::json(&STATUS_CODE_REASON_PHRASE.n200_ok, serde_json::to_vec(&v)?)` |
| Status codes | `STATUS_CODE_REASON_PHRASE.n201_created`, `.n404_not_found`, … |
| Errors as types | implement `IntoResponse` on an error enum, or `AppError::NotFound(..).into_response()` |
| Request headers | `request.header("authorization")` (case-insensitive) |
| Query string | `rust_web_server::extract::Query` |
| Middleware | `app.wrap(layer)`; first wrapped = outermost. `JwtLayer`, `BasicAuthLayer`, `RateLimitLayer`, `RequestIdLayer`, `CacheLayer`, `OtelLayer`, `ReverseProxy`, … |
| OpenAPI | `.openapi(OpenApiConfig::new(..).operation("GET", "/items/{id}", Operation::new("..").response::<Item>(200, "..")))`; types `#[derive(JsonSchema)]` |
| MCP tools | `McpServer::new(name, v).typed_tool(name, desc, \|ctx, args: In\| -> Result<Out, String>)`; `.require_bearer(token)`; `app.mcp(name, v)` for an app + tools |
| Let the agent inspect the app | `use rust_web_server::dev_mcp::DevMcpExt;` then `app.dev_mcp()` (18.28): `/mcp` with `routes` (+ middleware), `config` (secrets hidden), `layout` (`rws.app.toml`), `recent_errors`, and via `.dev_mcp_with(DevTools::new().model::<Post>().database(pool).migrations("migrations"))` `models`, `db_schema`, `migrations`; run with `RWS_ENV=dev` and `claude mcp add --transport http app http://localhost:7878/mcp` |
| Streaming response | a reader in `response.stream_pipe`; SSE: `Sse::stream()` |
| WebSocket | `.websocket("/chat/{room}", \|ws, params, state\| …)` |
| Database | `DbPool`, `#[derive(Model)]`, `QueryBuilder` (`model-*` features, async handlers) |
| gRPC service | `tower::mount("/pkg.Service/", tonic_service, app)` (`tower` feature) |
| Proxy without code | `rws.config.toml` with `[[upstream]]` + `[[route]]`; check it with `rws --check` |

## Tests

```rust
let client = rust_web_server::test_client::TestClient::new(app());
let response = client.post("/items").body_json(r#"{"name":"x"}"#).send();
assert_eq!(201, response.status());
let body: serde_json::Value = serde_json::from_slice(response.body_bytes()).unwrap();
```

`TestClient` needs no socket. It doesn't drive `stream_pipe`: read the
reader in the test.

## Pitfalls

- Configuration precedence: defaults < environment (`RWS_CONFIG_*`) <
  `rws.config.toml` in the working directory < command-line flags — a config
  file where the server runs overrides the environment, port included.
- The default port is 7878; `RWS_CONFIG_PORT` or `--port` changes it.
- An MCP endpoint must have a token (`.require_bearer`) or OAuth; never leave
  `/mcp` open. The one exception is `dev_mcp()` under `RWS_ENV=dev`, meant for
  a local port; outside `dev` it mounts only with `MCP_TOKEN` set.
- `Response` bodies are `content_range_list`; build them with
  `Response::json`/`Response::text`, not by hand.
- Don't add hyper, axum or a JSON-RPC crate: rws has its own server, router
  and MCP implementation.
