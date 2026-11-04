# AGENTS.md

An MCP server on [rust-web-server](https://rws8.tech) (rws). For coding
agents working in this project.

- `cargo test` runs every test without opening a socket (`TestClient`).
- `MCP_TOKEN=… cargo run` serves MCP at `http://127.0.0.1:7878/mcp`
  (`RWS_CONFIG_PORT` changes the port); it refuses to start without a token.
- `RWS_ENV=dev MCP_TOKEN=… cargo run` also serves the dev MCP endpoint at
  `/_dev/mcp` (the project's layout, config, recent requests and errors);
  outside `RWS_ENV=dev` only with `DEV_MCP_TOKEN` set (and requires it).

## Layout

The conventional rws layout, named in `rws.app.toml` — this app's "routes"
are its MCP tools, so the manifest says `routes = "src/tools"`:

- `src/main.rs`: wiring only (token check, setup, `dev_mcp`,
  `Server::run_tls`).
- `src/lib.rs`: `server()` (the MCP server), `Store`, `dev_tools()`.
- `src/tools/<subject>.rs`: a subject's types and its `register(server,
  store)` with the tools and resources. A new subject is a new module,
  added in `tools/mod.rs` and `server()`.
- `tests/`: integration tests against `server()`; `tests/dev_mcp.rs`
  checks every dev MCP tool answers (CI runs it).

`rws gen model` and `rws gen migration` (the `rws` binary of the same
release) write `#[derive(Model)]` types and migrations here too; `rws gen
route`/`test` are for HTTP apps with an `app()` and refuse this one — write
tools by hand. https://rws8.tech/building-apps/generators/

## rws's MCP server, in brief

Docs for this version: https://rws8.tech/mcp/overview/, all docs in one
file: https://rws8.tech/llms-full.txt.

- `McpServer::new(name, version)`; add tools with
  `.typed_tool(name, description, |ctx, args: In| -> Result<Out, String>)` —
  `In` and `Out` derive `serde` and `JsonSchema`, which become the tool's
  `inputSchema` / `outputSchema` (serde renames are followed). An `Err` is a
  tool error the agent sees, not a protocol failure.
- Resources: `.resource("scheme://{id}", name, description, |uri| …)`;
  prompts: `.prompt(...)`; completion: `.completion(...)`.
- Auth: `.require_bearer(token)`, `.require_bearer_tokens([...])` (per-agent
  identities in `ctx.auth_claims`), or `.require_oauth(provider, audience)`
  (`sso` feature).
- Both MCP protocol eras are served on one endpoint (2025-11-25 with sessions,
  2026-07-28 stateless); long-running work: `ctx.start_task(...)`.
- A REST API beside the tools: `App::with_state(...)...mcp(name, version)`,
  with typed handlers (`#[get("/notes/{id}")]`, `.route(f)`) in `src/routes/`.
