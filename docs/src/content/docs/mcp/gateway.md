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

## Prompts and resources

Upstream prompts appear as `<prefix>__<name>` with their arguments, and `prompts/get` is forwarded. Upstream resources and resource templates keep their own URIs (if two upstreams publish the same URI, the first keeps it) and `resources/read` is forwarded. Both are listed at `build()` — unlike tools, they aren't refreshed — and an upstream that has neither, or fails to list them, still federates its tools.

The rules from [`allow`](#who-may-call-what) cover them too: a caller sees a prompt when a rule matches its `<prefix>__<name>`, and a resource when one matches `<prefix>__<uri>` (so `github__*` covers everything from `github`; listing only tool names covers no prompts or resources). Hidden ones are left out of the lists and answer "not found" (since 18.9.0).

## Rate limits

`.limit(tools, max_calls, window_secs)` caps how often each caller (each `sub`) may call the tools matching `tools` — a full name, a prefix ending in `*`, or `*` — counted per tool:

```rust
use rust_web_server::mcp::gateway::{McpGateway, Upstream};

let gateway = McpGateway::new("gateway", "1.0")
    .upstream(Upstream::new("github", "http://127.0.0.1:9001/mcp"))
    .allow("*", ["*"])
    .limit("github__*", 60, 60)            // each caller: 60 calls a minute per GitHub tool
    .limit("github__create_issue", 5, 3600); // and 5 new issues an hour
```

Over a limit, the call answers a tool error saying when to retry, without reaching the upstream. A call must pass every limit that matches. Counts are kept in the gateway's process (since 18.9.0).

## Audit log

`.audit_log(true)` writes one JSON line per `tools/call` to the access log (stdout, or the file `RWS_CONFIG_ACCESS_LOG` names):

```json
{"time":"2026-10-29T12:00:00Z","event":"mcp_tool_call","sub":"alice","tool":"github__create_issue","upstream":"github","outcome":"ok","duration_ms":182.411}
```

`outcome` is `ok`, `tool_error`, `refused` (over a [rate limit](#rate-limits)) or `denied` (the caller may not use the tool). Arguments and results are not logged (since 18.9.0).

## Upstreams

- `Upstream::new(prefix, url)` — `url` is the server's Streamable HTTP
  endpoint. The prefix is letters, digits, `-` and single `_`.
- `.header(name, value)` — sent on every request to that upstream; the
  caller's `Authorization` is never passed on.
- `.header_for(name, |claims| …)` — a header computed for each tool call
  from the caller's verified claims (the `auth_claims` JSON), e.g. that
  user's own upstream token, so the upstream acts as them; `None` sends no
  header. Only `tools/call` carries a caller: listing, prompts and resources
  use the static headers (since 18.9.0).
- `.timeout_ms(ms)` — per request (default 30 s).

`McpGateway::refresh_every(Some(duration))` (default 5 minutes; `None` lists
once) re-reads every upstream's tools: new ones appear, removed ones go,
changed ones are re-published, and the gateway's clients get
`notifications/tools/list_changed`. Between refreshes `tools/list` is answered
from the gateway's copy; an upstream that can't be reached keeps the tools it
had (since 18.9.0).

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

Upstream notifications (`listChanged`, resource updates, progress) aren't followed — tools are re-read on the [refresh interval](#upstreams) instead — and tool results aren't cached. The [roadmap](https://github.com/bohdaq/rust-web-server/blob/main/spec/ROADMAP.md) tracks the gateway (#28).
