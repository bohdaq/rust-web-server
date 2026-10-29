# rws 18.9.0 — AI gateway

Written 2026-10-29. The three 18.9 items from [ROADMAP.md](ROADMAP.md),
built tranche by tranche; this file is updated as each lands.

## Decisions (user, 2026-10-29)

- One release, **18.9.0**, when all three land.
- **#28 MCP gateway**: library *and* config file (`rws.config.toml`), so binary
  users get one policy point.
- **#41 LLM proxy**: OpenAI-compatible (`/v1/chat/completions`, `usage` in the
  final SSE chunk) and Anthropic (`/v1/messages`, `message_start` /
  `message_delta` usage).
- **#41 budgets**: shared through Redis now (the RESP client `acme::shared`
  uses), in memory when no Redis is configured.
- **X-Forwarded-Proto**: `ConnectionInfo` gains `tls: bool`, plus `Default`
  and `ConnectionInfo::new`, so code outside rws can stop using full struct
  literals. Not `#[non_exhaustive]` (a further break; not asked for).

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | X-Forwarded-Proto | `tls` set where connections are accepted (TLS, h2c/plain, HTTP/3, Gateway API listeners, sync server, TestClient); every forwarder sends `X-Forwarded-Proto` unless the client did; byte-exact proxy tests; docs replace the `header_set` recipe |
| 2a | Gateway: version bridging | an upstream that only speaks 2026-07-28 (stateless) works; fake-upstream tests run in both modes |
| 2b | Gateway: `tools/list` refresh | TTL re-list, upstream `listChanged` → re-list + `notifications/tools/list_changed` to clients, via `McpServer::register_*`/`remove_*` |
| 2c | Gateway: per-tool rate limits | a generic `McpServer` before-call hook; limits per identity + tool |
| 2d | Gateway: audit log | one JSON line per `tools/call` (time, sub, tool, upstream, duration, outcome) through the access-log sink |
| 2e | Gateway: resources and prompts | federated with the same prefixes |
| 2f | Gateway: per-identity upstream credentials | upstream headers from a function of the caller's claims |
| 2g | Gateway: config file | `[[mcp_gateway]]` in `rws.config.toml`, `rws --check`, credentials by env name; e2e `ci/e2e/mcp_gateway.sh` federating two upstreams with ACLs; MCP conformance against the gateway |
| 3 | LLM proxy | `LlmLayer` / `[route.middleware.llm]`: OpenAI + Anthropic usage (JSON and SSE), TTFT, `rws_llm_*` metrics, virtual keys, provider fallback before the first byte, token budgets in Redis (memory without it); fuzzed SSE/usage parser; `ci/e2e/llm_proxy.sh` with a mock provider; `examples/llm_proxy.rs`; docs page |
| 4 | Release | CHANGELOG, migration notes for the `ConnectionInfo` field, 18.9.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Status

| # | State |
|---|---|
| 1 | **done 2026-10-29**: `ConnectionInfo.tls` (set by HTTP/1.1, HTTP/2, HTTP/3 accept paths; false for the sync server, TestClient, adapters), `Default` + `ConnectionInfo::new`; `proxy::Peer { ip, tls }` replaces `client_ip: &str` through the forwarders (all `pub(crate)`); `X-Forwarded-Proto` in `build_request`, the h2 upstream, Gateway API upgrades and mirrors, kept when the client sent one. 117 literals updated. Tests: 2231 default, 1918 http1, 93 gateway-api |
| 2a | **done**: fallback to stateless 2026-07-28 when `initialize` is refused (`server/discover`, `_meta` after the params' own members, `Mcp-Method`/`Mcp-Name`), 2026-only result fields stripped, `input_required` refused; test with a 2026-only fake upstream |
| 2b | **done**: `refresh_every` (default 5 min) re-lists, diffs by fingerprint, registers/removes on the live server, notifies `tools/list_changed`; unreachable upstreams keep their tools. Not done: following upstream `listChanged` pushes (the interval covers it) |
| 2c | **done**: `McpServer::before_tool_call` (refusal = tool error), `McpGateway::limit(tools, max, window)` per `sub` + tool on `RateLimiter` |
| 2d | **done**: `McpServer::after_tool_call` + `ToolCallOutcome` (ok/tool_error/refused/denied) + duration; `McpGateway::audit_log(true)` → JSON line per call via the access-log sink |
| 2e | in progress |
| 2f–2g | — |
| 3 | — |
| 4 | — |
