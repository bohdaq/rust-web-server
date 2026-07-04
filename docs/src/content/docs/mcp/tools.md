---
title: MCP Tools
description: Register callable tools that AI agents can invoke via the MCP protocol.
---

Tools are the primary way AI agents interact with your server. Each tool has a name, a description, a JSON Schema describing its inputs, and a handler closure that executes when the tool is called.

## Registering a tool

```rust
use rust_web_server::mcp::{McpServer, McpContent};

let mcp = McpServer::new("my-server", "1.0")
    .tool(
        "greet",                          // name (snake_case recommended)
        "Return a greeting for a name",  // description shown to the AI
        r#"{
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name to greet"
                }
            },
            "required": ["name"]
        }"#,
        |args| {
            let name = rust_web_server::mcp::extract_arg(args, "name")
                .unwrap_or_else(|| "World".to_string());
            Ok(McpContent::text(format!("Hello, {name}!")))
        },
    );
```

## Handler signature

```rust
Fn(&str) -> Result<McpContent, String>
```

The handler receives the raw `arguments` JSON string from the MCP `tools/call` request. On success, return `Ok(McpContent)`. On failure, return `Err(String)` — this is sent back to the AI as `isError: true` (a tool-level error, not a protocol error).

## Per-request context

A plain `.tool()` handler only ever sees `arguments` — it has no way to know which client is calling, what session it's part of, or anything from the request's headers. `.tool_with_context()` registers a tool whose handler additionally receives an `McpContext`:

```rust
use rust_web_server::mcp::{McpServer, McpContent};

let mcp = McpServer::new("my-server", "1.0")
    .tool_with_context(
        "whoami",
        "Report the caller's client info",
        "{}",
        |ctx, _args| {
            let name = ctx.client_name.as_deref().unwrap_or("unknown client");
            let version = ctx.client_version.as_deref().unwrap_or("?");
            Ok(McpContent::text(format!("Called by {name} v{version}")))
        },
    );
```

`McpContext` fields:

| Field | Type | Source |
|---|---|---|
| `client_name` | `Option<String>` | This session's `initialize` call's `params.clientInfo.name` |
| `client_version` | `Option<String>` | This session's `initialize` call's `params.clientInfo.version` |
| `session_id` | `Option<String>` | The `Mcp-Session-Id` header on this request |
| `auth_claims` | `Option<String>` | Reserved for a future JWT-auth integration — always `None` today |

### How the session gets established

1. A client calls `initialize`, optionally with `params.clientInfo`.
2. The server mints a session id and returns it in an `Mcp-Session-Id` response header.
3. The client echoes that header on every later request (`tools/call`, etc.).
4. `execute()` reads the header, looks up the `clientInfo` recorded for that session, and builds the `McpContext` your handler receives.

You don't need to do anything to opt into this — it's automatic for any server driven through `execute()` (i.e. any real HTTP request, whether served directly or via `TestClient`).

:::note[Calling `handle_request()` directly]
`handle_request(body)` — used in tests that skip the HTTP layer — has no `Request` to read a session header from, so `tool_with_context` handlers see an empty `McpContext` (every field `None`). Use `handle_request_with_context(body, ctx)` to supply one explicitly.
:::

:::caution[Session storage has no eviction]
Recorded sessions accumulate for the life of the process — nothing removes an entry, since the MCP Streamable HTTP transport has no session-termination signal to key cleanup off of. Fine for a modest, roughly-stable set of long-lived AI-agent clients; not recommended as-is for a public-internet-facing server serving unbounded distinct clients.
:::

## McpContent variants

```rust
use rust_web_server::mcp::McpContent;

// Plain text — sets mimeType to text/plain
McpContent::text("Operation completed successfully");

// JSON — sets mimeType to application/json
McpContent::json(r#"{"count": 42, "items": ["a", "b"]}"#);
```

:::note[Images]
The MCP spec supports image content with base64 encoding. The current `McpContent` API covers text and JSON. For binary image responses, encode to base64, wrap as JSON, and return `McpContent::json(...)` with the base64 string and MIME type as fields.
:::

## Extracting arguments

`extract_arg` pulls a string value from the arguments JSON by field name:

```rust
use rust_web_server::mcp::extract_arg;

// args = r#"{"query": "SELECT 1", "limit": 10}"#
let query = extract_arg(args, "query");   // Some("SELECT 1")
let limit = extract_arg(args, "limit");   // Some("10")
let other = extract_arg(args, "missing"); // None
```

For numeric or boolean fields, parse the returned `String`:

```rust
let limit: usize = extract_arg(args, "limit")
    .and_then(|s| s.parse().ok())
    .unwrap_or(100);
```

## Input schema format

The `input_schema` parameter is a JSON Schema **object** serialized as a string. Required fields:

```rust
// Minimal schema — no arguments
"{}"

// Single required string argument
r#"{"type":"object","properties":{"sql":{"type":"string"}},"required":["sql"]}"#

// Multiple arguments, one optional
r#"{
    "type": "object",
    "properties": {
        "sql":   { "type": "string",  "description": "SQL query to execute" },
        "limit": { "type": "integer", "description": "Max rows to return", "default": 100 }
    },
    "required": ["sql"]
}"#
```

## Complete example: database query tool

```rust
use rust_web_server::mcp::{McpServer, McpContent, extract_arg};

// Assume `db` is your connection pool — Arc<DbPool> for thread safety.
fn build_mcp(db: std::sync::Arc<crate::db::Pool>) -> McpServer {
    let db_tool = db.clone();

    McpServer::new("my-api", "1.0")
        .tool(
            "query_users",
            "Query the users table by email or ID",
            r#"{
                "type": "object",
                "properties": {
                    "email": { "type": "string", "description": "Filter by email address" },
                    "id":    { "type": "integer", "description": "Filter by user ID" }
                }
            }"#,
            move |args| {
                let email = extract_arg(args, "email");
                let id: Option<i64> = extract_arg(args, "id")
                    .and_then(|s| s.parse().ok());

                let results = db_tool.query_users(email.as_deref(), id)
                    .map_err(|e| e.to_string())?;

                // Serialize to JSON and return
                let json = serde_json::to_string(&results)
                    .map_err(|e| e.to_string())?;
                Ok(McpContent::json(json))
            },
        )
}
```

## Error handling

Return `Err(String)` for recoverable tool errors (invalid input, resource not found, downstream failure). The AI receives an `isError: true` response and can decide how to proceed or report it to the user.

```rust
|args| {
    let id: u64 = extract_arg(args, "id")
        .ok_or_else(|| "Missing required argument: id".to_string())?
        .parse()
        .map_err(|_| "Argument 'id' must be a positive integer".to_string())?;

    fetch_record(id).map_err(|e| format!("Database error: {e}"))
        .map(|r| McpContent::json(r.to_json()))
}
```
