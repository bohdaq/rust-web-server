# AGENTS.md — building an app on rust-web-server

For coding agents writing an application **on** this crate. (Changing the
crate itself? Read `CLAUDE.md` and `CONTRIBUTING.md` instead.)

Full docs: https://rws8.tech — all of it in one file:
[`llms-full.txt`](llms-full.txt). API: https://docs.rs/rust-web-server.
Working apps to copy from: [`examples/`](examples/), listed on
https://rws8.tech/getting-started/examples/; standalone starter projects in
[`starters/`](starters/) (`cargo generate --git https://github.com/bohdaq/rust-web-server starters/api`);
the same guidance as a skill / Cursor rule / Copilot instructions in
[`agents/`](agents/).

## Setup

```toml
[dependencies]
rust-web-server = "18"          # default features: HTTP/1.1, HTTP/2, HTTP/3, TLS, gzip/br/zstd
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Opt-in features you're likely to need: `serde` (JSON in and out),
`macros` (`#[derive(Model)]`, `#[derive(JsonSchema)]`), `auth` (JWT/Basic
middleware), `sso` (OIDC, MCP OAuth), `model-sqlite` / `model-postgres` /
`model-mysql` (ORM). The full list: https://rws8.tech/getting-started/features/.

## Start from this shape

Build the app in a function, serve it from `main`, test the function. See
[`examples/hello.rs`](examples/hello.rs) — every example follows it.

- `App::with_state(state)` → `.get/.post/.put/.patch/.delete(pattern, handler)`.
  Patterns: `/items/:id` or `/items/{id}` (one segment), `/files/*path` or
  `/files/{*path}` (the rest).
- A handler is `Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response`
  (a closure or a `fn`). It is **synchronous** and runs on a blocking pool;
  for `async` handlers use `App::with_async_state` (docs: building-apps/async-handlers).
- `Server::setup()` then `Server::run_tls(listener, pool, app).await` serves
  HTTPS + HTTP/2 + HTTP/3 when a certificate is configured, plain HTTP/1.1
  otherwise.
- Anything not routed falls through to the built-ins: static files from the
  working directory, `/healthz`, `/readyz`, `/metrics`.
- Middleware: `app.wrap(layer)` (first wrapped = outermost). Built-in layers
  are listed at https://rws8.tech/building-apps/middleware/.
- An MCP server for agents: `app.mcp(name, version)` (docs: mcp/overview,
  example `mcp_inventory`).

## Things the types don't tell you

- **State must be `Clone`** to be served (`run_tls` clones the app per
  worker). Share mutable data between the clones with `Arc<Mutex<…>>` /
  `Arc<RwLock<…>>` inside the state, as `examples/rest_api.rs` does.
- **Building responses:** `Response::text(status, &str)`,
  `Response::json(status, Vec<u8>)`, `Response::get_response(status, None, None)`
  for an empty body; statuses come from `STATUS_CODE_REASON_PHRASE.n404_not_found`
  and friends. Errors: return `AppError::NotFound(..).into_response()` or
  implement `IntoResponse` on your own error enum.
- **Headers:** read with `request.header("content-type")` (case-insensitive);
  add with `Header::new(Header::_CACHE_CONTROL, "no-cache")`.
- **Streaming:** a response body produced over time is a reader in
  `response.stream_pipe` (`examples/sse_ticker.rs`); a large request body is
  streamed only on routes you opt in with `.stream_body(method, pattern)`,
  read through `request.body.clone().into_reader()` (`examples/upload.rs`).
  Otherwise bodies are buffered in memory.
- **Configuration precedence:** defaults < environment (`RWS_CONFIG_*`) <
  `rws.config.toml` in the working directory < command-line flags. A config
  file in the directory you run from overrides your environment variables —
  including the port.

## Tests

`TestClient::new(app())` sends requests through the app without a socket:

```rust
let client = rust_web_server::test_client::TestClient::new(app());
assert_eq!(200, client.get("/healthz").send().status());
```

Docs: https://rws8.tech/testing/test-client/. `TestClient` doesn't drive
`stream_pipe`; read the reader in the test (see `examples/sse_ticker.rs`).

## Where to look

| Task | Docs page |
|---|---|
| Routes, path params, query strings | building-apps/routing, building-apps/extractors |
| JSON bodies, validation | building-apps/json, building-apps/validation |
| Errors | building-apps/error-handling |
| Database | database/ |
| Auth (JWT, Basic, OIDC, SAML) | features/auth, features/sso, features/saml |
| Reverse proxy in code or in `rws.config.toml` | proxy/reverse-proxy, proxy/config-driven |
| HTTPS, Let's Encrypt | features/https-tls, features/acme |
| Deploying (Docker, Kubernetes, systemd) | deployment/ |
| Every setting | configuration/env-vars, configuration/config-file |

Page paths are relative to https://rws8.tech/.
