---
title: Let a coding agent inspect your app
description: app.dev_mcp() mounts an MCP endpoint with your app's routes, middleware, configuration, recent errors, database schema and migrations, for Claude Code or Cursor while you develop.
---

A coding agent working on an rws app can read the code, but not what the
running app actually registered, which settings it ended up with, or why the
last request failed. `app.dev_mcp()` (since 18.28.0) puts an MCP endpoint in
front of the app that answers those questions: its routes and the
middleware around them, its effective configuration with secrets hidden, the
last requests and errors, and — with a `model-*` feature — its database
schema and migrations.

```rust
use rust_web_server::app::App;
use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::server::Server;

let app = App::with_state(state)
    .get("/api/items/:id", get_item)
    .post("/api/items", create_item)
    .wrap(CsrfLayer::new())
    .dev_mcp(); // /mcp in development, nothing in production
```

`dev_mcp()` works on every application type (`AppWithState`, `App`,
`WithMiddleware`, `AsyncAppWithState`, …) and returns one you serve as
before. Every path but `/mcp` still reaches the app.

## When it is mounted

| `RWS_ENV` | token set | `/mcp` |
|---|---|---|
| `dev` | no | mounted, **no authentication** |
| any | yes | mounted, requires `Authorization: Bearer <token>` |
| `test`, `prod`, unset | no | not mounted (one line on stdout says so) |

- `RWS_ENV` is `dev`, `test` or `prod` (the default; anything unrecognised
  counts as `prod`). See [environment variables](/configuration/env-vars/#environment-kind).
- The token is read from `MCP_TOKEN`, the variable the `rws` binary's
  [`/mcp`](/mcp/operate/) uses. `DevTools::token(…)` sets it in code,
  `DevTools::token_env("APP_DEV_TOKEN")` reads another variable.
- In development without a token anyone who can reach the port can call the
  tools, including the ones that change things: keep the port on
  `127.0.0.1` (`RWS_CONFIG_IP`) or set a token.
- Browser-based clients send an `Origin` header, which is refused unless it
  is listed with `DevTools::allowed_origins([…])`. CLI and desktop clients
  send none.
- Calls to the changing tools are recorded in the admin
  [audit log](/deployment/admin/#users-roles-and-the-audit-log), as for the binary.

Connect the agent while the app runs:

```bash
RWS_ENV=dev RWS_CONFIG_IP=127.0.0.1 cargo run
claude mcp add --transport http my-app http://localhost:7878/mcp
```

## The tools

| Tool | Answers |
|---|---|
| `routes` | Each route's method and pattern, in registration order, and the middleware layers around them in the order a request passes them: `{"routes":[{"method":"GET","pattern":"/api/items/:id"}],"middleware":["CsrfLayer"]}` |
| `config` | `RWS_ENV`, the compiled-in features and every `RWS_*`, `OTEL_*` and `MCP_*` setting in effect (config file and flags included). Values of settings named `*SECRET*`, `*PASSWORD*`, `*TOKEN*`, PKCS#11 PINs and passwords inside URLs (`postgres://app:(hidden)@db/app`) are hidden |
| `recent_requests` `{"n": 50}` | The last requests (at most 100): method, path, status, client IP, latency |
| `recent_errors` `{"n": 50}` | The last 4xx/5xx |
| `db_schema` | With `DevTools::database(pool)`: every table and its columns (name, type, nullable, default, primary key) — SQLite, PostgreSQL (the current schema) and MySQL (the current database) |
| `migrations` | With `DevTools::migrations(dir)` too: each migration file, whether it is applied and whether it has a `.down.sql` |
| `migrations_run` | Apply the pending migrations; answers the status afterwards |
| `migrations_rollback_last` | Undo the last applied migration; answers which (`null` when none) |

The middleware listed is the stack around the app's routes (each layer's
`Middleware::layer_name()`, by default its type name); rws has no per-route
middleware to report. Routes come from `Application::registered_routes()`: the
built-in controllers behind an `AppWithState` (static files, `/healthz`, …)
aren't listed.

`dev_mcp()` records the app's requests for `recent_requests` itself. If the
app already has a `LogLayer`, pass `.record_requests(false)` so each request
is listed once.

## Choosing what is mounted

`dev_mcp_with(DevTools)` takes the options:

```rust
use rust_web_server::dev_mcp::{DevMcpExt, DevTools};
use rust_web_server::model::DbPool;

let pool = DbPool::from_env().await?;
let app = routes
    .dev_mcp_with(
        DevTools::new()
            .database(pool.clone())
            .migrations("migrations")
            .with_ops()      // also the rws binary's 25 operations tools
            .read_only(),    // but none of the tools that change something
    );
```

| Method | Does |
|---|---|
| `.database(pool)` / `.migrations(dir)` | The database tools (`model-sqlite`, `model-postgres` or `model-mysql`) |
| `.with_ops()` | The [operations tools](/mcp/operate/#the-tools) as well: metrics, rate limits, maintenance mode, the IP blocklist, feature flags, authorization, traffic, config reload |
| `.cache(layer)` | The `cache_stats` / `cache_clear` operations tools for this response cache (a clone of the layer the app wraps); implies `.with_ops()` |
| `.read_only()` | Leaves out every tool that changes something (`migrations_run`, `migrations_rollback_last` and the operations tools in `dev_mcp::OPS_WRITE_TOOLS`) |
| `.token(t)` / `.token_env(name)` | The bearer token, or the variable it is read from (default `MCP_TOKEN`) |
| `.env(RwsEnv::Dev)` | Decide as if `RWS_ENV` were this (tests) |
| `.at("/_dev/mcp")` | Another path, for an app that serves its own MCP server at `/mcp` |
| `.server_info(name, version)` | What `initialize` reports (default `rws-dev`) |
| `.record_requests(false)` | Don't record requests (the app has a `LogLayer`) |

An app that builds its own `McpServer` and gates it itself can add the same
tools with `DevTools::new().register(server)` — the routes are those of the
app the server wraps. `dev_mcp::ops_tools(server, routes, cache)` adds only
the operations tools; it is what the `rws` binary mounts.

## Testing it

The tools are ordinary MCP tools, so `TestClient` drives them:

```rust
use rust_web_server::dev_mcp::{DevMcpExt, DevTools, RwsEnv};
use rust_web_server::test_client::TestClient;

let client = TestClient::new(app().dev_mcp_with(DevTools::new().env(RwsEnv::Dev)));
let res = client
    .post("/mcp")
    .body_json(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"routes","arguments":{}}}"#)
    .send();
assert!(res.body_text().contains("/api/items/:id"));
```

## Not yet

Models (no registry of `#[derive(Model)]` types exists at run time), jobs,
and the same data as MCP resources are planned with the conventional app
layout (roadmap #87).
