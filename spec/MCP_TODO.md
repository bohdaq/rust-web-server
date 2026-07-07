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

### ✅ TODO-6: Pagination for list methods — Done (v17.80.0)

Added `McpServer::page_size(n)` (clamps `n` to a minimum of `1`) storing `page_size: Option<usize>`
on the server — `None` by default, meaning every list method returns every item in one response
and never emits `nextCursor`, exactly the behavior before this existed.

**Cursor implementation**, matching this entry's own sketch of "opaque base64 offset": since this
crate has no base64 dependency (any feature), added small private `base64_encode`/`base64_decode`
free functions in `src/mcp/mod.rs` (RFC 4648 standard alphabet, `=` padding) plus `encode_cursor`/
`decode_cursor` wrappers that base64-encode/decode the offset's decimal string. This duplicates the
shape of `websocket::base64_encode` (used for `Sec-WebSocket-Accept`) rather than sharing it —
consistent with this codebase's existing pattern of each module keeping its own small, focused
encoding helpers (`webhook`, `auth`, `storage::azure_signature`, `acme::crypto` each do the same)
rather than introducing a shared crate-wide base64 module for a handful of call sites.

**Shared `paginate` helper**, not a copy-pasted slice-and-cursor block in three places: `do_tools_list`,
`do_resources_list`, `do_prompts_list` each render their full `Vec<String>` of item JSON blobs as
before, then call one `fn paginate(&self, items: &[String], body: &str) -> Result<(&[String],
Option<String>), (i32,String)>` that reads `params.cursor` (if `page_size` is set), decodes it to an
offset, slices, and returns the page plus an optional `nextCursor`. A shared `next_cursor_json()`
renders the `,"nextCursor":"..."` suffix (or `""`) spliced after each response's closing `]`.

**Edge cases**, beyond the entry's happy-path sketch:
- An invalid/tampered cursor (not valid base64, or valid base64 that isn't a decimal `usize`)
  returns a JSON-RPC `INVALID_PARAMS` (`-32602`) error rather than silently falling back to offset
  `0` — a client debugging its own cursor-handling bug gets a clear signal instead of a confusing
  restart-from-page-1.
- An offset at or past the end of the list returns an empty page with no `nextCursor`, not an
  error — the well-defined "you've reached the end" case, distinct from a malformed cursor.

11 new tests in `src/mcp/tests.rs`: `encode_cursor`/`decode_cursor` round-trip for several offsets
including `0` and `usize::MAX`, `decode_cursor` rejecting garbage input, first-page/second-page/
invalid-cursor/past-the-end behavior for `tools/list` against a 3-tool `page_size(2)` server, a
regression guard that `tools/list` stays fully unpaginated (no `nextCursor`) when `page_size` isn't
set, and one pagination test each for `resources/list` and `prompts/list`.

**Effort:** small, as estimated — one field, one builder, one base64 helper pair, one shared
pagination helper applied to three list handlers.

---

## Priority 2 — Spec completeness (medium effort)

### ✅ TODO-7: SSE streaming transport (`GET /mcp`) — Done (v17.81.0)

`GET /mcp` now returns a `text/event-stream` response that stays open indefinitely, and
`McpServer::notify(method, params_json)` broadcasts a JSON-RPC notification (no `id`, per spec —
fire-and-forget) to every connected client, framed as an SSE `data:` event.

**Actual leverage point turned out better than the entry's own sketch anticipated**: the entry
proposed a bespoke "streaming SSE response that reads from `rx`," implying new response-writing
machinery. That machinery already existed — `Response::stream_pipe: Option<Box<dyn Read + Send>>`,
added for reverse-proxy passthrough streaming, and `Server::pipe_stream` (unmodified by this work)
already reads from any `Read` source and forwards chunks with `Transfer-Encoding: chunked`, flushing
each one immediately. So instead of new server-side write-loop code, this only needed a `Read`
adapter over the channel: `SseChannelReader` wraps an `mpsc::Receiver<Vec<u8>>` and blocks in
`read()` until either a frame arrives, the sender side disconnects (clean EOF, `Ok(0)`), or
`SSE_KEEPALIVE_INTERVAL` (15s) elapses with nothing to send (writes a `: keep-alive` comment
instead). `GET /mcp` creates an `mpsc::sync_channel(32)` pair, stores the sender in a new
`sse_clients: Arc<Mutex<Vec<SyncSender<Vec<u8>>>>>` field, and returns a `Response` with
`stream_pipe` set to a boxed `SseChannelReader` over the receiver — matching this entry's own
"Leverage point" note almost exactly, just one layer lower (a `Read` impl, not a new response kind).

**Deliberate deviation from the sketch's `notify_all`:** the sketch's `tx.send(...)` on a
`SyncSender` blocks the calling thread if that one client's bounded buffer is full — meaning a
single slow SSE reader could stall every future `notify()` call from any thread. Implemented with
`try_send` instead (never blocks); a client whose buffer is full is retained/dropped by the exact
same `Vec::retain` sweep as a genuinely disconnected one — indistinguishable from the caller's
perspective, and consistent with "one bad client can't affect anyone else."

**No separate "keep-alive heartbeat thread"** as the entry's effort estimate assumed: folding the
keep-alive into `SseChannelReader::read`'s `recv_timeout` achieves the same effect (periodic writes
to idle connections) without spawning and managing an extra thread per server instance.

**Scope, stated plainly:** this only wires up the transport itself — the channel, the `GET`
endpoint, and the generic `.notify()` broadcast primitive other TODOs will build on
(`notifications/tools/list_changed` for TODO-9, `notifications/message` for TODO-8,
`notifications/progress` for TODO-10, etc. all still need their own triggering logic, not
implemented here). Also scoped to the plain HTTP/1.1 path only, matching `Response::stream_pipe`'s
existing scope — `h2_handler`/`h3_handler` don't drive `stream_pipe` for *any* response yet, a
pre-existing limitation this work didn't touch. Dead `sse_clients` entries (client disconnected, but
`notify()` never called since) are only pruned lazily on the next `notify()`, not proactively — the
same kind of "no eviction without a triggering event" tradeoff already documented for the session
map in TODO-2.

**Verified against a real socket, not just unit tests:** `TestClient` bypasses `Server::pipe_stream`
entirely (it inspects the returned `Response` directly), so it can't exercise the actual streaming
write loop. Beyond the unit tests below, this was manually verified end-to-end with a real running
server and `curl -N`: a live SSE connection received periodic `.notify()` pushes as they were sent,
two concurrent connections both received the same broadcast, and response headers
(`Content-Type: text/event-stream`, chunked transfer encoding) were confirmed on the wire.

12 new tests in `src/mcp/tests.rs`: `GET /mcp` via `Application`/`TestClient` returns `200` with
`Content-Type: text/event-stream` (superseding the old `application_returns_405_for_get_on_mcp_path`
test, renamed/repurposed since `GET` is no longer a 405); a new regression test that `DELETE /mcp`
(an actually-unsupported method) still gets `405`; the bearer-auth guard covers `GET` too; and
(calling the private `start_sse_stream`/`notify` directly, reading from the `stream_pipe` reader
in-process) headers/reader presence, a delivered frame's `method`+`params` shape, `params` omitted
when not given, broadcast to multiple simultaneous clients, a full-buffer client getting dropped,
and a disconnected client getting pruned on the next `notify()`.

**Effort:** medium, as estimated — though the actual work skewed toward "adapt an existing
mechanism" rather than "build new streaming infrastructure," since `stream_pipe` already did the
hard part.

---

### ✅ TODO-8: `logging/setLevel` and `notifications/message` — Done (v17.82.0)

Added `LogLevel` (the spec's eight RFC 5424 severities: `Debug`, `Info`, `Notice`, `Warning`,
`Error`, `Critical`, `Alert`, `Emergency`), `handle_request`/`dispatch`'s new
`"logging/setLevel" => self.do_set_log_level(body)` arm exactly as sketched, `min_log_level:
Arc<Mutex<LogLevel>>` on `McpServer` (matching the entry's own field sketch, just without the
`Arc<Mutex<_>>` needing its own comment since the pattern is already established by `sessions` and
`sse_clients`), and `.logging_enabled()` exactly as sketched — an opt-in builder that adds
`"logging":{}` to `initialize`'s advertised `capabilities`.

**`LogLevel`'s ordering is free, not hand-rolled:** deriving `PartialOrd`/`Ord` on the enum gives
correct severity comparisons (`Debug < Info < ... < Emergency`) directly from declaration order —
no manual rank/priority numbers to keep in sync with the variant list.

**No `mcp_log!` macro** — the entry's own text used one as a hypothetical example
(`mcp_log!(server, "info", "msg")`), but a `macro_rules!` adds surface area (import path,
crate-level `#[macro_export]` visibility rules) for no real benefit over a plain method that's
just as terse: `server.log(LogLevel::Info, Some("logger-name"), r#""msg""#)`. Every other MCP
feature so far (`.tool()`, `.notify()`, `.tool_annotated()`, ...) is a builder/method, not a macro,
so `.log()` matches the rest of the API's shape rather than introducing the crate's first macro
for this one case.

**Filtering reuses `.notify()` rather than duplicating the broadcast logic:** `.log()` builds the
`notifications/message` params JSON (`{"level":"...","logger":"...","data":...}`, `logger` omitted
when not given) and — only if `level >= *min_log_level.lock().unwrap()` — calls
`self.notify("notifications/message", Some(&params))`. This means `.log()` automatically inherits
every property `.notify()` (TODO-7) already has: never blocks the calling thread, drops a client
whose buffer fills up, HTTP/1.1-only scope. No separate code path to keep in sync.

**`.logging_enabled()` only changes what's advertised, not what works:** `.log()` and
`logging/setLevel` both function whether or not `.logging_enabled()` was ever called — this entry's
own text frames it as "advertises ... capability," not "enables," and treating it as a hard gate
would mean a server that forgot to call the builder couldn't be debugged via a manual `.log()` call
even though nothing else needs it. A spec-honest client just wouldn't send `logging/setLevel` in
the first place without seeing the capability, so pairing the two remains the expected usage without
requiring it in code.

**Default minimum level is `LogLevel::Debug`** (the least restrictive) rather than something more
conservative like `Info` or `Warning` — chosen so nothing is silently dropped unless a client
explicitly asks for less noise via `logging/setLevel`; a server that never receives that call
behaves as if every `.log()` call is delivered.

15 new tests in `src/mcp/tests.rs`: `LogLevel::parse`/`as_str` round-trip for all 8 levels,
rejecting unrecognized/wrong-case strings, and the full `Debug < ... < Emergency` ordering chain;
`initialize` omits `"logging"` by default and includes `"logging":{}` after `.logging_enabled()`;
`logging/setLevel` succeeds for a valid level and returns `INVALID_PARAMS` for a missing or
unrecognized one; and (reading from `start_sse_stream()`'s `stream_pipe`, same pattern as TODO-7's
tests) `.log()` delivers the correct `notifications/message` shape with `level`/`logger`/`data`,
omits `logger` when not given, is delivered by default at every level before any `setLevel` call,
and — the key regression guard — a message below a client-set minimum level is never queued at all
(proven by sending a filtered call followed by an allowed one and confirming only the allowed one is
read back, rather than just checking a boolean flag).

**Effort:** small, as estimated, now that TODO-7 exists — one enum, one field, one dispatch arm, one
builder, and one method that's mostly a thin filter in front of the already-built `.notify()`.

---

### ✅ TODO-9: Dynamic tool/resource/prompt registration + `listChanged` — Done (v17.83.0)

Changed `tools`/`resources`/`prompts` storage from a plain `Vec<T>` (the entry's premise said
`Arc<Vec<ToolDef>>`, but it was actually an un-shared plain `Vec` before this — either way,
immutable after construction) to `Arc<RwLock<Vec<T>>>` exactly as sketched, for all three
collections, not just tools.

**No separate `McpHandle` type** — the entry's own sketch (`build_with_handle()` returning a
`(server, handle)` pair) would introduce a second public type whose only job is holding the same
`Arc<RwLock<_>>>` fields `McpServer` already has. Since `McpServer` is `#[derive(Clone)]` and every
clone now shares the same underlying tools/resources/prompts storage (same pattern already
established by `sessions` and `sse_clients`), a clone of the server *is* a handle — there's nothing
a separate `McpHandle` would add. Implemented `register_tool`/`remove_tool`,
`register_resource`/`remove_resource`, and `register_prompt`/`remove_prompt` as plain `&self`
methods directly on `McpServer` instead:

```rust
server.register_tool("new_tool", desc, schema, handler);  // &self, not consuming
let existed: bool = server.remove_tool("old_tool");
```

Each pushes/removes exactly like the consuming `.tool()`/`.resource()`/`.prompt()` builders
internally (same `ToolDef`/`ResourceDef`/`PromptDef` construction), just through a `.write().unwrap()`
instead of a bare `.push()` on an owned `Vec`. `remove_*` uses `Vec::retain` and returns whether
anything was actually removed (`before.len() != after.len()`), so callers can distinguish "removed"
from "wasn't there."

**Handler invocation no longer holds the lock for the call's duration:** `do_tools_call`,
`do_resources_read`, and `do_prompts_get` used to `find()` inside a borrow of the `Vec` and call the
handler while still holding that borrow. With `RwLock`, doing the same would hold a read guard for
however long the handler takes to run — blocking any concurrent `register_*`/`remove_*` (which needs
the write lock) until the handler returns, including a slow one. Instead, each of those three now
clones the matched entry's `Arc<dyn Fn...>` handler (and, for prompts, the `description` string it
also needs) out from under a short-lived read guard, drops the guard, and only then calls the
handler — so a long-running tool call never stalls a registration change on another thread.

**`listChanged` notifications, matching TODO-7's `.notify()` exactly:** every successful
registration/removal pushes `notifications/tools/list_changed` (or the `resources`/`prompts`
equivalent), no `params` — per spec, these carry none. A `remove_*` call that finds nothing pushes
nothing, since nothing changed.

**Capabilities updated with one deliberate correction to the entry's own sketch:** the entry's JSON
example showed `"resources":{"listChanged":true,"subscribe":true}` — but `resources/subscribe`/
`resources/unsubscribe` (TODO-14) aren't implemented in this dispatch table. Advertising
`subscribe:true` would tell a client it can call a method that returns `METHOD_NOT_FOUND`. Set
`listChanged:true` for all three (tools, resources, prompts) as intended, but left
`resources.subscribe` at `false` until TODO-14 actually exists. Unlike the opt-in `.logging_enabled()`
(TODO-8), `listChanged:true` is unconditional — dynamic registration is always available on every
`McpServer`, nothing to opt into.

**Scope, stated plainly:** only the plain registration shapes have dynamic equivalents —
`register_tool`/`register_prompt` match `.tool()`/`.prompt()`, not `.tool_with_context()`,
`.tool_annotated()`, or `.prompt_with_args()`. Changing a dynamically-added tool's annotations or a
prompt's argument definitions means removing and re-registering under the same name.

10 new tests in `src/mcp/tests.rs`: `initialize` advertises `listChanged:true` for all three and
keeps `resources.subscribe:false`; `register_tool` makes a tool immediately callable via
`tools/call` and pushes `notifications/tools/list_changed` (asserting no `params` field);
`remove_tool` returns `true`/removes correctly and returns `false`/pushes nothing when the name
doesn't exist (proven the same way as TODO-8's level-filtering test — a marker notification sent
right after confirms nothing was queued by the no-op); matching register/remove tests for resources
and prompts; and a dedicated test proving registration through one `McpServer` clone is visible
through another — the actual point of the `Arc<RwLock<_>>>` change.

**Effort:** medium, as estimated — though skewed toward the `RwLock` migration and the
handler-invocation lock-scoping fix (both touching every existing list/call/read/get method) rather
than the registration methods themselves, which are mechanically similar to the existing builders.

---

### ✅ TODO-10: `notifications/progress` for long-running tools — Done (v17.84.0)

`do_tools_call` now extracts `params._meta.progressToken` and attaches it to the `McpContext` passed
to the handler; `McpContext::report_progress(progress, total, message)` pushes a
`notifications/progress` event over the `GET /mcp` SSE channel (TODO-7) for that token.

**Extraction wasn't the literal one-liner the entry sketched** — `json_rpc::extract_str(&params,
"_meta.progressToken")` isn't valid against this crate's hand-rolled JSON helpers, which only do
flat single-key lookups (no dotted-path support, and no JSON library to add one). Implemented as two
nested lookups instead: `json_rpc::extract_raw(&params, "_meta")` then `extract_raw(&meta,
"progressToken")` — using `extract_raw` rather than `extract_str` on the *token* deliberately, since
the spec allows `progressToken` to be a `string | number` and `extract_str` only handles quoted
string values. The raw JSON token (already correctly quoted if it's a string, or bare if a number)
is stored as-is in `McpContext::progress_token` and spliced back verbatim by `report_progress` — no
decode/re-encode round trip that could get one type right and the other wrong.

**`report_progress` doesn't take the token as a parameter**, unlike the entry's own sketched
signature (`ctx.report_progress(token, 0.0, 100.0, ...)`). The token is already sitting on `ctx`
(that's the whole point of routing it through `McpContext`) — requiring a handler to also pass it
back in on every call is redundant and a real footgun: nothing stops a handler from typoing or
copy-pasting the wrong token from a different call. Implemented signature: `ctx.report_progress(progress:
f64, total: Option<f64>, message: Option<&str>)` — reads `self.progress_token` internally and
silently no-ops if it's `None` (client didn't ask for updates), so a handler never needs to branch on
whether reporting is possible before calling it.

**`McpContext` gained a private `sse_clients: Option<Arc<Mutex<Vec<SyncSender<Vec<u8>>>>>>`** field
(not `pub` — it's plumbing, not context data a handler reads) alongside the new `pub progress_token:
Option<String>`. `context_for()` (called for every request, any method) now sets `sse_clients` to a
clone of the server's broadcast list unconditionally; `do_tools_call` is the only place that ever
sets `progress_token` to `Some`, since `_meta.progressToken` is specific to that one method. A
context built by hand (`McpContext { ..Default::default() }`, e.g. via
`handle_request_with_context` in a test) has `sse_clients: None`, so `report_progress` silently
no-ops there too — consistent with how `client_name`/`session_id` already behave empty in that path.

**Shared plumbing, not a duplicate broadcast path:** extracted `McpServer::notify`'s two responsibilities
into free functions — `render_notification(method, params_json) -> String` (the
`{"jsonrpc":"2.0","method":...,"params":...}` shape) and `broadcast_sse_to(clients: &Arc<Mutex<Vec<SseSender>>>,
json: &str)` (the `try_send`-and-prune loop, previously `McpServer::broadcast_sse`, now taking the
list explicitly instead of `&self`). `McpServer::notify` and `McpContext::report_progress` both call
these same two functions — `report_progress` couldn't call `.notify()` directly (that needs `&McpServer`,
which `McpContext` doesn't have and shouldn't need), but the actual rendering/broadcasting logic isn't
duplicated.

5 new tests: `report_progress` delivers two sequential progress frames with correct
`progressToken`/`progress`/`total`/`message` fields, in order; no frame is queued when the request
had no `progressToken` (proven the same marker-notification way as prior TODOs' "nothing was
queued" tests); `report_progress` is a safe no-op when called through `handle_request()`'s live-server-less
context even though the request itself included a `progressToken`; a numeric `progressToken` (not a
string) round-trips unquoted; `total`/`message` are omitted from the frame when not given.

**Effort:** small, as estimated, now that TODO-7 (SSE) and TODO-2 (`McpContext`) both exist — the
actual work was almost entirely in getting the nested-object extraction and the shared
render/broadcast refactor right, not new broadcast infrastructure.

---

### ✅ TODO-11: `completions/complete` — argument autocompletion — Done (v17.85.0)

Added `.completion(ref_type, ref_name, handler)` (a consuming builder, matching `.tool()`/
`.resource()`/`.prompt()`'s shape), a new `completions: Arc<RwLock<Vec<CompletionDef>>>` field, and
`dispatch`'s `"completion/complete" => self.do_completion(body)` arm — the real wire method name is
singular (`completion/complete`), matching the entry's own "Dispatch" code block exactly, even
though this entry's *heading* says the plural "completions/complete" (informal phrasing, not the
literal method).

**`ref_type` handles the mismatch between the entry's ergonomic builder signature and the wire
format**: `.completion("tool", "tool_name", ...)` takes the short form, but a real `completion/complete`
request's `ref.type` is `"ref/tool"`/`"ref/prompt"` (the actual MCP spec only defines `ref/prompt`
and `ref/resource`; `ref/tool` is this server's own extension, since tools are first-class here and
the entry's own example explicitly asks for tool completion). `do_completion` strips a leading
`"ref/"` from the incoming `ref.type` before matching against what was registered, so the builder
call stays exactly as terse as the entry's own example while still handling the real wire shape.

**No match returns empty values, not an error** — an unregistered `ref`/name combination, or an
argument name a handler doesn't recognize, gets back `{"values":[],"hasMore":false,"total":0}`
rather than `INVALID_PARAMS`. Completion is a best-effort UI hint per the spec, not a required
capability every tool/prompt/argument must support; treating "no completions configured for this"
as an error would make partial completion coverage across a server's tools impossible without
handlers having to explicitly enumerate every argument they don't want to complete.

**Response format extended slightly beyond the entry's own sketch**, matching the actual spec more
closely: a handler returning more than `MAX_COMPLETION_VALUES` (100, per the spec's guidance against
huge completion lists) has the response truncated to the first 100 with `hasMore:true` and the
untruncated `total` — the entry's sketch showed a fixed two-value example with `hasMore:false`
already, but didn't address what happens for a handler that returns many candidates.

**`completions` capability is auto-advertised, no separate opt-in flag** — unlike `.logging_enabled()`
(TODO-8), `initialize` checks `!self.completions.read().unwrap().is_empty()` at request time rather
than requiring a `.completions_enabled()` the entry didn't ask for and callers would have to
remember to pair with `.completion(...)`. A server with zero registered completions doesn't
advertise the capability; one with at least one always does.

**No dynamic (`&self`) equivalent** — unlike TODO-9's `register_tool`/`register_resource`/
`register_prompt`, completion providers are builder-only (registered before serving requests).
Out of scope here; nothing in this entry asked for it, and extending TODO-9's pattern to a fourth
collection wasn't requested.

11 new tests in `src/mcp/tests.rs`: matching values filtered by partial input, an omitted
`argument.value` defaulting to an empty partial, unregistered ref/argument-name each returning empty
values (not an error), a handler's `Err` mapping to `INVALID_PARAMS` with the handler's own message,
missing `ref`/`argument` each returning `INVALID_PARAMS`, `ref/prompt` support (not just
`ref/tool`), truncation to 100 values with correct `hasMore`/`total`, and `initialize` advertising
`"completions":{}` only once a completion is registered (absent by default).

**Effort:** small, as estimated — one builder, one dispatch arm, one handler, one new collection;
the `ref/` prefix handling and truncation were the only wrinkles beyond a direct implementation of
the entry's own sketch.

---

### ✅ TODO-12: Request cancellation (`notifications/cancelled`) — Done (v17.86.0)

Implemented as **cooperative cancellation via a plain `Arc<AtomicBool>` flag**, working uniformly on
both `http1` and `http2` builds — not the bifurcated design this entry sketched (sync builds "log
and ignore," async builds get a real `tokio_util::sync::CancellationToken`).

**Why the async-only half of the sketch wasn't built:** it depends on async tool handlers, which
don't exist in this crate yet (that's TODO-17, still open — every tool handler today, in every
build configuration, is a plain synchronous `Fn(...)`). Building `CancellationToken` plumbing for a
feature with zero consumers, and pulling in `tokio_util` as a new dependency to do it, would be
speculative work with nothing to actually exercise it. The entry's sync-side fallback ("not fixable
without thread interruption; log and ignore") undersold what's actually possible without async: a
synchronous handler that structures its own work as a loop (processing N items, say) can
voluntarily check a shared flag between iterations and return early — ordinary cooperative
cancellation, the same pattern `report_progress` (TODO-10) already established for progress
updates between a handler's own steps. That doesn't need `tokio_util`, async, or a bifurcated
implementation — so it's what got built, for every build configuration, instead of "log and ignore."

**Mechanics:** `McpServer` gained `cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>`,
keyed by a `tools/call` request's raw `id` JSON token (same "store the raw token, not a decoded
value" approach as `progress_token`/`requestId`, since ids can be `string | number`). A new private
`dispatch_with_cancellation` wraps `dispatch` in both `handle_request_with_context` and
`handle_batch`: for `method == "tools/call"` (the only method this applies to — the id is guaranteed
`Some` there, since a notification-shaped `tools/call` with no id never reaches dispatch at all) it
registers a fresh flag, attaches it to a modified `McpContext`, calls through to `dispatch`, then
removes the entry — regardless of whether the handler ever checked the flag. This map can never
accumulate stale entries the way `sessions`/`sse_clients` can, since every insert has a
matching remove on the same call stack.

`notifications/cancelled` is special-cased in both `handle_request_with_context` and
`handle_batch`, ahead of the generic "notification → 202, no processing" branch that would otherwise
silently swallow it (this notification carries no `id` of its own — it's fire-and-forget, referencing
a *different* request's id via `params.requestId`). `handle_cancellation` reads `requestId` (again as
a raw token) and flips the matching flag if the target request is still in flight; an unknown or
already-finished request id is silently ignored, not an error — the target call may simply have
completed naturally before the cancellation arrived.

**`McpContext::is_cancelled(&self) -> bool`** is the handler-facing surface: reads the attached flag
(private `cancellation: Option<Arc<AtomicBool>>` field, same "plumbing, not `pub`" treatment as
`sse_clients`), defaulting to `false` for anything other than a live `tools/call` context. Always
safe to call, matching the "never needs a capability check first" convention `report_progress` and
`notify` already established.

6 new tests in `src/mcp/tests.rs`: `is_cancelled()` defaults to `false` without any cancellation; a
handler observes `is_cancelled() == true` after a simulated mid-call cancellation (a single-threaded
test can't send a real concurrent notification, so the handler holds a clone of the server sharing
the same `cancellations` map and sends the cancellation to itself, targeting its own request id —
proving the actual registration/lookup/flip mechanism, not just the getter); a string request id
matches the same way a numeric one does; an unknown request id is a silent no-op; a completed call's
cancellation entry is removed (no leak); and a `notifications/cancelled` batch element produces no
response entry, like any other notification.

**Effort:** ended up smaller than the "medium" estimate, once scoped to what's actually buildable
today — no new dependency, no async, and the map-based flag-tracking mechanics turned out to be a
close structural match for `sessions`'s existing `Arc<Mutex<HashMap<...>>>` pattern.

---

## Priority 3 — Enterprise / advanced (lower urgency)

### ✅ TODO-13: OAuth 2.0 Authorization (MCP 2025-03-26) — Done (v17.98.0)

**The leverage point held up exactly as scoped** — `sso::jwks::JwksCache` (built for the `sso` feature's OIDC client support, long before this TODO was picked up) does all of the real work here; this TODO only needed to wire it into `McpServer`. `McpServer::require_oauth(provider: sso::OidcProvider, audience: impl Into<String>) -> Self` is the new builder, stored as `Option<Arc<OAuthConfig>>` (`OAuthConfig { jwks: JwksCache, provider: OidcProvider, audience: String }`), both gated `#[cfg(feature = "sso")]` end to end (field, type, builder, verification/metadata logic in `execute()`) — `mcp` itself has no Cargo feature of its own and is always compiled, so `.require_oauth()` only exists to callers who also enable `sso`.

**One parameter shape deviation from this entry's own sketch, and it's a correction, not a simplification:** the sketch's `.require_oauth(jwks_url: "...well-known/openid-configuration", audience: "...")` names its first parameter `jwks_url` but gives it a **discovery document URL** as an example value — those are two different things (`sso::discovery::OidcProvider::discover(issuer)` fetches the discovery document and extracts `jwks_uri` from it; `JwksCache::new(jwks_uri)` takes the JWKS endpoint directly, e.g. `https://www.googleapis.com/oauth2/v3/certs` for Google, not the discovery URL). Accepting a full `sso::OidcProvider` (a preset like `OidcProvider::google()`, or the result of a live `OidcProvider::discover(issuer)?` call) rather than a bare URL string resolves the ambiguity correctly, reuses this crate's own existing, already-tested discovery infrastructure instead of re-deriving a jwks_uri by hand, and — as a direct side effect — gives `GET /.well-known/oauth-authorization-server` a complete, accurate metadata document (issuer, both endpoints, jwks_uri) for free, rather than only ever knowing a bare jwks_uri and nothing else.

**`GET /.well-known/oauth-authorization-server`** is served automatically whenever `.require_oauth()` is configured, checked ahead of the main `self.path` (`/mcp`) match in `execute()` since it's a sibling top-level path, not nested under the MCP endpoint. It is **not a full RFC 8414 document** — this server is a resource server verifying tokens issued elsewhere, not the authorization server itself, so the document only ever carries what `OidcProvider` already knows (`issuer`, `authorization_endpoint`, `token_endpoint`, `jwks_uri`, `response_types_supported: ["code"]`). When OAuth isn't configured, this path isn't special-cased at all — it falls through to the fallback app exactly like any other non-MCP path.

**Claims injection matches the sketch exactly:** on a successful verify, `McpContext.auth_claims` (a field that already existed, `None` since `TODO-2`, explicitly reserved in its own doc comment for "TODO-11/TODO-13" — TODO-11 ended up not needing it, so this is the field's first real use) is set to the verified `sso::OidcClaims`, serialized to a JSON string via `serde_json::to_string`. Failure (missing bearer, bad signature, wrong audience/issuer, expired) returns `401` with `WWW-Authenticate: Bearer` — reusing the exact same `unauthorized()` helper `.require_bearer()` already used, so the failure shape is identical between the two mechanisms; only the verification logic feeding into it differs.

**Interaction with `.require_bearer()`, not addressed by the sketch at all:** if a server configures both (an unusual, unsupported combination — pick one), OAuth verification runs and the static token is never checked. This is an explicit tie-break, not an accident: `execute()` checks `self.oauth` first and only falls back to `self.auth_token` in the `else` branch.

**Tests:** 9 new tests in an `oauth_tests` submodule of `src/mcp/tests.rs` (`#[cfg(feature = "sso")]`), using a real RSA-2048 keypair, real signed JWTs, and a loopback fake JWKS server (the same techniques already established in `sso`'s own test suite, duplicated here in a self-contained form rather than shared across modules): a valid token succeeding; a missing bearer, a token signed by the wrong key, a wrong audience, and an expired token all returning `401`; the metadata endpoint returning the expected document when configured and falling through (not `200`) when it isn't; verified claims actually reaching a `.tool_with_context()` handler's `ctx.auth_claims`; and OAuth taking precedence over a simultaneously-configured `.require_bearer()`. All 9 passed on the first attempt. `cargo test --lib --features sso mcp::` runs 169 mcp tests (160 + 9 new); the default/http1/http2 three-way build is unaffected (the whole feature compiles out cleanly without `sso`).

**Effort:** small, exactly as estimated — `JwksCache` genuinely did all the hard verification work; the actual task was builder plumbing, one path-routing addition, and reusing `OidcProvider` instead of a bare URL.

---

### ✅ TODO-14: `resources/subscribe` and `resources/unsubscribe` — Done (v17.87.0)

Added `.notify_resource_updated(uri)` (a `&self` method directly on `McpServer`, not a separate
`handle.` type — same reasoning as TODO-9's `register_tool`/etc.: a cloned `McpServer` already
shares the storage a handle would need) and `dispatch`'s `"resources/subscribe"`/
`"resources/unsubscribe"` arms, exactly matching this entry's own method names.

**Subscription store ended up keyed by session id, not by raw `SseSender`, unlike the entry's own
sketch** (`Arc<RwLock<HashMap<String, Vec<SseSender>>>>`). The entry's sketch would work if a
subscription were created *from* the same connection that later receives the push, but
`resources/subscribe` arrives as a `POST /mcp` request (its own request/response cycle, no
persistent connection) while pushes go out over a *separate* `GET /mcp` SSE connection — there's no
`SseSender` available at the moment `resources/subscribe` is processed to even put in that map. The
actual missing piece was correlating "this `POST resources/subscribe` call" with "that already-open
(or not-yet-open) `GET /mcp` SSE connection," which only `Mcp-Session-Id` can do (the same header
`.tool_with_context()`'s session tracking, from TODO-2, already established as this server's
session-continuity mechanism). So `subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>>` stores
session ids, not senders; `start_sse_stream` gained a new step reading `Mcp-Session-Id` off the
`GET` request and tagging the connection with it via a new `SseClient { session_id, sender }`
struct, replacing the previously-untagged flat `Vec<SseSender>` (`sse_clients` field). This is also
why `do_resource_subscribe`/`do_resource_unsubscribe` need `ctx.session_id`, not just `body` — this
entry's own "New methods" sketch already anticipated needing `session_id` as a parameter, just via
`handle_request`'s signature rather than routed through `McpContext`.

**Genuinely targeted, unlike every other notification in this module:** `.notify()`, `.log()`, and
`list_changed` all broadcast to *every* connected SSE client via `broadcast_sse_to` — that was an
acceptable, honestly-documented simplification when TODO-7 was built (nothing else needed
per-session routing yet). `notify_resource_updated` is the first notification that must reach only
specific subscribers, so it uses a new `send_sse_to_sessions` (a `session_id`-filtering sibling of
`broadcast_sse_to`, added alongside it) instead.

**Requires `Mcp-Session-Id`, by design, not an oversight:** `resources/subscribe`/
`resources/unsubscribe` both return `INVALID_PARAMS` for a request with no session id — a
subscription created with no way to later correlate it to an SSE connection could never actually
fire, so accepting it silently would be worse than rejecting it outright. `resources/unsubscribe`
prunes a URI's `subscriptions` entry entirely once its subscriber list empties, rather than
leaving a stale empty `Vec` around.

**Capabilities:** `initialize`'s `resources` capability now advertises `"subscribe":true`
unconditionally (previously `false`, since this didn't exist) — matching how `listChanged` became
unconditional in TODO-9, not opt-in like `.logging_enabled()`, since the methods are always
available on every `McpServer`.

**Known limitation, stated plainly:** a session's SSE connection disconnecting doesn't proactively
clean up its entries in `subscriptions` — the same "no eviction without an explicit call" tradeoff
already accepted for `sessions` in TODO-2. A stale session id just sits harmlessly in a URI's
subscriber list (there's no live `SseClient` for it anymore, so `send_sse_to_sessions` simply never
finds a matching connection to send to) until an explicit `resources/unsubscribe`.

**Verified end-to-end against a real running server** (this feature's session/SSE-connection
correlation isn't something unit tests alone can fully exercise, similar to TODO-7): `initialize` →
read back `Mcp-Session-Id` → `GET /mcp` with that header → `resources/subscribe` with the same
header → `.notify_resource_updated(...)` from a background thread on a timer. The subscribed
session's SSE stream received `notifications/resources/updated`; a second, simultaneously-open SSE
connection using a different, never-subscribed session id received nothing — confirming the
targeting actually works, not just that broadcast-to-everyone would have looked the same.

10 new tests in `src/mcp/tests.rs`: subscribe succeeds with a session id and fails
(`INVALID_PARAMS`) without one or without `uri`; unsubscribe fails without a session id;
`notify_resource_updated` delivers to a subscribed session's SSE reader with the correct
`method`/`uri`; it does *not* reach a second, connected-but-unsubscribed session (proven via the
established "send a marker next, confirm it's the only thing read back" pattern); it's a silent
no-op for a URI nobody ever subscribed to; unsubscribing stops further delivery; unsubscribing the
last subscriber prunes the URI's `subscriptions` entry entirely; and `initialize` advertises
`resources.subscribe:true`.

**Effort:** medium, as estimated — the `Mcp-Session-Id`-based correlation (not anticipated in the
entry's sketch, which assumed the wrong kind of state to store) was the actual source of the
effort, not the subscribe/unsubscribe method bodies themselves, which are straightforward once the
right key (session id) was identified.

---

### ✅ TODO-15: `sampling/createMessage` — server-side sampling — Done (v17.88.0)

`ctx.sample(request, timeout)` (a `.tool_with_context()`-only method, matching this entry's own
`server_handle.sample(...)` sketch in spirit) sends a `sampling/createMessage` request over the
`GET /mcp` SSE channel (TODO-7) and blocks until the client's `POST /mcp` reply arrives or `timeout`
elapses.

**Blocking, not `async fn`, deliberately** — the entry's own sketch used `.await`, but this entry's
"Depends on: TODO-7 and async execution" was never fully buildable: async tool handlers (TODO-17)
still don't exist in this crate, so there is no `.await` point inside a tool handler to suspend at
even if `sample()` itself were async. A synchronous, thread-parking implementation sidesteps that
missing dependency entirely — the calling thread blocks on an `mpsc::Receiver` for up to `timeout`,
tying up one worker on a thread-pool server, the same tradeoff `timeout::with_timeout` already
accepts (there, bounding a slow handler's *caller*; here, bounding this handler's own wait).

**No `SamplingMessage` type** — the entry's sketch used one, but `PromptMessage` (existing, from
TODO-2 era prompt support) already models the exact same wire shape
(`{"role":...,"content":{"type":"text",...}}`), constructors included. `SamplingRequest.messages:
Vec<PromptMessage>` reuses it directly rather than introducing a duplicate type for an identical
shape. `SamplingRequest` itself covers `messages`/`max_tokens`/`system_prompt` — the spec's other
optional fields (`modelPreferences`, `stopSequences`, `metadata`, `includeContext`, `temperature`)
are out of scope here, an explicit, deliberate scope narrowing rather than an oversight, given this
entry's own "Effort: large" framing already anticipated trimming somewhere.

**The actual novel piece wasn't sending the request — it was routing the reply back.** Every prior
MCP feature in this server only ever needed to recognize incoming *requests* (things with a
`method`). A `sampling/createMessage` reply is a JSON-RPC *response* — no `method` field, by
definition — arriving as an ordinary `POST /mcp` body that looks, to the rest of this dispatch
table, exactly like a malformed request. `McpServer` gained `pending_sampling: Arc<Mutex<HashMap<String,
mpsc::Sender<Result<String, String>>>>>`, keyed by a freshly minted (and pre-quoted, spec allows
`string | number`) request id; `sample()` registers a sender there, sends the request via the
existing `send_sse_to_sessions` (TODO-14's targeted-delivery primitive — sampling requests go to
exactly the session that asked for them, same as resource updates), and blocks on the matching
receiver. `handle_request_with_context`/`handle_batch` both now check a method-less body with a
recognized `id` against `pending_sampling` (`try_deliver_sampling_response`) *before* falling back
to the pre-existing "Missing method" `INVALID_REQUEST` error — an unrecognized method-less body
still gets that same error, unchanged from before this feature existed.

**Fails fast, spec-aware, before ever sending anything:** a new `StoredClientInfo.supports_sampling`
bool (checked from `params.capabilities.sampling` at `initialize` time, alongside the existing
`clientInfo` extraction) reflects that sampling is a *client*-declared capability per spec — the
server doesn't advertise it in its own `initialize` response the way `logging`/`completions` are
server-declared, since it's the client, not the server, doing the answering. `sample()` returns an
immediate error (no request sent, no wait) if the connecting client never declared it, if there's no
session id to address a request to, or if `ctx` has no live server behind it. Only a genuine
non-answer (client connected, capable, but silent) surfaces as the `recv_timeout` error — including
the case where the client never even opened a `GET /mcp` SSE connection for that session, since
`send_sse_to_sessions` has no separate "nobody's listening" signal to report back.

**Verified end-to-end against a real running server** (this bidirectional flow, even more than
TODO-7/TODO-14, isn't something unit tests alone prove — though a real spawned-thread unit test
does cover the actual channel mechanics): `initialize` declaring `capabilities.sampling` → `GET
/mcp` with the returned session id → a `tools/call` invoking `ctx.sample(...)`, which blocked → the
`sampling/createMessage` request appeared on that session's SSE stream with a fresh id → a `POST`
carrying `{"id":<same id>,"result":{...}}` unblocked the original call, which returned the sampled
content end to end.

5 new tests in `src/mcp/tests.rs`: fails fast without a declared sampling capability; fails fast
without a session id even when sampling is otherwise declared; times out (short timeout, ~50ms) when
nobody ever responds; the full round trip (a spawned thread plays the client — reads the outbound
request off the real SSE reader, extracts its id, posts the matching response — while the main
thread's `tools/call` blocks in `ctx.sample()` until that arrives), also confirming `pending_sampling`
is cleaned up afterward; and an error response (`{"error":{"message":...}}` instead of `{"result":...}`)
surfaces that message as `sample()`'s `Err`.

**Effort:** large, as estimated, though the actual size came almost entirely from the
request/response correlation (`pending_sampling`, the method-less-body detection in both dispatch
paths) rather than the request-sending half, which was a short hop from TODO-14's existing
`send_sse_to_sessions`.

---

### ✅ TODO-16: `roots/list` and `notifications/roots/list_changed` — Done (v17.89.0)

`ctx.list_roots(timeout)` asks the connected client which filesystem roots it has access to,
returning `Vec<McpRoot>` (`{ uri: String, name: Option<String> }`, matching this entry's own sketch
of the shape). `notifications/roots/list_changed` invalidates a per-session cache so the next call
re-fetches.

**Not stored directly on `McpContext` as `pub roots: Vec<McpRoot>`**, unlike the entry's own sketch
— `McpContext` is rebuilt fresh for every request (from headers, in `context_for`), so a field on it
has no way to persist a fetched-once value across a session's multiple requests the way this
entry's caching intent (implied by "re-request... on list_changed", which only makes sense against
something that was cached) requires. Roots are cached instead in the existing per-session
`StoredClientInfo` (alongside `client_name`/`client_version`/`supports_sampling`) as a new
`roots: Option<Vec<McpRoot>>` field — `None` meaning "never fetched, or invalidated." `ctx.list_roots()`
is the accessor: cache hit returns immediately with nothing sent; cache miss does a live round trip
and populates it.

**"Requires sampling-style bidirectional call — needs TODO-15" was exactly right, and TODO-15 is
now done** — so this entry's "partial implementation" fallback wasn't needed. Rather than
duplicating TODO-15's request/response plumbing a second time, `McpContext::sample`'s internals were
extracted into a shared private `send_and_wait(method, params_json, timeout)` (mint a request id,
register a reply channel in what was `pending_sampling` and is now more accurately named
`pending_replies: Arc<Mutex<HashMap<String, mpsc::Sender<Result<String,String>>>>>` since it's no
longer sampling-specific, send via `send_sse_to_sessions`, block, clean up). `sample()` and the new
`list_roots()` are now both thin callers of it, differing only in the method/params sent and how
each parses its own `result` shape (`parse_sampling_response` vs. the new `parse_roots_response`).

**`notifications/roots/list_changed` needed a session-id-only correlation**, unlike
`notifications/cancelled` (which keys off a `requestId` in its own params) — the spec's
`list_changed` notification carries no params at all; it's purely "the connection that sent this
has stale roots." `handle_request_with_context`/`handle_batch` both special-case it (ahead of the
generic notification-swallowing branch, same position as the `cancelled` check) and call a new
`invalidate_roots_cache(&ctx.session_id)`, which clears that session's `StoredClientInfo.roots`
back to `None`.

**`roots` capability support-check mirrors `sampling`'s exactly**: a new
`StoredClientInfo.supports_roots` bool, set from `params.capabilities.roots`'s presence at
`initialize` time (client-declared, like sampling, not server-advertised), gates `list_roots()`
failing fast before ever sending a request to a client that never said it could answer one.

**Verified end-to-end against a real running server**, exercising the full lifecycle in one
session: `initialize` declaring `capabilities.roots` → `GET /mcp` → a `tools/call` invoking
`ctx.list_roots(...)`, which round-tripped over SSE and returned the client's roots → a second
`tools/call` returned instantly from cache (no new SSE frame) → `notifications/roots/list_changed`
→ a third `tools/call` produced a *new* `roots/list` request with a fresh id, resolving to
newly-provided roots — confirming both the cache and its invalidation actually work, not just that
a single round trip does.

5 new tests in `src/mcp/tests.rs`: fails fast without a declared `roots` capability; fails fast
without a session id even when declared; times out when nobody answers; a full round trip that also
proves caching (a second `tools/call` needs no responder interaction at all to succeed); and
`notifications/roots/list_changed` invalidation (a responder thread answering exactly two
sequential `roots/list` requests — the test would hang, not merely fail, if invalidation didn't
work, since the responder would block forever on a second request that never arrives).

**Effort:** ended up smaller than "medium" once TODO-15's plumbing existed to extract and reuse —
the actual new surface area was `McpRoot`, the cache field, and the notification correlation, not a
second bidirectional-request implementation.

---

### ✅ TODO-17: Async tool handlers (`http2` feature) — Done (v17.90.0)

`.async_tool(name, description, schema, handler)` registers a tool whose handler is
`Fn(&str) -> impl Future<Output = Result<McpContent, String>>`, matching this entry's own sketch
almost exactly (its example even used the same `call_api`/`AsyncClient` shape).

**Bridge is `block_on_isolated`, not `tokio::task::block_in_place`** — the one deliberate deviation
from this entry's own text. `block_in_place` only works on the `multi_thread` tokio scheduler and
panics under `current_thread`; this crate already discovered that limitation building
`H2ReverseProxy`/`AsyncAppWithState::execute` (both documented in `CLAUDE.md` as using
`crate::async_bridge::block_on_isolated` specifically *because* `block_in_place` "requires
`multi_thread`") and built `block_on_isolated` to work under either scheduler by spawning a scoped
OS thread with its own single-threaded runtime when already inside one. Using `block_in_place` here
as the entry suggested ("same pattern as `H2ReverseProxy::handle`" — which itself does *not* use
`block_in_place`, contradicting its own parenthetical) would have silently constrained every
async-tool user to a `multi_thread` runtime with no compile-time signal that they'd done so. Reusing
the existing bridge instead means no new mechanism and no new dependency.

**Storage kept separate (`async_tools: Arc<RwLock<Vec<AsyncToolDef>>>`), not unified into `ToolDef`
via a handler enum** — exactly the entry's own "new `AsyncToolDef` storage" framing. Unifying would
have meant touching every existing sync-tool code path (`ToolFn`'s call sites, `register_tool`,
`.tool()`/`.tool_with_context()`/`.tool_annotated()`) to route through a handler enum for a feature
that's optional and off by default outside `http2`; keeping two parallel, `#[cfg(feature = "http2")]`-gated
collections is far less invasive. `tools/list` merges both transparently via a new shared
`render_tool_list_entry` helper (so the two collections render identically, no duplicated
formatting code); `tools/call` checks `tools` first, then `async_tools` if `http2` is enabled — from
a client's perspective there is no distinction between the two kinds of tool.

**Went a little beyond the entry's literal ask, for completeness rather than leaving an
asymmetry:** added `.register_async_tool(...)` (the TODO-9-style dynamic `&self` equivalent of
`.async_tool()`) and made `.remove_tool(name)` check *both* collections — otherwise sync tools would
have gotten construction-time *and* dynamic registration/removal while async tools only got
construction-time, an inconsistency nothing in the entry asked for but that would have been an odd,
unexplained gap to leave. No async equivalent of `.tool_with_context()`/`.tool_annotated()` was
added, though — genuinely out of scope, not requested, and adding either is a small, obvious
follow-up if ever needed (the internal `AsyncToolFn` type deliberately has no unused `McpContext`
parameter sitting around for a feature that doesn't exist yet).

**Verified end-to-end against a real running `http2`/`http3`-featured server** — the scenario this
feature actually targets, a live tokio runtime driving the connection, not just an isolated unit
test: an async tool handler that internally `tokio::time::sleep(...).await`ed took exactly as long
as that sleep and returned the correct result, confirming `block_on_isolated` genuinely polled the
future to completion rather than merely calling and dropping it.

6 new tests in `src/mcp/tests.rs` (in a `#[cfg(feature = "http2")] mod async_tool_tests`, matching
how the rest of the crate's `http2`-gated code is tested): an async tool appears in `tools/list` and
is callable; an async tool's `Err` surfaces as `isError:true`; `register_async_tool` makes a tool
immediately callable and pushes `notifications/tools/list_changed`; `remove_tool` removes a
dynamically-registered async tool; a server with both a sync and an async tool lists and calls both
correctly; and an unknown tool name is still an error when only async tools are registered (a
regression guard that the fallback-to-error path still works once a second lookup was added).

**Effort:** medium, as estimated — the actual size was almost entirely in touching `do_tools_list`/
`do_tools_call`/`remove_tool` to check two collections instead of one, not the bridging mechanism
itself, which already existed and needed zero changes to reuse.

---

## Implementation order

```
Phase 1 — Quick wins (no new dependencies, mostly additive)
  TODO-1  protocol version negotiation     (tiny)              ✅ done (v17.75.0)
  TODO-2  McpContext in tool handlers      (small)              ✅ done (v17.76.0)
  TODO-3  tool annotations 2025-03-26      (tiny)              ✅ done (v17.77.0)
  TODO-4  image + embedded content types   (small)              ✅ done (v17.78.0)
  TODO-5  JSON-RPC batch requests          (small)              ✅ done (v17.79.0)
  TODO-6  list pagination                  (small)              ✅ done (v17.80.0)
  TODO-11 completions/complete             (small)              ✅ done (v17.85.0)

Phase 2 — Streaming foundation (enables all notification features)
  TODO-7  GET /mcp SSE channel            (medium — unblocks 8, 9, 10, 14, 15, 16)   ✅ done (v17.81.0)
  TODO-8  logging/setLevel + notifications (small, needs TODO-7)              ✅ done (v17.82.0)
  TODO-9  dynamic registration             (medium, needs TODO-7)             ✅ done (v17.83.0)
  TODO-10 notifications/progress           (small, needs TODO-7 + TODO-2)      ✅ done (v17.84.0)

Phase 3 — Enterprise + advanced
  TODO-11 completions/complete            (small, can go in Phase 1)          ✅ done (v17.85.0)
  TODO-12 request cancellation            (medium, http2 only)               ✅ done (v17.86.0, sync cooperative flag — no http2 dependency needed)
  TODO-13 OAuth 2.0 (2025-03-26)         (small — JwksCache already exists)      ✅ done (v17.98.0)
  TODO-14 resources/subscribe             (medium, needs TODO-7 + TODO-9)      ✅ done (v17.87.0)
  TODO-17 async tool handlers             (medium, http2 only)               ✅ done (v17.90.0)
  TODO-15 sampling/createMessage          (large)               ✅ done (v17.88.0)
  TODO-16 roots/list                      (medium, needs TODO-15)             ✅ done (v17.89.0)
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
| 6 | List pagination | 2024-11-05 | **P1** | Small | ✅ Done (v17.80.0) |
| 11 | `completions/complete` | 2024-11-05 | **P1** | Small | ✅ Done (v17.85.0) |
| 7 | SSE transport (`GET /mcp`) | Streamable HTTP | **P2** | Medium | ✅ Done (v17.81.0) |
| 8 | `logging/setLevel` | 2024-11-05 | **P2** | Small | ✅ Done (v17.82.0) |
| 9 | Dynamic registration + `listChanged` | 2024-11-05 | **P2** | Medium | ✅ Done (v17.83.0) |
| 10 | `notifications/progress` | 2024-11-05 | **P2** | Small | ✅ Done (v17.84.0) |
| 12 | Request cancellation | 2024-11-05 | **P3** | Medium | ✅ Done (v17.86.0) |
| 13 | OAuth 2.0 auth | 2025-03-26 | **P3** | Small | ✅ Done (v17.98.0), `sso` feature |
| 14 | `resources/subscribe` | 2024-11-05 | **P3** | Medium | ✅ Done (v17.87.0) |
| 17 | Async tool handlers | Ergonomics | **P3** | Medium | ✅ Done (v17.90.0) |
| 15 | `sampling/createMessage` | 2024-11-05 | **P3** | Large | ✅ Done (v17.88.0) |
| 16 | `roots/list` | 2024-11-05 | **P3** | Medium | ✅ Done (v17.89.0) |
