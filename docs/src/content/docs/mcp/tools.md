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
| `progress_token` | `Option<String>` | `params._meta.progressToken` from this `tools/call` request, if the client sent one — see [Progress reporting](#progress-reporting) |

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

## Progress reporting

If a client includes `params._meta.progressToken` on a `tools/call` request, it's asking for periodic progress updates while the tool runs. `.tool_with_context()` handlers can send them with `ctx.report_progress(progress, total, message)`, which pushes a `notifications/progress` event over the [`GET /mcp` SSE stream](/mcp/overview/#sse-streaming-transport):

```rust
use rust_web_server::mcp::{McpContent, McpServer};

let mcp = McpServer::new("my-server", "1.0")
    .tool_with_context(
        "process_batch",
        "Process a large batch of records",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            ctx.report_progress(0.0, Some(100.0), Some("starting"));
            // ... do the first half of the work ...
            ctx.report_progress(50.0, Some(100.0), Some("halfway"));
            // ... finish the work ...
            ctx.report_progress(100.0, Some(100.0), Some("done"));
            Ok(McpContent::text("batch processed"))
        },
    );
```

`total` and `message` are both optional (`None` omits them from the pushed event). Only `.tool_with_context()` handlers can report progress — a plain `.tool()` handler never receives `McpContext` at all, so it has no `progress_token` to report against.

:::note[Always safe to call]
`report_progress` silently does nothing if the client didn't ask for progress updates (`ctx.progress_token` is `None`) or if `ctx` wasn't built through a live server (e.g. `handle_request()`'s empty default context, rather than `execute()` — see the note above about calling `handle_request()` directly). A handler never needs to check whether progress reporting is actually possible before calling it.
:::

## Tool annotations

The MCP 2025-03-26 spec adds **annotations** — behavioral hints that clients like Claude Desktop use to decide whether to warn or ask for confirmation before calling a tool (e.g. skip confirmation for a read-only tool, warn before a destructive one). Register them with `.tool_annotated()`, which takes the same arguments as `.tool()` plus a `ToolAnnotations` value:

```rust
use rust_web_server::mcp::{McpServer, McpContent, ToolAnnotations};

let mcp = McpServer::new("my-server", "1.0")
    .tool_annotated(
        "delete_file",
        "Delete a file from disk",
        r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
        ToolAnnotations {
            destructive_hint: Some(true),
            read_only_hint: Some(false),
            idempotent_hint: Some(true), // deleting twice = deleting once
            ..Default::default()
        },
        |_args| Ok(McpContent::text("deleted")),
    );
```

`ToolAnnotations` fields:

| Field | Type | Meaning |
|---|---|---|
| `read_only_hint` | `Option<bool>` | The tool does not modify its environment |
| `destructive_hint` | `Option<bool>` | The tool may perform destructive updates (only meaningful when `read_only_hint` isn't `Some(true)`) |
| `idempotent_hint` | `Option<bool>` | Calling the tool repeatedly with the same arguments has no additional effect beyond the first call |
| `open_world_hint` | `Option<bool>` | The tool may interact with an open-ended set of external entities (e.g. web search), as opposed to a fixed, closed set |

Every field defaults to `None` — build a partial set with `..Default::default()`. Only fields that are `Some` are serialized, using the spec's camelCase key names, into the tool's `tools/list` entry:

```json
{"name":"delete_file","description":"Delete a file from disk","inputSchema":{...},"annotations":{"destructiveHint":true,"readOnlyHint":false,"idempotentHint":true}}
```

A plain `.tool()` or `.tool_with_context()` tool has no `annotations` key at all.

:::caution[Hints, not enforcement]
These are advisory only — nothing in `McpServer` verifies that a handler registered with `read_only_hint: Some(true)` actually refrains from writing to disk. Set them accurately; a client may still ask for confirmation regardless.
:::

:::note[No combined context + annotations builder]
`.tool_annotated()`'s handler is `Fn(&str) -> Result<McpContent, String>` — the same plain shape as `.tool()`, not the `Fn(McpContext, &str) -> ...` shape of `.tool_with_context()`. There is currently no single builder that gives you both `McpContext` and `ToolAnnotations` on the same tool.
:::

## McpContent variants

```rust
use rust_web_server::mcp::McpContent;

// Plain text — sets mimeType to text/plain
McpContent::text("Operation completed successfully");

// JSON — sets mimeType to application/json
McpContent::json(r#"{"count": 42, "items": ["a", "b"]}"#);

// Image — data is base64-encoded binary, mime_type e.g. "image/png"
McpContent::image(base64_encoded_png, "image/png");

// Embedded resource — a resource included directly in the tool's response,
// as opposed to one the client fetches separately via resources/read
McpContent::embedded("docs://readme", "# My Project\n...", "text/markdown");
```

`McpContent::image(data, mime_type)` serializes to `{"type":"image","data":"<b64>","mimeType":"..."}`. This crate has no third-party dependencies, so it doesn't ship a base64 encoder — `data` must already be base64-encoded by the caller before it's passed in.

`McpContent::embedded(uri, text, mime_type)` serializes to `{"type":"resource","resource":{"uri":"...","mimeType":"...","text":"..."}}`.

All four variants work anywhere an `McpContent` is expected — tool results (`tools/call`) and prompt messages (`prompts/get`) alike.

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
