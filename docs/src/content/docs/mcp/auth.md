---
title: MCP Authentication
description: Protect your MCP endpoint with a static Bearer token using require_bearer.
---

By default the MCP endpoint (`POST /mcp`) is open to any caller. Call `.require_bearer(token)` to gate every request behind a static Bearer token.

## require_bearer

```rust
use rust_web_server::mcp::McpServer;

let mcp = McpServer::new("my-server", "1.0")
    .require_bearer(std::env::var("MCP_TOKEN").expect("MCP_TOKEN not set"));
```

Every `POST /mcp` request must include:

```
Authorization: Bearer <token>
```

A missing or incorrect token produces an immediate `401 Unauthorized` before any JSON-RPC processing:

```
HTTP/1.1 401 Unauthorized
WWW-Authenticate: Bearer
Content-Type: text/plain

Unauthorized
```

## Loading the token from an environment variable

Never hard-code the token in source code. The conventional variable is `MCP_TOKEN`:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;

let mcp = App::new()
    .mcp("my-server", "1.0")
    .require_bearer(
        std::env::var("MCP_TOKEN").expect("MCP_TOKEN env var not set"),
    );
```

Set the variable before starting the server:

```sh
export MCP_TOKEN="$(openssl rand -hex 32)"
cargo run
```

The bundled `rws` binary reads `MCP_TOKEN` automatically when the config file enables the built-in MCP server.

## Configuring Claude Desktop

Add the `Authorization` header to the server entry in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "my-server": {
      "url": "http://localhost:7878/mcp",
      "headers": {
        "Authorization": "Bearer <your-token-here>"
      }
    }
  }
}
```

Replace `<your-token-here>` with the value of `MCP_TOKEN`.

## Full example

```rust
use rust_web_server::server::Server;
use rust_web_server::mcp::{McpServer, McpContent};

# #[cfg(not(feature = "http2"))]
# fn main() {
let token = std::env::var("MCP_TOKEN")
    .expect("Set MCP_TOKEN before starting the server");

let mcp = McpServer::new("secure-server", "1.0")
    .require_bearer(token)
    .tool(
        "ping",
        "Returns pong",
        r#"{"type":"object","properties":{}}"#,
        |_| Ok(McpContent::text("pong")),
    );

let (listener, pool) = Server::setup().unwrap();
Server::run(listener, pool, mcp);
# }
```

## CORS preflight

`OPTIONS /mcp` requests always receive `200 OK` without checking the Bearer token. This allows browser-based MCP clients to complete the CORS preflight before attaching credentials.

:::note[Multi-user auth]
`.require_bearer` enforces a single shared token — it is not a per-user authentication mechanism. For per-user access control (e.g. validating JWT claims or checking a database), implement a custom `Application` wrapper that inspects the `Authorization` header and delegates to `McpServer` only on success:

```rust
use rust_web_server::application::Application;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::mcp::McpServer;

struct AuthGate {
    inner: McpServer,
}

impl Application for AuthGate {
    fn execute(&self, req: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        // your custom auth logic here
        self.inner.execute(req, conn)
    }
}
```
:::
