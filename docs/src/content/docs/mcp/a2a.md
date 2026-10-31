---
title: A2A agents
description: Serve an agent other AI agents can call over the Agent2Agent (A2A) 1.0 protocol, with an Agent Card, messages and tasks.
---

[Agent2Agent (A2A)](https://a2a-protocol.org/latest/specification/) lets one
AI agent call another: it reads the other's **Agent Card**, sends it a
**message**, and follows the **task** that starts. MCP connects an agent to
tools; A2A connects agents to agents. `A2aServer` (since 18.18.0) serves the
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

Clients must send `A2A-Version: 1.0` (header or query parameter); other
versions get `-32009`. Errors use the A2A codes (`-32001` task not found,
`-32004` unsupported operation, …) with a `google.rpc.ErrorInfo` detail.
Streaming, push notifications and the extended card aren't served yet; the
card says so (`capabilities`), and their methods answer `-32004`/`-32003`.

## Messages and tasks

A `Message` has `parts`: text, raw bytes, a URL, or JSON data
(`Part::text`, `Part::raw`, `Part::url`, `Part::data`); `message.text()`
joins the text parts. The handler's `ctx` builds the task to return:

- `ctx.complete(artifacts)`: done, with outputs (`Artifact::text(name, text)`);
- `ctx.input_required(question)`: waiting for the client's answer;
- `ctx.fail(reason)` / `ctx.reject(reason)`;
- `ctx.task(state, message, artifacts)` for anything else.

The task's history keeps every message in order. A handler that panics
fails its task instead of the request. At most 10,000 tasks are kept, oldest
first out.
