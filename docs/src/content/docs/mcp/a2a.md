---
title: A2A agents
description: Serve an agent other AI agents can call over the Agent2Agent (A2A) 1.0 protocol, with an Agent Card, messages and tasks.
---

[Agent2Agent (A2A)](https://a2a-protocol.org/latest/specification/) lets one
AI agent call another: it reads the other's **Agent Card**, sends it a
**message**, and follows the **task** that starts. MCP connects an agent to
tools; A2A connects agents to agents. `A2aServer` (since 18.17.0) serves the
A2A 1.0 JSON-RPC binding; you write what the agent does.

```rust,no_run
use rust_web_server::a2a::{A2aReply, A2aServer, AgentCard, Artifact, Message, Skill};
use rust_web_server::app::App;
use rust_web_server::core::New;

let agent = A2aServer::new(
    AgentCard::new("Invoice agent", "Reads invoices and answers questions about them", "1.0.0")
        .skill(Skill::new("summarize", "Summarize an invoice", "Totals, due date, line items").tag("invoices")),
)
.on_message(|ctx, message| {
    let question = message.text();
    if question.is_empty() {
        return A2aReply::Message(Message::agent_text("Which invoice?"));
    }
    if ctx.task.is_none() && !question.contains("INV-") {
        // Ask back; the client's next message carries this task's id.
        return A2aReply::Task(ctx.input_required("Which invoice number?"));
    }
    A2aReply::Task(ctx.complete(vec![Artifact::text("summary", format!("Summary for {question}"))]))
})
.wrap(App::new()); // everything that isn't A2A goes to your app
```

## What it serves

- `GET /.well-known/agent-card.json`: the card, with its name, description,
  version, skills, input and output media types (`text/plain` by default,
  `.modes(…)` to change), and the endpoint's URL (from the request's `Host`,
  or `.url("https://…")` behind a proxy).
- `POST /a2a` (`.at("/path")` to move it): JSON-RPC 2.0 with the 1.0 method
  names:

| Method | What it does |
|---|---|
| `SendMessage` | Calls your handler. An `A2aReply::Message` answers directly; an `A2aReply::Task` is stored and returned. A message with the `taskId` of a task waiting for input continues it (`ctx.task` is that task) |
| `GetTask` | A stored task, `historyLength` most recent messages |
| `ListTasks` | Tasks, newest first, by `contextId` / `status`, `pageSize` 1–100 with `pageToken` |
| `CancelTask` | Cancels a task that isn't finished (`-32002` when it is) |
| `SendStreamingMessage` | Like `SendMessage`, as server-sent events: the task first, then each `statusUpdate` and `artifactUpdate`, ending when the task finishes or asks for input (or one `message` event) |
| `SubscribeToTask` | Follows a task that isn't finished: the task first, then its updates |

Clients must send `A2A-Version: 1.0` (header or query parameter); other
versions get `-32009`. Errors use the A2A codes (`-32001` task not found,
`-32004` unsupported operation, …) with a `google.rpc.ErrorInfo` detail.
Push notifications and the extended card are off until you turn them on
(below); the card's `capabilities` say which are on, and their methods
answer `-32003` / `-32004` while off.

## Messages and tasks

A `Message` has `parts`: text, raw bytes, a URL, or JSON data
(`Part::text`, `Part::raw`, `Part::url`, `Part::data`); `message.text()`
joins the text parts. The handler's `ctx` builds the task to return:

- `ctx.complete(artifacts)`: done, with outputs (`Artifact::text(name, text)`);
- `ctx.input_required(question)`: waiting for the client's answer;
- `ctx.fail(reason)` / `ctx.reject(reason)`;
- `ctx.task(state, message, artifacts)` for anything else.

## Long-running work

`ctx.start()` stores the task (working) and gives you a `TaskHandle` to
update it from any thread; return `A2aReply::Running(handle)`:

```rust,ignore
.on_message(|ctx, message| {
    let task = ctx.start();
    let worker = task.clone();
    std::thread::spawn(move || {
        worker.working(Some("reading the invoice"));
        if worker.is_canceled() { return; }           // CancelTask was called
        worker.artifact(Artifact::text("summary", "…"));
        worker.complete();                            // or fail(reason), input_required(question)
    });
    A2aReply::Running(task)
})
```

`worker.artifact_chunk(artifact, append, last_chunk)` streams an output in
pieces: chunks with `append` add their parts to the artifact with the same
id.

`SendMessage` waits until the task finishes or asks for input (A2A's
default), up to `.blocking_timeout(…)` (300 s), then answers with its state;
a client that sends `configuration.returnImmediately: true` gets the task at
once and polls `GetTask` or streams. Streams get every update as it happens.

## Authentication

```rust,ignore
let agent = A2aServer::new(card)
    .require_bearer_tokens([("token-for-billing", "billing"), ("token-for-crm", "crm")]);
```

Requests to the JSON-RPC endpoint need `Authorization: Bearer <token>`
(`401` otherwise); the card stays public and lists the bearer scheme
(`securitySchemes`, `securityRequirements`). Each token stands for an
identity, and callers only see their own tasks. `require_bearer(token)` is
the one-token form.

## Push notifications

A client that won't hold a stream open can give a webhook: in `SendMessage`
(`configuration.taskPushNotificationConfig`) or later with
`CreateTaskPushNotificationConfig` (plus `Get…`, `List…`, `Delete…`). Each
update is POSTed to it as the same JSON a stream carries (`task`,
`statusUpdate`, `artifactUpdate`), `Content-Type: application/a2a+json`,
with `Authorization: <scheme> <credentials>` from the config's
`authentication` and its `token` as `X-A2A-Notification-Token`.

```rust,ignore
let agent = A2aServer::new(card)
    .push_notifications(["https://hooks.partner.example/", "https://ci.example.com/a2a/"]);
```

Webhook URLs come from clients, so rws calls only URLs that start with a
prefix you list (requests to anywhere else, such as `169.254.169.254` or your
internal network, are refused with `-32602`) and follows no redirects.
Deliveries go out in order from one thread, 10 s timeout, three attempts
(0.5 s and 2 s apart); at most 10 webhooks per task, and credentials are
never returned by `Get…`/`List…`.

## Extended card

```rust,ignore
let agent = A2aServer::new(public_card)
    .require_bearer_tokens([("token-for-billing", "billing")])
    .extended_card(AgentCard::new("Invoice agent", "With the internal skills", "1.0.0").skill(audit));
```

Authenticated callers get the fuller card from `GetExtendedAgentCard`; the
public card says one exists (`capabilities.extendedAgentCard`).

## Gateway

The `rws` binary can also stand in front of agents that run elsewhere (any
A2A 1.0 implementation), from `rws.config.toml`:

```toml
[[route]]
  [route.match]
  path = "/agents/billing/*"
  [route.action]
  type = "a2a"
  [route.action.a2a]
  upstream = "http://10.0.0.7:9000"
  headers_env = { Authorization = "BILLING_AGENT_AUTHORIZATION" }
  [route.middleware.auth]
  type = "bearer"
  token_env = "PARTNER_TOKEN"
```

Callers read the card at `/agents/billing/.well-known/agent-card.json`; its
only interface is JSON-RPC at `/agents/billing`, so the agent's own address
stays private. Calls are forwarded with the configured headers instead of
the caller's `Authorization`, streams pass through, and with
`audit_log = true` each call is a JSON line (client, method, status,
duration). Settings: [config file](/configuration/config-file/#routeaction--a2a).

## Conformance

CI runs the official A2A compatibility kit
([a2aproject/a2a-tck](https://github.com/a2aproject/a2a-tck), pinned) over
JSON-RPC against `examples/a2a_tck.rs`, which implements the kit's
scenarios. It passes every JSON-RPC check that applies (94 of 101; the rest are optional features the test agent leaves off); `ci/a2a-tck-baseline.txt` would list known failures and is empty.

## History and limits

The task's history keeps every message in order. A handler that panics
fails its task instead of the request. At most 10,000 tasks are kept (the
oldest finished ones go first), 16 streams per task, 4 MiB per request.
