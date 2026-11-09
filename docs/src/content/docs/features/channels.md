---
title: Channels
description: Publish messages to named topics and push them to browsers over SSE or WebSocket with channels::Hub.
---

A `channels::Hub` holds named topics. Pages subscribe to a topic over [SSE](/features/sse/) or [WebSocket](/features/websocket/); server code publishes to it — "a comment was added", "the import finished" (18.35.0):

```rust
use rust_web_server::app::App;
use rust_web_server::channels::Hub;

let hub = Hub::new();
let app = App::with_state(State { hub: hub.clone(), db })
    // EventSource("/posts/7/events").addEventListener("post-7", …)
    .get("/posts/:id/events", |_, params, _, state| {
        state.hub.sse(&format!("post-{}", params.get("id").unwrap_or("")))
    })
    // A chat room: every client gets what any of them sends.
    .websocket("/rooms/:room", |ws, params, state| {
        let room = params.get("room").unwrap_or("lobby").to_string();
        state.hub.serve_websocket(ws, &room, |hub, room, text| { hub.publish(room, text); });
    })
    .post("/posts/:id/comments", |request, params, _, state| {
        // … save the comment …
        state.hub.publish(&format!("post-{}", params.get("id").unwrap_or("")), "comment added");
        Response::text(STATUS_CODE_REASON_PHRASE.n201_created, "ok")
    });
```

| Method | What it does |
|---|---|
| `publish(topic, message)` | Sends to every subscriber; returns how many it reached. SSE clients get an event named after the topic, WebSocket clients a text message |
| `sse(topic)` | A streaming SSE response subscribed to `topic` — return it from the handler |
| `serve_websocket(ws, topic, on_message)` | Subscribes a WebSocket connection and reads it until it closes, calling `on_message(hub, topic, text)` for each text message |
| `subscribe_websocket(topic, sender)` | Subscribes a `WebSocketSender` you manage yourself |
| `subscribe(topic)` | An in-process `Receiver<String>` (a worker thread, a test) |
| `subscribers(topic)`, `topics()` | What's subscribed |

A `Hub` is cheap to clone; all clones share the topics. Delivery is best effort: a message that doesn't fit a slow SSE client's buffer is dropped for that client, and clients that have gone away are removed on the next publish to their topic. WebSocket messages are written by the publishing thread.

Channels are in-process: each replica has its own subscribers. Behind several replicas, publish where every replica hears it — consume a queue (Kafka, Redis) in each and call `publish` from there.
