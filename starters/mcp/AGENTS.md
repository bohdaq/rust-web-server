# AGENTS.md

An MCP server on [rust-web-server](https://rws8.tech) (rws). For coding
agents working in this project.

- `cargo test` runs every test without opening a socket (`TestClient`).
- `MCP_TOKEN=… cargo run` serves MCP at `http://127.0.0.1:7878/mcp`
  (`RWS_CONFIG_PORT` changes the port); it refuses to start without a token.
- Everything is in `src/main.rs`: types, `server()` (tools and resources),
  `main`, tests. Keep that shape.

rws's MCP server, in brief (docs for this version: https://rws8.tech/mcp/overview/,
all docs in one file: https://rws8.tech/llms-full.txt):

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
- A REST API beside the tools: `App::with_state(...)...mcp(name, version)`.
