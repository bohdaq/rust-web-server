---
title: Events
description: A typed in-process event bus — publish values, subscribe by type, forward to MCP notifications or channels.
---

`events::EventBus` decouples the code that does something from the code that reacts to it: publish an event value, and every subscriber to its type runs (18.35.0).

```rust
use rust_web_server::events::EventBus;

#[derive(Clone)]
struct OrderPlaced { id: i64, email: String }

let bus = EventBus::new();
bus.subscribe(|e: &OrderPlaced| println!("send the receipt for {} to {}", e.id, e.email));
bus.subscribe(|e: &OrderPlaced| println!("update stock for {}", e.id));

// In the handler that places the order:
bus.publish(&OrderPlaced { id: 7, email: "ann@example.com".into() });
```

| Method | What it does |
|---|---|
| `subscribe(\|e: &E\| …)` | Runs the closure for every `E` published from now on; returns a `SubscriptionId` |
| `unsubscribe(id)` | Stops one subscription |
| `publish(&event)` | Runs the subscribers of the event's type in subscription order; returns how many completed |
| `subscribers::<E>()` | How many there are |
| `forward_to_mcp(server, method, \|e\| json)` | Sends each `E` to MCP clients as a notification (`McpServer::notify`) |
| `forward_to_channel(hub, \|e\| topic, \|e\| message)` | Publishes each `E` to a [channel](/features/channels/) topic — live updates in pages |

Subscribers run on the publishing thread. One that panics is skipped and reported on stderr; the others still run. Keep them quick: anything slow (sending mail, calling an API) belongs in a [job](/features/jobs/) the subscriber enqueues. A bus is cheap to clone and all clones share the subscribers; put one in the app state. Events stay in the process — across replicas, use a queue.
