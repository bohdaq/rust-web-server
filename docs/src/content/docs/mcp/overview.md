---
title: MCP Server Overview
description: Expose tools, resources, and prompts to AI agents via the Model Context Protocol over HTTP.
---

The Model Context Protocol (MCP) is a JSON-RPC 2.0 standard that lets AI agents (Claude Desktop, Cursor, custom agents) call server-defined **tools**, read **resources**, and retrieve **prompt templates** over plain HTTP. `rust-web-server` ships a first-class `McpServer` that implements the MCP 2024-11-05 specification with no external dependencies, and negotiates that version down for clients that ask for something different — see [Protocol version negotiation](#protocol-version-negotiation) below.

## Creating an MCP server

```rust
use rust_web_server::server::Server;
use rust_web_server::mcp::{McpServer, McpContent, PromptMessage};

let mcp = McpServer::new("my-server", "1.0")
    .tool(
        "echo",
        "Echo text back to the caller",
        r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
        |args| {
            let text = rust_web_server::mcp::extract_arg(args, "text")
                .unwrap_or_else(|| "(nothing)".to_string());
            Ok(McpContent::text(text))
        },
    )
    .resource(
        "docs://{topic}",
        "Documentation",
        "Return documentation for a topic",
        |uri| Ok(McpContent::text(format!("Docs for: {uri}"))),
    )
    .prompt(
        "summarize",
        "Summarize the given text",
        |args| {
            let text = rust_web_server::mcp::extract_arg(args, "text")
                .unwrap_or_else(|| "some text".to_string());
            Ok(vec![PromptMessage::user(format!("Please summarize: {text}"))])
        },
    );

// Pass directly to the server — McpServer implements Application.
// let (listener, pool) = Server::setup().unwrap();
// Server::run(listener, pool, mcp);
```

## Attaching MCP to an existing app

If you already have routes, state, or middleware, use `.wrap()` so that non-MCP requests fall through to your existing `Application`:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::mcp::{McpContent};

let server = App::new()
    .mcp("my-server", "1.0")
    .tool("ping", "Ping the server", "{}", |_| Ok(McpContent::text("pong")))
    .wrap(App::new()); // non-MCP requests handled by the built-in App
```

:::note[app.mcp() shorthand]
`App::new().mcp(name, version)` is equivalent to `McpServer::new(name, version)` with the built-in `App` automatically wired as the fallback. Both forms pass directly to `Server::run`.
:::

## MCP endpoint

All JSON-RPC 2.0 messages travel over `POST /mcp`. The endpoint handles the full MCP lifecycle:

| JSON-RPC method      | Purpose                                      |
|----------------------|----------------------------------------------|
| `initialize`         | Capability negotiation and server info       |
| `ping`               | Liveness check                               |
| `tools/list`         | List all registered tools                    |
| `tools/call`         | Invoke a tool by name                        |
| `resources/list`     | List all registered resources                |
| `resources/read`     | Read a resource by URI                       |
| `prompts/list`       | List all registered prompt templates         |
| `prompts/get`        | Retrieve a rendered prompt by name           |

`OPTIONS /mcp` is also handled for CORS preflight. All other methods return `405 Method Not Allowed`.

Override the default path with `.at("/custom-path")` if needed.

## Protocol version negotiation

`initialize` inspects the client's requested `params.protocolVersion` and responds with the lower of that and the server's own version, rather than always claiming its own regardless of what the client asked for:

```json
// Client requests a newer version than this server implements:
{"method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "my-client", "version": "1.0"}}}

// Server responds with the version it actually speaks — not "2025-06-18":
{"result": {"protocolVersion": "2024-11-05", "capabilities": {...}, "serverInfo": {...}}}
```

Version strings are `YYYY-MM-DD` dates, so a plain string comparison already orders them correctly — no date parsing needed. A client requesting an *older* version than the server's is honored as sent (the server confirms it'll speak that version) rather than being overridden. If `protocolVersion` or `params` is missing entirely, `initialize` doesn't error — it falls back to the server's own version, same as before this negotiation existed.

`params.clientInfo` (if the client sends it) is logged to stderr at `initialize` time; there's no session storage yet to make it available to tool handlers later in the connection.

## Built-in rws tools

The binary ships 8 built-in tools when run in MCP mode via `app.mcp(...)`:

| Tool name             | Description                              |
|-----------------------|------------------------------------------|
| `server_config`       | Return current server configuration      |
| `feature_flags`       | List compiled feature flags              |
| `server_metrics`      | Prometheus-format metrics snapshot       |
| `rate_limit_config`   | Current rate limit settings              |
| `check_rate_limit`    | Check remaining quota for a client IP   |
| `cors_config`         | Active CORS rules                        |
| `list_static_files`   | Files served from the static root        |
| `reload_config`       | Trigger a hot config reload              |

## Connecting Claude Desktop

Add the server to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "my-server": {
      "url": "http://localhost:7878/mcp"
    }
  }
}
```

With bearer token authentication:

```json
{
  "mcpServers": {
    "my-server": {
      "url": "http://localhost:7878/mcp",
      "headers": {
        "Authorization": "Bearer your-token-here"
      }
    }
  }
}
```

## Connecting Cursor

In Cursor settings under **MCP Servers**, add:

```json
{
  "my-server": {
    "url": "http://localhost:7878/mcp"
  }
}
```

Or for HTTPS deployments:

```json
{
  "my-server": {
    "url": "https://api.example.com/mcp",
    "headers": {
      "Authorization": "Bearer your-token-here"
    }
  }
}
```
