[Read Me](../README.md) > [Spec](.) > MCP TODO

# MCP TODO — Enhancement Backlog

Current implementation: `src/mcp/mod.rs`, targeting **MCP 2024-11-05** (Streamable HTTP transport).

Baseline covered: `initialize`, `tools/*`, `resources/*`, `prompts/*`, static Bearer auth,
`notifications/initialized` (202 no-content), `ping`, CORS preflight, `.wrap(app)` fallthrough.

---

## Priority 1 — Correctness and ergonomics (do first)

### ✅ TODO-1: Protocol version negotiation — Done (v17.75.0)

`initialize` used to always return `"protocolVersion":"2024-11-05"` regardless of what the client
sent. `do_initialize` now takes `body: &str`, extracts `params.protocolVersion` via
`json_rpc::extract_raw(body, "params")` + `json_rpc::extract_str(&params, "protocolVersion")`
(mirroring the same params-extraction pattern `do_tools_call`/`do_resources_read` already used),
and returns the lower of the client's and the server's `PROTOCOL_VERSION` — version strings are
`YYYY-MM-DD` dates, so a plain `&str` comparison (`v < PROTOCOL_VERSION`) already orders them
correctly with no date parsing needed. A client asking for a newer version than this server
implements is told the version it actually speaks (so it can abort if that's incompatible for it);
an older-version request is honored as sent. Missing `protocolVersion`/`params` falls back to the
server's own version rather than erroring `initialize` out — before this change `initialize` could
never fail, and that stays true. `params.clientInfo` (`name`/`version`), if sent, is logged to
stderr (`[mcp] initialize from client {name} v{version}`) — "store for logging" only ever meant
logging within that one request, since `McpServer`/`execute()` are fully stateless with no session
storage to carry it further (that's TODO-2's job, not this one's). The client-supplied version
string is `json_escape`d before being embedded back in the response JSON, same as `serverInfo`'s
fields already were — it's attacker-controlled input once decoded out of the incoming JSON by
`extract_str`, so it needs the same escaping on the way back out.

5 new tests in `src/mcp/tests.rs`: negotiating down for a newer client version, honoring an older
client version, echoing back a matching version, defaulting to the server version when
`protocolVersion` is absent, and defaulting when `params` is missing entirely (no error). The
existing `initialize_returns_protocol_version` test needed no changes.

---

### ✅ TODO-2: Per-request context in tool handlers — Done (v17.76.0)

Tool handlers used to receive only `arguments: &str` — no access to caller identity, session, or
HTTP headers, so a tool couldn't behave differently per user or log which MCP client called it.

Added `McpContext` (exactly the fields this entry specified) and `.tool_with_context(name, desc,
schema, |ctx: McpContext, args: &str| -> Result<McpContent, String> { ... })`. `.tool()` still
works unchanged — internally it now wraps the plain `Fn(&str) -> ...` closure in one that ignores
`McpContext`, so both builders share one `ToolFn` type (`Arc<dyn Fn(McpContext, &str) -> ...>`)
instead of `ToolDef` needing two handler variants.

**How the session/`clientInfo` half actually works** — the entry's "store `clientInfo` from
`initialize`" line needed an actual session mechanism, since `McpServer`/`execute()` were (and
still are, otherwise) fully stateless with nothing to key storage on across two separate requests:

1. `handle_request_with_context` mints a session id (`crate::request_id::generate_request_id()` —
   reusing the existing splitmix64 ID generator rather than inventing a new one) on every
   successful `initialize`, records that call's `params.clientInfo` under it in a new
   `sessions: Arc<Mutex<HashMap<String, StoredClientInfo>>>` field on `McpServer` (an `Arc` so
   every `Clone` of the server shares the same map), and returns the id via an `Mcp-Session-Id`
   response header — this is the actual MCP Streamable HTTP transport's session mechanism, not a
   bespoke one.
2. The client is expected to echo that header back on later requests. `execute()` (which has the
   `Request` this whole feature needs headers from) reads `Mcp-Session-Id`, looks up the recorded
   `clientInfo`, and builds the `McpContext` `do_tools_call` passes to a `tool_with_context` handler.
3. `handle_request(body)` (used directly in the ~50 existing tests that bypass the HTTP layer) still
   works unchanged — it delegates to a new `pub fn handle_request_with_context(body, ctx)` with
   `McpContext::default()`, so `tool_with_context` handlers just see an empty context in that path
   rather than every one of those tests needing rewriting to construct a `Request`.

**Known limitation, called out in the code and docs rather than silently shipped**: the session map
has no eviction — nothing removes an entry, since the MCP Streamable HTTP transport has no
session-termination signal to key cleanup off of. Acceptable for the expected usage (a modest,
roughly-stable set of long-lived AI-agent clients); not recommended as-is for a public-internet-
facing server churning through unbounded distinct clients.

`auth_claims` stays `None` always, as this entry's own comment anticipated (`// JSON string of
verified JWT claims (TODO-11)`) — no JWT verification exists in this module yet.

Scoped to tools only, matching this entry's own text — `.resource()`/`.prompt()` handlers have the
identical "no context" limitation but are out of scope here (not mentioned in the original ask).

8 new tests in `src/mcp/tests.rs`: `initialize` returns a non-empty `Mcp-Session-Id` header, two
`initialize` calls mint different session ids, a `tool_with_context` handler sees an empty context
via plain `handle_request`, the full real flow (`initialize` via `execute()`/`TestClient` → read the
session header → `tools/call` with that header → handler sees the recorded `clientInfo` and session
id), an unrecognized session id gets an empty `clientInfo` but the session id is still visible on
the context, and a regression guard that a plain `.tool()` still works unaffected by all of this.

**Effort:** small — new struct, new builder variant, plumb through `do_tools_call`. (The session
mechanism ended up being most of the actual diff, but the entry's stated effort was still roughly
right — no new dependency, no async, no protocol extension beyond the one header MCP already
defines for this purpose.)

---

### ✅ TODO-3: Tool annotations (MCP 2025-03-26) — Done (v17.77.0)

Added `ToolAnnotations` (exactly the four `Option<bool>` fields this entry specified:
`read_only_hint`, `destructive_hint`, `idempotent_hint`, `open_world_hint`) plus a private
`to_json()` that renders only the `Some` fields, using the spec's camelCase key names
(`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`), joined into a `{...}`
object — `"{}"` if every field is `None`.

`ToolDef` gained a fifth field, `annotations: Option<ToolAnnotations>`. Both existing builders
(`.tool()`, `.tool_with_context()`) set it to `None` — a plain-registered tool still has no
`annotations` key in `tools/list` at all, not an empty object. New builder:

```rust
.tool_annotated(name, desc, schema, annotations, handler) // handler: Fn(&str) -> Result<McpContent, String>
```

`do_tools_list` conditionally appends `,"annotations":{...}` to each tool's JSON only when
`t.annotations` is `Some` — exactly the entry's own sketch.

**Scope decision, called out explicitly rather than silently expanded:** `.tool_annotated()`'s
handler is the plain `Fn(&str) -> ...` shape, matching `.tool()`, not the `Fn(McpContext, &str) -> ...`
shape `.tool_with_context()` (TODO-2) added. There is no single builder combining annotations with
per-request context — call `.tool_with_context()` instead if you need `McpContext` and don't need
annotations. Same kind of explicit, honest limitation as TODO-2's resources/prompts context gap.

4 new tests in `src/mcp/tests.rs`: a `.tool_annotated()` tool's `tools/list` entry contains the
correct camelCase keys/values for a partial hint set (one `None` field correctly omitted from the
JSON), `ToolAnnotations::default()` (all `None`) still emits `"annotations":{}` (the key is present
because `Some(annotations)` was passed to the builder, even though every hint inside is unset), and
a regression guard that existing plain `.tool()`-registered tools have no `annotations` key at all.

**Effort:** tiny, as estimated — two struct additions, one new builder, one conditional JSON block.

---

### ✅ TODO-4: `image` and `embedded_resource` content types — Done (v17.78.0)

Added `McpContent::image(data, mime_type)` and `McpContent::embedded(uri, text, mime_type)`,
matching this entry's sketch (both constructors are generic over `impl Into<String>` for every
string arg, not just the `data`/`text` positions, for consistency with `::text`/`::json`).

`McpContent` gained a fifth field, `uri: Option<String>` (only set — and only serialized — for the
`"resource"` kind); `kind` now takes one of `"text"`, `"image"`, `"resource"`. `to_content_json()`
branches on `kind`: `"image"` renders `{"type":"image","data":"...","mimeType":"..."}`, `"resource"`
renders `{"type":"resource","resource":{"uri":"...","mimeType":"...","text":"..."}}`, and everything
else (i.e. `"text"`) keeps the original `{"type":"text","text":"..."}` shape. Both new variants flow
through the same `to_content_json()` call site already used by `tools/call` results and
`prompts/get` messages, so no dispatch code needed touching.

**Scope note:** `resources/read`'s response format wasn't touched — it already builds its own
fixed `{"contents":[{"uri":...,"mimeType":...,"text":...}]}` shape by hand rather than going through
`to_content_json()`, so a resource handler still can't return image content from `resources/read`
directly. Out of scope here since the entry only asked about tool-response content types.

This crate has no third-party dependencies (no base64 crate), so `McpContent::image` takes an
already-base64-encoded string rather than encoding raw bytes itself — documented on the constructor
and in DEVELOPER.md/docs rather than silently expecting callers to guess.

2 new tests in `src/mcp/tests.rs`: a tool returning `McpContent::image(...)` serializes `type`,
`data`, and `mimeType` correctly in a `tools/call` response (and omits the `text` field entirely);
a tool returning `McpContent::embedded(...)` serializes `type`, `uri`, `mimeType`, and `text`
correctly.

**Effort:** small, as estimated — one new field, two constructors, one branch in `to_content_json`.

---

### ✅ TODO-5: JSON-RPC batch requests — Done (v17.79.0)

`handle_request_with_context` now checks `body.trim_start().starts_with('[')` before doing
anything else and, if so, hands off to a new `handle_batch`, exactly this entry's own sketch.

**Splitting the array** needed one new hand-rolled parser, since this crate has no JSON
library: `json_rpc::split_array_elements(json: &str) -> Vec<String>` walks the array tracking
brace/bracket depth and string content (reusing the same escape/quote-tracking approach as the
existing `bracket_extract`), splitting on top-level commas only — a comma inside a nested
`params` object or inside a quoted string doesn't split the array in the wrong place.

**Dispatch table de-duplicated rather than copy-pasted**: the entry's own sketch implied
`handle_batch` would need the same `match method.as_str() { ... }` block that
`handle_request_with_context` already had. Instead of duplicating it, that block moved into a
new private `fn dispatch(&self, method: &str, body: &str, ctx: McpContext) -> Result<String,
(i32, String)>` called by both; likewise the `{"jsonrpc":"2.0","result":...}`/`error` rendering
moved into `fn format_result(id_str, &result) -> String`, also shared. Neither
`handle_request_with_context`'s nor `handle_batch`'s externally-visible behavior changed as a
result — this was a pure extract-method refactor alongside the new feature.

**Edge cases handled, matching JSON-RPC 2.0's own spec examples**, not just this entry's happy
path:
- Notifications (no `id`) in a batch contribute no entry to the response array — same as this
  entry said.
- A batch consisting *entirely* of notifications returns `202 Accepted` with no body, matching
  what a single standalone notification gets (not an empty `[]`, which nothing in JSON-RPC 2.0
  asks for and no client expects).
- An empty array (`[]`) is itself an invalid request per the JSON-RPC 2.0 spec's own test
  vectors — returns one `{"error":{"code":-32600,...}}` object, not `[]`.
- A successful `initialize` inside a batch still mints a session and attaches
  `Mcp-Session-Id` to the overall response, via the existing `start_session` — only the *first*
  `initialize` in a batch is honored this way, since one HTTP response carries exactly one
  session id and sending multiple `initialize`s in one batch has no sensible session semantics
  anyway. Not something the entry's text anticipated, but left silently unhandled would have
  meant batched `initialize` silently failing to establish a session at all.

10 new tests: 4 unit tests for `split_array_elements` (simple split, commas inside nested
objects/strings correctly ignored, empty array, single element) in `src/mcp/tests.rs`'s
`json_rpc` section, plus 6 `handle_batch` behavior tests (mixed-method batch dispatches and
wraps correctly, per-element success/error preserved independently, notifications omitted from
the response array, all-notification batch returns 202 with no body, empty array returns one
Invalid Request error, `initialize` inside a batch still sets `Mcp-Session-Id`).

**Effort:** small, as estimated — one branch, one array-splitting helper, and an extract-method
refactor of the existing dispatch table rather than a duplicate copy of it.

---

### TODO-6: Pagination for list methods

`tools/list`, `resources/list`, `prompts/list` return all items unconditionally. The spec
supports cursor-based pagination via `params.cursor` → response `nextCursor`.

For servers with many tools or resources, clients must page. Claude Desktop already sends
`cursor` on subsequent list calls.

**Add to `McpServer`:**
```rust
pub fn page_size(mut self, n: usize) -> Self { ... }
```

**In `do_tools_list`:**
- Read `params.cursor` (an opaque base64 offset)
- Slice `self.tools[offset..offset+page_size]`
- If more items remain, set `"nextCursor": "<offset+page_size as base64>"`

**Effort:** small — cursor = base64(usize offset), three list handlers updated.

---

## Priority 2 — Spec completeness (medium effort)

### TODO-7: SSE streaming transport (`GET /mcp`)

The MCP Streamable HTTP spec defines a second transport on the same path:
- `POST /mcp` — client → server requests (implemented)
- `GET /mcp` — server → client SSE stream for push notifications (**missing**)

Without the GET SSE channel, **all three** `listChanged` / `subscribe` capabilities must
remain `false`. This blocks:
- `notifications/tools/list_changed` (TODO-9)
- `notifications/resources/updated` + `notifications/resources/list_changed`
- `notifications/progress` for long-running tools (TODO-10)
- `notifications/message` log stream (TODO-8)

**Design:**
```rust
// Internal broadcast bus
type SseSender = std::sync::mpsc::SyncSender<String>;

struct McpServer {
    // ... existing fields ...
    sse_clients: Arc<Mutex<Vec<SseSender>>>,
}
```

In `execute()`, when `request.method == "GET"` and path matches:
1. Create a `(tx, rx)` pair.
2. Push `tx` into `sse_clients`.
3. Return a streaming SSE response that reads from `rx` until the channel closes.

Push a notification from anywhere:
```rust
fn notify_all(&self, event: &str) {
    let json = format!(r#"{{"jsonrpc":"2.0","method":"{}"}}"#, event);
    let mut clients = self.sse_clients.lock().unwrap();
    clients.retain(|tx| tx.send(format!("data: {json}\n\n")).is_ok());
}
```

**Leverage point:** `src/sse/mod.rs` already produces correct `text/event-stream` headers
and framing. The internal plumbing here uses a raw MPSC channel rather than the `Sse` builder
(which assembles a finished response body) — a lightweight equivalent that writes frames
incrementally is needed. This is the largest single item in this list.

**Effort:** medium — new GET handler, MPSC broadcast bus, keep-alive heartbeat thread.

---

### TODO-8: `logging/setLevel` and `notifications/message`

The spec lets clients request a minimum log level and receive server log messages as SSE
notifications. Useful during development: the MCP client shows server diagnostics inline.

**Depends on:** TODO-7 (SSE channel for push).

**New method in `handle_request`:**
```
"logging/setLevel" => self.do_set_log_level(body)
```

Store `min_level: Arc<Mutex<LogLevel>>`. When the server calls `mcp_log!(server, "info", "msg")`,
check the level, format as `notifications/message`, and push over the SSE channel.

**Builder:**
```rust
let server = McpServer::new(...)
    .logging_enabled()  // advertises logging capability in initialize
```

**Effort:** small once TODO-7 is done.

---

### TODO-9: Dynamic tool/resource/prompt registration + `listChanged`

Currently tools are in `Arc<Vec<ToolDef>>` — immutable after build. There is no way to add or
remove a tool at runtime (e.g. after discovering a plugin, connecting to a DB, or hot-reloading
config).

**Change storage to `Arc<RwLock<Vec<ToolDef>>>`.**

**Add a `McpHandle` returned from `.build()` (or exposed via `.handle()`):**
```rust
let (server, handle) = McpServer::new(...).tool(...).build_with_handle();

// Later, from any thread:
handle.register_tool("new_tool", desc, schema, handler);
handle.remove_tool("old_tool");
// Automatically pushes notifications/tools/list_changed over SSE (needs TODO-7)
```

Update `do_initialize` capabilities:
```json
{"tools":{"listChanged":true},"resources":{"listChanged":true,"subscribe":true},...}
```

**Effort:** medium — `RwLock` swap, `McpHandle` type, `listChanged` notification dispatch.

---

### TODO-10: `notifications/progress` for long-running tools

When a client includes `_meta.progressToken` in a `tools/call` request, the server should
send periodic `notifications/progress` events over the SSE channel as the tool runs.

**Depends on:** TODO-7 (SSE channel).

**In `do_tools_call`:**
```rust
let progress_token = json_rpc::extract_str(&params, "_meta.progressToken");
// pass to handler via McpContext (TODO-2)
```

**In tool handlers:**
```rust
|ctx: McpContext, args: &str| {
    if let Some(token) = &ctx.progress_token {
        ctx.report_progress(token, 0.0, 100.0, Some("starting".into()));
    }
    // ... do work ...
    Ok(McpContent::text("done"))
}
```

**Effort:** small once TODO-7 is done (TODO-2 already is).

---

### TODO-11: `completions/complete` — argument autocompletion

Clients like Cursor and VS Code use `completions/complete` to offer autocomplete when the user
fills in tool or prompt arguments. Without it, argument fields are plain text boxes.

**New builder method:**
```rust
.completion("tool", "tool_name", |arg_name, partial| {
    match arg_name {
        "region" => Ok(vec!["us-east-1", "eu-west-1", "ap-southeast-1"]),
        _        => Ok(vec![]),
    }
})
```

**Dispatch:**
```
"completion/complete" => self.do_completion(body)
```

The response format:
```json
{"completion":{"values":["us-east-1","eu-west-1"],"hasMore":false,"total":2}}
```

**Effort:** small — one new handler, one new builder method, one new internal vec.

---

### TODO-12: Request cancellation (`notifications/cancelled`)

The spec allows a client to send `notifications/cancelled` to abort a long-running `tools/call`.
Currently the server never reads this — the tool runs to completion regardless.

**For `http1` builds (synchronous):** not fixable without thread interruption; log and ignore.

**For `http2` async builds:**
```rust
// In McpContext (TODO-2):
pub cancellation: CancellationToken,  // tokio_util::sync::CancellationToken
```

The `notifications/cancelled` handler finds the in-flight request by `id` and calls
`token.cancel()`. Async tool handlers check `token.is_cancelled()` between steps.

**Effort:** medium (async only, requires CancellationToken tracking map).

---

## Priority 3 — Enterprise / advanced (lower urgency)

### TODO-13: OAuth 2.0 Authorization (MCP 2025-03-26)

The 2025-03-26 spec defines an OAuth 2.0 authorization flow: the server exposes
`/.well-known/oauth-authorization-server`, requires Bearer tokens from an authorization server,
and supports PKCE. This enables multi-tenant or enterprise deployments where each user
authenticates independently.

**Leverage point:** `sso::JwksCache` already does RS256/ES256 JWT verification. A new builder:
```rust
.require_oauth(
    jwks_url:  "https://accounts.google.com/.well-known/openid-configuration",
    audience:  "my-mcp-client-id",
)
```

In `execute()`: extract Bearer token → verify with `JwksCache` → inject claims into `McpContext`
(TODO-2) as `auth_claims`. Return `401` with `WWW-Authenticate: Bearer` on failure.

Also serve `GET /.well-known/oauth-authorization-server` with the metadata document.

**Effort:** small — `JwksCache` already does the hard work.

---

### TODO-14: `resources/subscribe` and `resources/unsubscribe`

Clients can subscribe to a specific resource URI and receive `notifications/resources/updated`
when it changes. Used for live-updating resource panels in Claude Desktop.

**Depends on:** TODO-7 (SSE channel) and TODO-9 (dynamic registration).

**Add subscription store:**
```rust
subscriptions: Arc<RwLock<HashMap<String, Vec<SseSender>>>>
// key: resource URI, value: list of SSE channels subscribed to it
```

**New methods in `handle_request`:**
```
"resources/subscribe"   => self.do_resource_subscribe(body, session_id)
"resources/unsubscribe" => self.do_resource_unsubscribe(body, session_id)
```

**API for resource owners to signal changes:**
```rust
handle.notify_resource_updated("config://main");
```

**Effort:** medium (depends on TODO-7 and TODO-9).

---

### TODO-15: `sampling/createMessage` — server-side sampling

The spec allows the MCP server to ask the client to run inference ("sampling"). This reverses
the normal flow: the server sends a `sampling/createMessage` request over the SSE channel and
the client responds via POST.

```rust
let response = server_handle.sample(SamplingRequest {
    messages: vec![SamplingMessage::user("What is 2+2?")],
    max_tokens: 100,
    model_preferences: None,
}).await?;
```

This is the most unusual MCP feature — only needed for agent-to-agent or meta-agent patterns.
**Depends on:** TODO-7 and async execution.

**Effort:** large (bidirectional request/response over SSE).

---

### TODO-16: `roots/list` and `notifications/roots/list_changed`

Clients can advertise filesystem roots to the server so the server knows which directories the
client has access to. Useful for file-system-aware tools that should only operate within the
client's workspace.

**Store received roots in `McpContext` (TODO-2):**
```rust
pub roots: Vec<McpRoot>,  // { uri: String, name: Option<String> }
```

**On `notifications/roots/list_changed`:** re-request `roots/list` from the client (requires
sampling-style bidirectional call — needs TODO-15).

**Effort:** medium (depends on TODO-15 for full implementation; partial read from context is small).

---

### TODO-17: Async tool handlers (`http2` feature)

Tool handlers are `Box<dyn Fn(&str) -> Result<McpContent, String> + Send + Sync>` — synchronous.
A tool that calls `AsyncClient`, queries a database, or waits for an AI response blocks a tokio
worker thread.

**New builder variant (gated on `#[cfg(feature = "http2")]`):**
```rust
.async_tool("call_api", desc, schema,
    |args: &str| async move {
        let resp = AsyncClient::new().get("https://api.example.com").send().await?;
        Ok(McpContent::json(resp.text()?))
    }
)
```

In `execute()` the async variant uses `tokio::task::block_in_place` to bridge the sync
`Application::execute` boundary (same pattern as `H2ReverseProxy::handle`).

**Effort:** medium — new `AsyncToolDef` storage, `block_in_place` bridge.

---

## Implementation order

```
Phase 1 — Quick wins (no new dependencies, mostly additive)
  TODO-1  protocol version negotiation     (tiny)              ✅ done (v17.75.0)
  TODO-2  McpContext in tool handlers      (small)              ✅ done (v17.76.0)
  TODO-3  tool annotations 2025-03-26      (tiny)              ✅ done (v17.77.0)
  TODO-4  image + embedded content types   (small)              ✅ done (v17.78.0)
  TODO-5  JSON-RPC batch requests          (small)              ✅ done (v17.79.0)
  TODO-6  list pagination                  (small)
  TODO-11 completions/complete             (small)

Phase 2 — Streaming foundation (enables all notification features)
  TODO-7  GET /mcp SSE channel            (medium — unblocks 8, 9, 10, 14, 15, 16)
  TODO-8  logging/setLevel + notifications (small, needs TODO-7)
  TODO-9  dynamic registration             (medium, needs TODO-7)
  TODO-10 notifications/progress           (small, needs TODO-7 + TODO-2)

Phase 3 — Enterprise + advanced
  TODO-11 completions/complete            (small, can go in Phase 1)
  TODO-12 request cancellation            (medium, http2 only)
  TODO-13 OAuth 2.0 (2025-03-26)         (small — JwksCache already exists)
  TODO-14 resources/subscribe             (medium, needs TODO-7 + TODO-9)
  TODO-17 async tool handlers             (medium, http2 only)
  TODO-15 sampling/createMessage          (large)
  TODO-16 roots/list                      (medium, needs TODO-15)
```

---

## Summary table

| # | Enhancement | Spec | Priority | Effort | Dependency |
|---|-------------|------|----------|--------|------------|
| 1 | Protocol version negotiation | 2024-11-05 | **P1** | Tiny | ✅ Done (v17.75.0) |
| 2 | `McpContext` in tool handlers | Ergonomics | **P1** | Small | ✅ Done (v17.76.0) |
| 3 | Tool annotations | 2025-03-26 | **P1** | Tiny | ✅ Done (v17.77.0) |
| 4 | `image` + `embedded` content | 2024-11-05 | **P1** | Small | ✅ Done (v17.78.0) |
| 5 | JSON-RPC batch | JSON-RPC 2.0 | **P1** | Small | ✅ Done (v17.79.0) |
| 6 | List pagination | 2024-11-05 | **P1** | Small | — |
| 11 | `completions/complete` | 2024-11-05 | **P1** | Small | — |
| 7 | SSE transport (`GET /mcp`) | Streamable HTTP | **P2** | Medium | — |
| 8 | `logging/setLevel` | 2024-11-05 | **P2** | Small | #7 |
| 9 | Dynamic registration + `listChanged` | 2024-11-05 | **P2** | Medium | #7 |
| 10 | `notifications/progress` | 2024-11-05 | **P2** | Small | #7 + #2 |
| 12 | Request cancellation | 2024-11-05 | **P3** | Medium | `http2` async |
| 13 | OAuth 2.0 auth | 2025-03-26 | **P3** | Small | `sso` feature |
| 14 | `resources/subscribe` | 2024-11-05 | **P3** | Medium | #7 + #9 |
| 17 | Async tool handlers | Ergonomics | **P3** | Medium | `http2` feature |
| 15 | `sampling/createMessage` | 2024-11-05 | **P3** | Large | #7 |
| 16 | `roots/list` | 2024-11-05 | **P3** | Medium | #15 |
