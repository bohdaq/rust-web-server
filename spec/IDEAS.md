# IDEAS

Potential improvements and future directions for `rust-web-server`.
Items are grouped by theme; there is no implied priority order within a group.

---

## Streaming

**Streaming SSE** — The current `Sse` builder buffers the entire body before writing.
Add a `SseWriter` that holds a reference to the raw TCP stream and sends each event
immediately as it is produced.  Required for live AI token output, sensor feeds, and
long-running push scenarios.  The WebSocket module already documents the stream-takeover
pattern; SSE can use the same approach.

**WebSocket inside the router** — The WebSocket API today requires bypassing `Server::run`
and running a custom accept loop.  Expose a `ws_upgrade(stream, handler)` helper that
can be called from inside a normal router handler so WebSocket routes can live alongside
HTTP routes without a separate listener.

---

## Protocol

**gRPC / Connect** — HTTP/2 is already wired up.  Add gRPC-over-HTTP/2 framing (5-byte
length-prefixed protobuf messages, `application/grpc` content type, trailer-based status)
so the server can speak to gRPC clients without an external proxy.  Start with unary RPCs;
streaming RPCs follow the SSE/WebSocket stream-takeover model.

**HTTP/2 server push** — The `h2_handler` already negotiates HTTP/2 connections.  Add an
opt-in `push(path)` API on `Response` that triggers `PUSH_PROMISE` frames so assets can
be preemptively sent alongside an HTML page.

---

## Developer experience

**Admin web UI** — Serve a minimal HTML/JS dashboard at `/rws-admin` (behind
`BasicAuthLayer` by default) that wraps the existing MCP tools in a browser interface:
maintenance-mode toggle, IP blocklist editor, live metrics chart, and a recent-requests
table.  No external JS framework — keep it self-contained.

**`rwsctl` CLI** — A companion binary that speaks to the MCP `/mcp` endpoint so operators
can run `rwsctl maintenance on`, `rwsctl block-ip 1.2.3.4`, or `rwsctl config reload`
from the shell without needing an MCP-capable client.

---

## Auth and security

**HTTP Message Signatures (RFC 9421)** — Add a `HttpSigLayer` for server-to-server
request signing and verification.  Builds on the HMAC-SHA256 already used in the `auth`
feature.

**API key rotation** — Extend `JwtLayer` / `BasicAuthLayer` with a multi-key validation
callback so secrets can be rotated without a restart: the new key is accepted immediately
and the old key is accepted during a configurable grace period.

---

## Load balancing (proxy)

**`WeightedRoundRobin`** — Allow backends to carry a weight so traffic can be shifted
gradually during a rolling deploy.

---

## Distribution

**Wasm middleware** — Allow middleware plugins compiled to Wasm (Wasmtime host) so users
can extend the server without native compilation or restarting the binary.  Define a
simple ABI: the Wasm module receives request bytes, returns a decision (pass / modify /
respond), and has no ambient capabilities.
