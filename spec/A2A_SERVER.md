[Read Me](../README.md) > [Spec](.) > A2A server

# A2A server module (18.17.0–18.18.0)

Tranches 1–2 shipped in 18.17.0 (the user's call, 2026-10-31: the release went out with them rather than rewriting history to leave them out).

Planned 2026-10-31. [Agent2Agent (A2A) 1.0](https://a2a-protocol.org/latest/specification/)
lets one AI agent call another: discover it by its Agent Card, send it a
message, follow the task it starts. MCP (which rws already serves) connects
an agent to tools; A2A connects agents to agents. Official SDKs exist for
Python, JS, Java, Go, .NET and Rust; the compatibility kit is
[a2aproject/a2a-tck](https://github.com/a2aproject/a2a-tck), which grades
JSON-RPC, gRPC and HTTP+JSON servers.

## What rws offers

An `A2aServer` like `McpServer`: an `Application` you mount next to your
routes, where the app supplies only the agent logic.

```rust,ignore
let agent = A2aServer::new(AgentCard::new("Invoice agent", "Reads invoices, answers questions", "1.0.0")
        .skill(Skill::new("summarize", "Summarize an invoice").tags(["invoices"])))
    .on_message(|ctx, message| {
        // reply directly…
        // A2aReply::message(Message::agent_text("Done"))
        // …or start a task and update it from a thread
        let task = ctx.start_task();
        std::thread::spawn(move || {
            task.working("reading");
            task.artifact(Artifact::text("summary", "…"));
            task.complete();
        });
        A2aReply::task(task)
    })
    .require_bearer("…");
let app = App::new().a2a(agent);
```

## Protocol scope (JSON-RPC binding, 1.0)

- `GET /.well-known/agent-card.json` — the card: `name`, `description`,
  `version`, `supportedInterfaces` (`{url, protocolBinding: "JSONRPC",
  protocolVersion: "1.0"}`), `capabilities` (`streaming`,
  `pushNotifications`, `extendedAgentCard`), `defaultInputModes`,
  `defaultOutputModes`, `skills` (`id`, `name`, `description`, `tags`,
  `examples`), `securitySchemes`/`securityRequirements` from the configured
  auth (bearer, OAuth via `sso`).
- `POST <path>` JSON-RPC 2.0, PascalCase methods: `SendMessage`
  (blocking by default until terminal or interrupted state;
  `configuration.returnImmediately`, `historyLength`,
  `acceptedOutputModes`), `SendStreamingMessage` and `SubscribeToTask` (SSE:
  each event `data:` a JSON-RPC response whose `result` holds one of
  `task`, `message`, `statusUpdate`, `artifactUpdate`; the stream ends at a
  terminal state), `GetTask`, `ListTasks` (`contextId`, `status`,
  `pageSize` 1–100, `pageToken`), `CancelTask`, `GetExtendedAgentCard`
  (authenticated callers only).
- Push notifications: `Create/Get/List/DeleteTaskPushNotificationConfig`;
  webhooks POST `StreamResponse` JSON through `http_client` with the
  config's `token`; outbound URLs checked against an allow-list (SSRF) and
  retried with backoff.
- Data model as 1.0 serializes it: camelCase fields, enums as
  `TASK_STATE_*` / `ROLE_*` strings, `Part` = exactly one of `text`, `raw`
  (base64), `url`, `data` plus `mediaType`/`filename`/`metadata` (no `kind`
  discriminator), ISO 8601 UTC timestamps.
- `A2A-Version` header: `1.0` served; empty/`0.3` → `VersionNotSupportedError`
  (-32009) for now (0.3 compatibility only if a user needs it). Errors
  -32001…-32009 with `data: [{"@type": …ErrorInfo}]`.
- Not in scope: gRPC and HTTP+JSON bindings (later, if asked), card signing
  (needs a private-key signer rws doesn't have yet).

## Reuse

- JSON: the MCP module's hand-rolled `json_rpc` helpers (no serde needed;
  `serde` feature adds typed `Part::data`).
- SSE: `crate::sse::SseReader`, like `GET /mcp`.
- Tasks: the state machine `mcp::tasks` already has (Mutex + Condvar,
  terminal states sticky, listeners per task) moved to a shared module.
- Auth: `require_bearer`/`require_bearer_tokens`/`require_oauth` as in MCP;
  tasks keyed by the caller's identity so callers can't read each other's.
- Limits: max tasks kept (LRU, 1 h TTL after terminal), max SSE subscribers,
  history cap, body size — the MCP session limits' pattern.

## Proxy side (A2A gateway)

`[[route]] action = "a2a"` in front of upstream agents: rewrites the
upstream Agent Card's `supportedInterfaces[].url` to the public URL, applies
the route's auth and rate limits, streams SSE through, logs calls like the
MCP gateway's audit log. Its own tranche, after the server.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | **done 2026-10-31**: `src/a2a/` — model (`Message`, `Part` exactly-one-of text/raw/url/data, `Task`, `TaskStatus`, `Artifact`, `TaskState`, ProtoJSON out, parse in via `json_lite`, now depth-capped at 128), `AgentCard`/`Skill` (card URL from Host or `.url`), `A2aServer` (`on_message`, `at`, `wrap`), `SendMessage` (message or stored task, continuing interrupted tasks, terminal → -32004, panic → failed task), `GetTask` (`historyLength`), and pulled forward from tranche 2: `ListTasks` (filters, paging), `CancelTask`; `A2A-Version` header or query (major.minor = 1.0, else -32009), errors -32001…-32009 + JSON-RPC codes with ErrorInfo data; streaming/push/extended card answer -32004/-32003. Tests: 6 (card, message/task/GetTask, input-required flow, list/cancel, errors/versions, parts round trip + fallthrough); fuzz `a2a_request`; docs `mcp/a2a.md` | done |
| 2 | **done 2026-10-31**: `a2a::tasks` — `Shared` (Mutex store + Condvar), `TaskHandle` (`working`, `artifact`, `complete[_with]`, `fail`, `input_required`, `is_canceled`, `snapshot`; updates after a terminal state ignored), `ctx.start()` + `A2aReply::Running`; `SendMessage` waits (Condvar) only for running tasks until terminal/interrupted or `blocking_timeout` (300 s), `returnImmediately`; `SendStreamingMessage`/`SubscribeToTask` via `SseStream` (task first, then statusUpdate/artifactUpdate; ends at terminal or, for SendStreamingMessage, interrupted; one message event for direct replies; subscribe to a terminal task → -32004; full/closed subscriber dropped; 16 per task); `CancelTask` through the same path (flag for workers); `require_bearer[_tokens]` (401 + WWW-Authenticate, identity = task owner, constant-time compare), card `capabilities.streaming: true` and `securitySchemes` `{bearer:{httpAuthSecurityScheme:{scheme:Bearer}}}` + `securityRequirements` (shapes from `specification/a2a.proto`); eviction prefers finished tasks. Tests: +3 (waiting/returnImmediately/timeout/cancel, streams incl. subscribe, auth + per-caller isolation + card) | done |
| 3 | **done 2026-10-31**: `a2a::push` — `PushConfig` (from `TaskPushNotificationConfig` JSON; returned without credentials; control characters refused in token/credentials, scheme a token), `push_notifications(prefixes)` allow-list (http(s), prefix match, no whitespace), one `rws-a2a-push` thread per server, queue 1024 (full → dropped + logged), 3 attempts (0.5 s, 2 s), 10 s timeout, `max_redirects(0)`; body = StreamResponse, `Content-Type: application/a2a+json`, `Authorization: <scheme> <credentials>`, `X-A2A-Notification-Token` (header name not in the spec text; SDK convention). Create/Get/List/Delete (idempotent), ≤ 10 per task, `SendMessage` `configuration.taskPushNotificationConfig` (checked before the handler; the task's current state delivered first). `extended_card(card)` + `GetExtendedAgentCard` for authenticated callers; not declared → -32004 (spec: -32007 only when declared but unset, which can't happen here). Tests: +2 (deliveries in order with headers, CRUD, SSRF refusals, push off; extended card) | done |
| 4 | **done 2026-11-01**: `examples/a2a_tck.rs` (the kit's scenarios by messageId prefix; loopback + example.com webhooks allowed for the run only), `ci/e2e/a2a_tck.sh` + workflow `a2a-tck.yml` (kit pinned at d4d6fae, JSON-RPC, JUnit compared with `ci/a2a-tck-baseline.txt`, reports uploaded). Runs: 27/30 → found `/a2a/` 404 (clients join "/" to the card URL), card ETag/Last-Modified/304, ListTasks order + includeArtifacts → 1 → snake_case proto field names (ProtoJSON) → green; push on → `http_client` used only the first resolved address (localhost → ::1) and continued `A2aReply::Task` didn't notify → **jsonrpc 94/101, 0 failed, 7 skipped** (extended card ×2, required-extension, content-type error, can't-trigger cases), baseline empty. `TaskHandle::artifact_chunk` added for chunked artifacts | done |
| 5 | **done 2026-11-01**: `proxy_config::a2a_gateway` — `[route.action] type = "a2a"` (`upstream`, `timeout_ms` 300 s, `audit_log`, `headers_env`): upstream card fetched (no redirects, configured headers) and cached 60 s; served at `<path>/.well-known/agent-card.json` with one JSON-RPC interface = this route (other bindings and the agent's address hidden); `POST <path>` forwarded through `proxy::forward_upstream_trusting` to the card's JSON-RPC URL (streams pass through, `Proxy-Status`), caller `Authorization`/`Host` replaced; audit line `a2a_call` (client, method, upstream, status, duration); unreachable/odd card → 503 + Retry-After; validation + unknown keys. Auth/rate limits come from the route's middleware. Tests: 2 (card rewrite + forwarding with a stand-in agent, header swap; unreachable + config errors) | done |
| 6 | Docs (`ai/a2a.md`), starter or example, CHANGELOG, release 18.18.0 | — |
