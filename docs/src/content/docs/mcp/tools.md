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

## Async tool handlers

Requires the `http2` feature. A tool whose work is naturally async — an `AsyncClient` HTTP call, an async database query, awaiting another future — doesn't have to block a thread to do it inside a plain synchronous `.tool()` handler. Register it with `.async_tool()` instead:

```rust
use rust_web_server::mcp::{McpContent, McpServer};

let mcp = McpServer::new("my-server", "1.0")
    .async_tool(
        "call_api",
        "Call an external API",
        r#"{"type":"object"}"#,
        |_args: &str| async move {
            // let resp = AsyncClient::new().get("https://api.example.com").send().await?;
            Ok(McpContent::text("response"))
        },
    );
```

The handler signature is `Fn(&str) -> impl Future<Output = Result<McpContent, String>>` — same arguments and return type as a plain `.tool()` handler, just `async`. `.register_async_tool(name, description, schema, handler)` is the dynamic (`&self`) equivalent of `.async_tool()`, usable after the server is already serving requests, the same way [`.register_tool()`](/mcp/overview/#dynamic-registration) is for sync tools. `.remove_tool(name)` removes a tool by name regardless of whether it was registered as sync or async — you don't need to remember which kind it was.

`tools/list` lists sync and async tools together, and `tools/call` dispatches to whichever collection has a matching name — from a client's point of view there is no difference between the two.

:::note[How this bridges into `Application::execute`]
`Application::execute` (and therefore `tools/call`) is a synchronous trait method. `.async_tool()`'s handler is driven to completion via `crate::async_bridge::block_on_isolated` — the same mechanism `H2ReverseProxy` and `AsyncAppWithState` already use to call async code from sync trait methods — rather than `tokio::task::block_in_place`, which only works on the `multi_thread` tokio scheduler and panics under `current_thread`. `block_on_isolated` works either way: it spawns a scoped thread with its own single-threaded runtime if already inside one, or builds a temporary runtime directly if not.
:::

:::caution[No context or annotations support yet]
Like `.tool()` (not `.tool_with_context()`), an async tool's handler only receives `arguments` — there is no `.async_tool_with_context()` or `.async_tool_annotated()` yet.
:::

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

## Cancellation

A client can send `notifications/cancelled` to ask the server to abort a long-running `tools/call`. `.tool_with_context()` handlers can check `ctx.is_cancelled()` between steps of their own work and stop early:

```rust
use rust_web_server::mcp::{McpContent, McpServer};

let mcp = McpServer::new("my-server", "1.0")
    .tool_with_context(
        "process_batch",
        "Process a large batch of records",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            for i in 0..1_000_000 {
                if ctx.is_cancelled() {
                    return Err("cancelled by client".to_string());
                }
                // ... process item i ...
            }
            Ok(McpContent::text("done"))
        },
    );
```

:::caution[Cooperative, not preemptive]
Rust cannot forcibly interrupt a running synchronous closure — there is no mechanism to stop a handler mid-step unless the handler itself checks `is_cancelled()` and chooses to return early. A handler that never calls it runs to completion regardless of any `notifications/cancelled` the client sends, exactly as if this feature didn't exist. This is the same limitation `with_timeout`/`with_timeout_state` document for per-route timeouts — Rust's synchronous execution model has no forced-preemption primitive to build on.
:::

Only `.tool_with_context()` handlers can check cancellation — a plain `.tool()` handler never receives `McpContext`. `is_cancelled()` is always safe to call: it returns `false` if the client never sent a cancellation, if this wasn't a `tools/call` (the only method cancellation applies to), or if `ctx` has no live server behind it.

## Server-side sampling

Most MCP traffic flows client → server. `sampling/createMessage` reverses that: the *server* asks the connected client to run LLM inference, and the client answers. This is useful for agent-to-agent or meta-agent patterns — a tool that needs its own LLM call to decide what to do next, using whatever model the client already has configured, rather than the server managing its own API key and provider.

Call `ctx.sample(request, timeout)` from a `.tool_with_context()` handler:

```rust
use rust_web_server::mcp::{McpServer, PromptMessage, SamplingRequest};
use std::time::Duration;

let mcp = McpServer::new("my-server", "1.0")
    .tool_with_context(
        "ask_llm",
        "Ask the connected client's model a question",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            let response = ctx.sample(
                SamplingRequest {
                    messages: vec![PromptMessage::user("What is 2+2?")],
                    max_tokens: 100,
                    system_prompt: None,
                },
                Duration::from_secs(30),
            )?;
            Ok(response.content)
        },
    );
```

`SamplingRequest.messages` reuses [`PromptMessage`](/mcp/prompts/#promptmessage) rather than a near-identical `SamplingMessage` type — the spec's sampling message shape (`{"role":...,"content":{"type":"text",...}}`) is exactly what `PromptMessage` already models, including its `::user()`/`::assistant()` constructors. `SamplingResponse` (what `ctx.sample()` returns on success) has `role`, `content` (an `McpContent`), `model` (which model the client actually used), and `stop_reason`.

:::caution[This blocks the calling thread]
`ctx.sample()` is not `async fn` — it blocks synchronously until the client responds or `timeout` elapses. This is deliberate: tool handlers in this crate are plain synchronous closures, with no async tool handler support to `.await` inside of yet. On a thread-pool server this ties up one worker thread for up to `timeout` — the same tradeoff per-route timeouts (`with_timeout`) already accept from the other direction.
:::

`ctx.sample()` fails fast, before sending anything, if:

- The client's `initialize` call never declared `capabilities.sampling` — sampling is a *client* capability the spec has the client declare (not something a server advertises), so a client that never said it could handle sampling requests won't get one sent to it.
- This request has no session id (`Mcp-Session-Id`) — there'd be no way to address the request to a specific client connection.
- `ctx` has no live server behind it (e.g. a context built by hand via `handle_request_with_context` in a test, rather than a real request through `execute()`).

Otherwise, it fails with a timeout error if the client doesn't respond in time — including if the client simply has no [`GET /mcp` SSE connection](/mcp/overview/#sse-streaming-transport) open for that session at all, since there's no separate "not connected" signal.

## Filesystem roots

A client can advertise which filesystem roots (workspace directories, mounted volumes) it has access to, so a file-system-aware tool knows to stay within them rather than assuming access to the whole filesystem. `ctx.list_roots(timeout)` asks the connected client for its roots, built on the exact same request/response mechanism as [`ctx.sample()`](#server-side-sampling) — a server-initiated `roots/list` request over SSE, answered by the client's `POST /mcp`:

```rust
use rust_web_server::mcp::{McpContent, McpServer};
use std::time::Duration;

let mcp = McpServer::new("my-server", "1.0")
    .tool_with_context(
        "list_workspace_files",
        "List files in the client's workspace",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            let roots = ctx.list_roots(Duration::from_secs(10))?;
            let names: Vec<String> = roots.iter().map(|r| r.uri.clone()).collect();
            Ok(McpContent::text(names.join(", ")))
        },
    );
```

Each `McpRoot` has a `uri` (typically a `file://` URI, per spec) and an optional human-readable `name`.

Unlike `ctx.sample()`, the result is **cached per session**: the first `list_roots()` call after `initialize` does a live round trip; every later call in the same session returns the cached list without sending anything, so a handler can call it on every invocation without worrying about spamming the client. The cache is invalidated when the client sends `notifications/roots/list_changed` — the next `list_roots()` call after that does a fresh round trip.

:::caution[Same blocking caveat as `ctx.sample()`]
`ctx.list_roots()` also blocks the calling thread rather than being `async fn`, for the same reason `ctx.sample()` does — see the caution above.
:::

`ctx.list_roots()` fails fast, before sending anything, under the same conditions as `ctx.sample()` (translated to roots): the client's `initialize` call never declared `capabilities.roots`, this request has no session id, or `ctx` has no live server behind it. Otherwise it fails with a timeout error if the client doesn't respond to a live request in time.

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
