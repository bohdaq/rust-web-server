# AGENTS.md

A JSON API on [rust-web-server](https://rws8.tech) (rws). For coding agents
working in this project.

- `cargo test` runs every test without opening a socket (`TestClient`).
- `cargo run` serves on `http://127.0.0.1:7878` (change with
  `RWS_CONFIG_PORT=8080`); `/openapi.json` and `/docs` describe the API.
- `RWS_ENV=dev cargo run` also serves the dev MCP endpoint at `/mcp`:
  `claude mcp add --transport http app-dev http://127.0.0.1:7878/mcp`, then
  ask it for `routes`, `layout`, `config`, `recent_errors`. Outside
  `RWS_ENV=dev` it is mounted only with `MCP_TOKEN` set (and requires it).

## Layout

The conventional rws layout, named in `rws.app.toml` (the dev endpoint's
`layout` tool reads it). Put new code where it says:

- `src/main.rs`: wiring only (setup, `dev_mcp`, `Server::run_tls`).
- `src/lib.rs`: `app()` (the routes and OpenAPI), `Store` (the state),
  `dev_tools()`.
- `src/routes/<resource>.rs`: a resource's types and typed handlers, its
  `register(app)` and `describe(openapi)`. A new resource is a new module,
  added in `routes/mod.rs`, `app()` and `openapi()`.
- `tests/`: integration tests against `app()`; `tests/dev_mcp.rs` checks
  every dev MCP tool answers (CI runs it).
- `src/models/`, `src/jobs/`, `migrations/`, `templates/`, `static/`: where
  `#[derive(Model)]` types, jobs, SQL migrations, Tera templates and static
  files go when the app gets them.

New code of those kinds comes from `rws gen` (the `rws` binary of the same
release, `cargo install rust-web-server`): `rws gen route posts
title:String`, `rws gen test posts`, `rws gen model Post title:String`
(adds the `model-sqlite` feature), `rws gen migration <name>`. It writes
against this release's API, registers the routes in `app()`, and never
overwrites a file; `--dry-run` shows the plan. Add the new routes to
`openapi()` by hand. https://rws8.tech/building-apps/generators/

## rws, in brief

Docs for this version: https://rws8.tech, one file:
https://rws8.tech/llms-full.txt.

- Typed handlers: `#[get("/todos/{id}")] fn show(Path(id): Path<u64>, store:
  &Store) -> Result<Json<Todo>, AppError>`, registered with `.route(show)`.
  Arguments are extractors (`Path`, `Json`, `Query`, `Form`, `State`, `&S`,
  `&Request`, …); the attribute checks `{id}` against `Path` at compile
  time. Return anything `IntoResponse`: `Json(v)`, `(status, Json(v))`,
  `Result<_, AppError>`.
- Errors: `AppError::NotFound(msg)` & co. answer problem details
  (`application/problem+json`, `detail` = msg); extractor rejections too.
- State is shared behind an `Arc`; keep mutable data behind a `Mutex`.
- `app().route_conflicts()` lists routes that never run (a test checks it
  is empty): register `/todos/new` before `/todos/{id}`.
- OpenAPI: describe each route in its module's `describe()` with
  `Operation` and `#[derive(JsonSchema)]` types, using the handler's
  `METHOD`/`PATTERN` — the document follows serde's renames.
- Middleware: `app.wrap(layer)`; built-ins (auth, rate limits, CORS, request
  ids, …): https://rws8.tech/building-apps/middleware/.
