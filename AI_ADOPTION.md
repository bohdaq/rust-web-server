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
| AI API proxy backend | Routing, JSON, CORS, K8s probes | Outbound HTTP client (`ureq`/`reqwest`) |
| Streaming (SSE / token streaming) | HTTP/2 framing infrastructure | Streaming `Response` write path |
| MCP tool server | Full HTTP stack, JSON | One new controller + MCP envelope parsing |
| OpenAI-compat `/v1/chat` | Routing, JSON bodies | Outbound client + schema mapping |

**Recommended first step**: Path 3 (MCP server) is the highest-leverage and lowest-risk addition — purely additive, no core changes, and immediately makes this server usable as a tool by Claude and other AI agents.
