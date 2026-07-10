# AI Adoption

Three concrete paths to make this server AI-first, grounded in what the codebase already has.

---

## Path 1: AI API Backend

Add controllers that call an upstream AI provider (Claude, OpenAI, etc.) and return results. This server already has everything needed:

- JSON parsing (from scratch, already in the codebase)
- CORS headers
- HTTP/2 + HTTP/3 for low-latency clients
- Health/ready/metrics endpoints (already wired)

What to add: a `POST /v1/chat/completions` controller that makes an outbound HTTP call to `https://api.anthropic.com/v1/messages`, formats the response, and returns it. The `Controller` trait in `src/controller/mod.rs` makes this a clean addition.

**Gap**: The server has no outbound HTTP client. Add `ureq` or `reqwest` as a dependency for the upstream call.

---

## Path 2: Streaming / SSE

AI responses need to stream token-by-token. This requires **Server-Sent Events (SSE)** — chunked `text/event-stream` responses.

The current `Response` type uses `Vec<ContentRange>` for the body (`src/response/mod.rs`), which is buffered — it does not support streaming. This is the biggest architectural gap for AI-first use.

**What is needed**: A streaming response path where the controller can write chunks incrementally to the socket rather than buffering the whole response first.

---

## Path 3: MCP Server

The [Model Context Protocol](https://modelcontextprotocol.io) is HTTP + JSON-RPC. This server can implement `POST /mcp` as an MCP tool provider — letting Claude or any MCP-compatible client call the server as a tool.

This is entirely additive: one controller that speaks the MCP JSON-RPC envelope format. No streaming required for request/response tools (only for sampling).

---

## Summary

| Goal | What exists | What is missing |
|------|-------------|-----------------|
| AI API proxy backend | Routing, JSON, CORS, K8s probes, `ReverseProxy` middleware (v17.20.0) | Outbound HTTP client for custom header injection (`ureq`/`reqwest`) |
| Streaming (SSE / token streaming) | HTTP/2 framing infrastructure | Streaming `Response` write path |
| MCP tool server | Full HTTP stack, JSON | One new controller + MCP envelope parsing |
| OpenAI-compat `/v1/chat` | Routing, JSON bodies | Outbound client + schema mapping |

**Recommended first step**: Path 3 (MCP server) is the highest-leverage and lowest-risk addition — purely additive, no core changes, and immediately makes this server usable as a tool by Claude and other AI agents.

---

## Making the Framework an AI First Class Citizen

The goal: when a developer tells Claude, Cursor, or Copilot "build me a REST API with rust-web-server", the AI generates correct, idiomatic, compiling code on the first try — without hallucinating APIs, inventing async patterns that don't exist, or using wrong types.

There are five concrete layers:

---

### Layer 1 — `llms.txt` (highest leverage, one file)

[llmstxt.org](https://llmstxt.org) is the emerging standard — a flat Markdown file at the repo root that AI tools consume to understand a project. It is like `robots.txt` but for LLMs. Claude Code, Cursor, and others already look for it.

Contents: what the framework is, the key types (`Request`, `Response`, `Controller`), the routing pattern, concrete code snippets for every common task. This is the single highest-ROI file you can add.

---

### Layer 2 — `examples/` directory (Cargo-native)

Right now there is no `examples/` directory. Cargo examples are indexed by `docs.rs`, scraped by GitHub, and are exactly what AI coding tools pattern-match against. Every common use case needs one:

- `examples/hello_world.rs` — minimal GET endpoint
- `examples/rest_api.rs` — CRUD with JSON bodies
- `examples/auth.rs` — bearer token check in `is_matching`
- `examples/file_upload.rs` — multipart body parsing
- `examples/sse.rs` — streaming / AI token output (future)

---

### Layer 3 — Ergonomic API helpers (less boilerplate = less hallucination surface)

Right now returning a JSON response is ~6 lines. AI tools generate each line and can get any one wrong. If it were:

```rust
Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, body_bytes)
Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid input")
```

…the surface area for errors collapses to one line. Helpers to add to `Response` and `Request`:

- ✅ **Done** — `Response::json(status, bytes) -> Response` (`src/response/mod.rs`). Takes already-serialized bytes, not a `Serialize` value, so it has no dependency on the `serde` feature; pair with `serde_json::to_vec` or hand-rolled JSON. Builds a `Content-Type: application/json` response via the existing `Range::get_content_range`/`Response::get_response` machinery — no new body-construction path. For a typed value in one call, `Json(value).into_response()` (`serde` feature) already existed and still handles the serialize-and-respond case.
- ✅ **Done** — `Response::text(status, str) -> Response` (`src/response/mod.rs`). Builds a `Content-Type: text/plain` response the same way.
- `Request::body_as_str(&self) -> Result<&str, Response>` — not yet implemented.
- `Request::query_param(&self, key) -> Option<&str>` (wraps the existing `URL` machinery) — not yet implemented.

---

### Layer 4 — System prompt file

A `prompts/SYSTEM_PROMPT.md` that users paste into their AI tool's system prompt. It contains the key types, patterns, and constraints (no async in http1, routing by declaration order, etc.) in the exact format AI models consume best. Ship it as part of the repo.

---

### Layer 5 — MCP server controller (forward-looking)

One controller implementing the Model Context Protocol JSON-RPC envelope at `POST /mcp`. This makes the running server itself a tool that AI agents can call — no external infrastructure needed. Purely additive to the existing Controller pattern.

---

### Priority order to implement

| # | What | Effort | Impact |
|---|------|--------|--------|
| 1 | `llms.txt` | 1 hour | Immediate — all AI tools pick it up |
| 2 | `examples/` directory (3-5 examples) | 2-3 hours | docs.rs indexing + GitHub discovery |
| 3 | `Response::json` / `Response::text` helpers | 1 hour | ✅ Done — every AI-generated controller gets shorter |
| 4 | `prompts/SYSTEM_PROMPT.md` | 30 min | Users can activate it today |
| 5 | MCP server controller | 1 day | Future-proofing for agentic tools |
