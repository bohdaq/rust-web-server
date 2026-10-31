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
Push notifications and the extended card aren't served yet; the card says
so (`capabilities`), and their methods answer `-32003`/`-32004`.

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

## History and limits

The task's history keeps every message in order. A handler that panics
fails its task instead of the request. At most 10,000 tasks are kept (the
oldest finished ones go first), 16 streams per task, 4 MiB per request.
