---
title: MCP gateway
description: Put one authenticated MCP endpoint in front of several MCP servers — prefixed tool names, a per-caller allow-list, and upstream credentials that never leave the gateway.
---

`McpGateway` (since 18.0.0) federates upstream MCP servers behind one
`/mcp`. Clients connect to the gateway with their own credentials; the
gateway calls each upstream with *its* credentials, so the tokens for your
GitHub, Jira or internal MCP servers live in one place and never reach the
agents. Which caller may use which tools is one list.

The runnable version is
[`examples/mcp_gateway.rs`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_gateway.rs):
`cargo run --example mcp_gateway` starts two upstreams in-process and the
gateway in front of them.

```rust
use rust_web_server::mcp::gateway::{McpGateway, Upstream};

let gateway = McpGateway::new("gateway", "1.0")
    .upstream(Upstream::new("github", "https://mcp.github.example/mcp")
        .header("Authorization", format!("Bearer {}", std::env::var("GITHUB_MCP_TOKEN")?)))
    .upstream(Upstream::new("jira", "http://jira-mcp.internal:8080/mcp"))
    .allow("alice", ["*"])
    .allow("ci", ["github__search_*", "jira__get_issue"])
    .build()?                                   // connects and lists every upstream's tools
    .require_bearer_tokens([
        (std::env::var("ALICE_TOKEN")?, "alice"),
        (std::env::var("CI_TOKEN")?, "ci"),
    ]);

let (listener, pool) = Server::setup()?;
Server::run_tls(listener, pool, gateway).await;
```

## What callers see

- Each upstream tool appears as `<prefix>__<tool>` (`github__search_issues`),
  with the upstream's description, input and output schemas, title and
  behaviour hints.
- A call is checked against the upstream's input schema, then forwarded;
  the upstream's result comes back as it was sent — content, structured
  content, `isError`.
- When the upstream fails — it is down, answers HTTP 5xx, or returns a
  JSON-RPC error — the caller gets an `isError` result naming the upstream
  (`github: rate limited`), never a protocol error.

## Who may call what

`.allow(identity, tools)` rules add up:

| Pattern | Matches |
|---|---|
| `github__search_issues` | that tool |
| `github__*` | every tool from `github` (any prefix ending in `*`) |
| `*` | every tool |

The identity is the caller's `sub`: the name given with
[`require_bearer_tokens`](/mcp/auth/#one-token-per-caller), or the `sub`
claim of an OAuth token with [`require_oauth`](/mcp/auth/#oauth-20-authorization-mcp-2025-03-26).
`.allow("*", ...)` applies to every identified caller. A caller no rule
names sees an empty `tools/list`, and a tool it may not use answers as if
it didn't exist (`Unknown tool`) — nothing is forwarded. A plain
`require_bearer` token carries no identity, so behind it the gateway allows
nothing: use `require_bearer_tokens` or `require_oauth`.

## Upstreams

- `Upstream::new(prefix, url)` — `url` is the server's Streamable HTTP
  endpoint. The prefix is letters, digits, `-` and single `_`.
- `.header(name, value)` — sent on every request to that upstream; the
  caller's `Authorization` is never passed on.
- `.timeout_ms(ms)` — per request (default 30 s).

The gateway initializes a session with each upstream, sends the session id
and negotiated `MCP-Protocol-Version` on later requests, reads JSON or
event-stream replies, and starts a new session once if the upstream has
forgotten its own. `build()` fails, naming the upstream and its URL, if one
can't be reached at startup.

### Protocol versions

The gateway opens a 2025-11-25 session with each upstream. An upstream that
refuses `initialize` (one that speaks only 2026-07-28) is used statelessly
instead (since 18.9.0): `server/discover` once, then every request carries
the version and an empty client capabilities object in `params._meta` and
the `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers, with no
session. Its results lose the 2026-only fields (`resultType`, cache hints)
on the way through, so 2025 clients of the gateway can use its tools; a
tool that asks for more input (2026 multi round-trip) answers an error.
Clients of the gateway may use either version.

## Not yet

This is the minimal gateway. Tools are listed once, at `build()` — restart
the gateway to pick up an upstream's new tools. Upstream resources, prompts
and notifications aren't federated, results aren't cached, there are no
per-tool rate limits or audit log. The [roadmap](https://github.com/bohdaq/rust-web-server/blob/main/spec/ROADMAP.md)
tracks the full gateway (#28).
