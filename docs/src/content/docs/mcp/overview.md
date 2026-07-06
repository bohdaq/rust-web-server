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

`OPTIONS /mcp` is handled for CORS preflight, and `GET /mcp` opens an SSE stream for server → client push — see [SSE streaming transport](#sse-streaming-transport) below. All other HTTP methods return `405 Method Not Allowed`.

Override the default path with `.at("/custom-path")` if needed.

## Batch requests

`POST /mcp` also accepts a top-level JSON array instead of a single object — a JSON-RPC 2.0 batch request, letting a client send several calls in one HTTP round trip:

```json
// Request:
[{"jsonrpc":"2.0","method":"tools/list","id":1},
 {"jsonrpc":"2.0","method":"ping","id":2}]

// Response — one entry per element, in order:
[{"jsonrpc":"2.0","result":{"tools":[...]},"id":1},
 {"jsonrpc":"2.0","result":{},"id":2}]
```

Each element is dispatched through the same method table as a standalone request, and each one's success or error is independent — one element failing (e.g. an unknown method) doesn't affect the others or fail the batch as a whole.

Elements with no `id` (notifications) contribute no entry to the response array, exactly like a standalone notification produces no response body. A batch made up entirely of notifications returns `202 Accepted` with an empty body. An empty array (`[]`) is itself invalid per the JSON-RPC spec — it gets back a single `Invalid Request` error object rather than an empty `[]`.

:::note[initialize inside a batch]
If a batch includes a successful `initialize`, the *first* one mints a session and attaches `Mcp-Session-Id` to the overall response, same as a standalone `initialize` would. Sending more than one `initialize` in a single batch is unusual and only the first is honored for session purposes — one HTTP response can only carry one session id.
:::

## Pagination

`tools/list`, `resources/list`, and `prompts/list` return every registered item in one response by default. For a server with a lot of tools or resources, call `.page_size(n)` when building the server to cap each response to `n` items and enable cursor-based pagination:

```rust
use rust_web_server::mcp::McpServer;

let server = McpServer::new("my-server", "1.0").page_size(50);
```

A response with more items remaining includes `"nextCursor"` — an opaque string the client echoes back as `params.cursor` on its next call to get the next page:

```json
// First call — no cursor:
{"method":"tools/list","params":{}}
// → {"result":{"tools":[...50 items...],"nextCursor":"NTA="}}

// Next call — cursor from the previous response:
{"method":"tools/list","params":{"cursor":"NTA="}}
// → {"result":{"tools":[...remaining items...]}}  — no nextCursor once exhausted
```

Claude Desktop and other MCP clients already send `cursor` back automatically once a `nextCursor` appears in a response — no extra client-side wiring is needed.

:::note[The cursor is opaque]
The cursor is base64 of a decimal offset, but treat it as an opaque token — don't construct or parse it yourself. A malformed or tampered cursor gets a JSON-RPC `INVALID_PARAMS` (`-32602`) error rather than silently resetting to the first page. An offset past the end of the list returns an empty page with no `nextCursor`, not an error.
:::

## SSE streaming transport

The MCP Streamable HTTP spec defines a second transport alongside `POST /mcp`: a client that sends `GET /mcp` instead gets back a `text/event-stream` response that stays open indefinitely, for server → client push (log messages, progress updates, list-changed notifications, and anything else you want to push proactively).

Call `.notify(method, params_json)` from anywhere in your code — a background thread, a webhook handler, another tool's own handler — to push a JSON-RPC notification to every client currently connected to the SSE stream:

```rust
use rust_web_server::mcp::McpServer;

let server = McpServer::new("my-server", "1.0");

// Elsewhere, e.g. after a background job finishes:
server.notify("notifications/message", Some(r#"{"level":"info","data":"job finished"}"#));
```

`params_json`, if given, must already be valid JSON (usually an object) — it's spliced into the notification verbatim, not escaped or re-serialized. `method` alone (no `id`) matches how the JSON-RPC spec defines a notification: fire-and-forget, no response expected.

```json
// What a connected client sees on the SSE stream after the call above:
data: {"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"job finished"}}
```

:::note[Backpressure and disconnection]
`.notify()` never blocks the calling thread. Each connected client has a bounded 32-frame buffer; a client that isn't reading fast enough (buffer full) is dropped from the broadcast list exactly like a disconnected one — one slow or stuck client can never stall notifications to everyone else. Idle connections receive a `: keep-alive` SSE comment every 15 seconds, both so intermediate proxies don't time out a silent connection and because it forces a write attempt that reveals a dead peer.
:::

:::caution[HTTP/1.1 only]
The SSE channel is only wired up for the plain HTTP/1.1 path (`Server::run`/`Server::process`). This matches the scope of `Response::stream_pipe` generally (the mechanism this feature is built on) — the HTTP/2 and HTTP/3 handlers don't drive `stream_pipe` for any response yet, not just this one.
:::

## Protocol version negotiation

`initialize` inspects the client's requested `params.protocolVersion` and responds with the lower of that and the server's own version, rather than always claiming its own regardless of what the client asked for:

```json
// Client requests a newer version than this server implements:
{"method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "my-client", "version": "1.0"}}}

// Server responds with the version it actually speaks — not "2025-06-18":
{"result": {"protocolVersion": "2024-11-05", "capabilities": {...}, "serverInfo": {...}}}
```

Version strings are `YYYY-MM-DD` dates, so a plain string comparison already orders them correctly — no date parsing needed. A client requesting an *older* version than the server's is honored as sent (the server confirms it'll speak that version) rather than being overridden. If `protocolVersion` or `params` is missing entirely, `initialize` doesn't error — it falls back to the server's own version, same as before this negotiation existed.

`params.clientInfo` (if the client sends it) is logged to stderr at `initialize` time and recorded under a freshly minted session id, returned to the client via an `Mcp-Session-Id` response header — see [Per-request context](/mcp/tools/#per-request-context) for how a `.tool_with_context()` handler gets it back on later requests in the same session.

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
