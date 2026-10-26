---
title: Examples
description: Runnable example apps in the repository — a first route, a JSON API, a static site, a reverse proxy, server-sent events, streamed uploads and MCP servers — each with its own test.
---

The repository's [`examples/`](https://github.com/bohdaq/rust-web-server/tree/main/examples)
directory holds small, complete apps. Each one runs with `cargo run --example
<name>` from a clone and has a test that `cargo test` runs, so they keep
compiling and working as the crate changes.

```bash
git clone https://github.com/bohdaq/rust-web-server && cd rust-web-server
cargo run --example hello
curl localhost:7878/hello/world      # Hello, world!
```

Without a certificate they serve plain HTTP/1.1 on port 7878; a
`rws.config.toml` in the working directory, environment variables and
command-line flags apply as they do to `rws` (see
[Configuration](/configuration/env-vars/)).

| Example | Shows | Run | Read more |
|---|---|---|---|
| [`hello`](https://github.com/bohdaq/rust-web-server/blob/main/examples/hello.rs) | Two routes and a path parameter | `cargo run --example hello` | [Routing](/building-apps/routing/) |
| [`rest_api`](https://github.com/bohdaq/rust-web-server/blob/main/examples/rest_api.rs) | JSON CRUD over shared state, typed errors (400, 404, 422) | `cargo run --example rest_api --features serde` | [JSON](/building-apps/json/), [Error handling](/building-apps/error-handling/) |
| [`static_site`](https://github.com/bohdaq/rust-web-server/blob/main/examples/static_site.rs) | The working directory as a site, behind request-id and rate-limit middleware | `cargo run --example static_site` | [Static files](/features/static-files/), [Middleware](/building-apps/middleware/) |
| [`reverse_proxy`](https://github.com/bohdaq/rust-web-server/blob/main/examples/reverse_proxy.rs) | `/api/*` forwarded to a backend, the rest served locally | `cargo run --example reverse_proxy` | [Reverse proxy](/proxy/reverse-proxy/) |
| [`sse_ticker`](https://github.com/bohdaq/rust-web-server/blob/main/examples/sse_ticker.rs) | Server-sent events written as they are produced | `cargo run --example sse_ticker` | [Server-sent events](/features/sse/) |
| [`upload`](https://github.com/bohdaq/rust-web-server/blob/main/examples/upload.rs) | Large uploads streamed to disk, not buffered | `cargo run --example upload` | [Forms and uploads](/building-apps/forms-uploads/) |
| [`mcp_inventory`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_inventory.rs) | One domain served as a REST API and as MCP tools for agents | `cargo run --example mcp_inventory --features "macros serde sso"` | [Agent tools](/mcp/agent-tools/) |
| [`mcp_gateway`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_gateway.rs) | One authenticated MCP endpoint in front of several MCP servers | `cargo run --example mcp_gateway` | [MCP gateway](/mcp/gateway/) |
| [`mcp_conformance`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_conformance.rs) | The server the official MCP conformance suite tests | `cargo run --example mcp_conformance` | [MCP overview](/mcp/overview/) |

The comment at the top of each file has the `curl` commands to try it.

## Testing an example

Each example builds its app in a function (`app()`) that `main` serves and
its test drives through [`TestClient`](/testing/test-client/) — no socket,
no port. Copy that split into your own app:

```bash
cargo test --example rest_api --features serde
```

## For AI coding agents

[`AGENTS.md`](https://github.com/bohdaq/rust-web-server/blob/main/AGENTS.md)
at the repository root tells a coding agent how to build an app on the crate:
which entry point to start from, the handler signature, the conventions that
aren't obvious from the types, and where in these docs each topic lives. The
whole documentation is also available as one file,
[`llms-full.txt`](https://github.com/bohdaq/rust-web-server/blob/main/llms-full.txt).
