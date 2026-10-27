<!-- Generated from agents/skills/rws/SKILL.md by ci/gen_agents.py: edit that file. -->

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
```

Or by hand: `rust-web-server = { version = "18", features = [...] }` and
`tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`.
Features you'll want: `serde` (JSON), `macros` (`#[derive(JsonSchema)]`,
`#[derive(Model)]`), `openapi`, `auth`, `sso`, `model-sqlite`/`-postgres`/`-mysql`,
`tower` (tonic, tower-http).

## The shape of an app

```rust
use rust_web_server::prelude::*;

#[derive(Clone, Default)]
pub struct State; // shared data goes in here, behind Arc<Mutex<…>>

pub fn app() -> rust_web_server::state::AppWithState<State> {
    App::with_state(State::default())
        .get("/items/{id}", |_request, params, _connection, _state| {
            let id = params.get("id").unwrap_or_default();
            Response::json(&STATUS_CODE_REASON_PHRASE.n200_ok, format!(r#"{{"id":"{id}"}}"#).into_bytes())
        })
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, app()).await;
}
```

- Build the app in a function; serve it in `main`; test the function.
- Patterns: `/items/{id}` or `/items/:id` (one segment), `/files/{*path}` or
  `/files/*path` (the rest). `params.get("id")`.
- Handlers are **synchronous** (`Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response`)
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
  `/mcp` open.
- `Response` bodies are `content_range_list`; build them with
  `Response::json`/`Response::text`, not by hand.
- Don't add hyper, axum or a JSON-RPC crate: rws has its own server, router
  and MCP implementation.
