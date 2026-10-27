# AGENTS.md

A JSON API on [rust-web-server](https://rws8.tech) (rws). For coding agents
working in this project.

- `cargo test` runs every test without opening a socket (`TestClient`).
- `cargo run` serves on `http://127.0.0.1:7878` (change with
  `RWS_CONFIG_PORT=8080`); `/openapi.json` and `/docs` describe the API.
- Everything is in `src/main.rs`: types, `app()` (the routes), `openapi()`,
  `main`, tests. Keep that shape: build the app in a function, test the
  function.

rws, in brief (docs for this version: https://rws8.tech, one file:
https://rws8.tech/llms-full.txt):

- Routes: `App::with_state(state).get(pattern, handler)` (also `.post`,
  `.put`, `.patch`, `.delete`). Patterns: `/todos/{id}` or `/todos/:id` for
  one segment, `/files/{*path}` for the rest; `params.get("id")`.
- A handler is a synchronous `Fn(&Request, &PathParams, &ConnectionInfo, &S)
  -> Response` running on a blocking pool; `App::with_async_state` for
  `async fn` handlers.
- State is cloned per worker: keep shared data behind `Arc<Mutex<…>>`.
- Responses: `Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, bytes)`;
  statuses are `STATUS_CODE_REASON_PHRASE.n404_not_found` and friends.
- Request body: `request.body` (bytes; `serde_json::from_slice`). Headers:
  `request.header("content-type")`.
- OpenAPI: describe each route in `openapi()` with `Operation` and
  `#[derive(JsonSchema)]` types — the document follows serde's renames.
- Middleware: `app.wrap(layer)`; built-ins (auth, rate limits, CORS, request
  ids, …): https://rws8.tech/building-apps/middleware/.
