---
title: Server-Sent Events
description: Stream events to browsers as they happen with Sse::stream, or send a known sequence with the Sse builder.
---

## Streaming

`Sse::stream()` gives a handler the response to return and an `SseSender`
that pushes events to the client as they are sent — AI tokens, progress,
live updates — over HTTP/1.1, HTTP/2 and HTTP/3 (since 18.5.0):

```rust
use rust_web_server::app::App;
use rust_web_server::sse::{Sse, SseEvent};

let app = App::with_state(()).get("/tokens", |_req, _params, _conn, _state| {
    let (events, response) = Sse::stream();
    std::thread::spawn(move || {
        for (n, token) in ["Hello", ",", " world"].iter().enumerate() {
            let event = SseEvent::data(*token).id(n.to_string()).event_type("token");
            if events.send(event).is_err() {
                return; // the client went away
            }
        }
        // Dropping the last sender ends the stream.
    });
    response
});
```

| `SseSender` | |
|---|---|
| `.send(event)` | waits while the buffer is full: a slow client slows the sender, memory stays bounded |
| `.try_send(event)` | `Ok(false)` and the event dropped when the buffer is full |
| `.send_async(event).await` | `send` for async handlers (`http2` builds) |
| `.data(text)`, `.comment(text)` | shortcuts |
| `.clone()` | several producers; the stream ends when the last clone is dropped |

Every method returns `Err(SseClosed)` once the client is gone. When nothing
is sent for 15 seconds a `: keep-alive` comment goes out, which keeps
proxies from timing the connection out and notices a client that left. To
change the buffer (32 events) or the interval:

```rust
use std::time::Duration;
use rust_web_server::sse::SseStream;

let (events, response) = SseStream::new().buffer(256).keep_alive(Duration::from_secs(5)).start();
```

The MCP server's `GET /mcp` stream is built on the same writer.
`examples/realtime.rs` serves one, and `ci/e2e/realtime.sh` checks that its
events arrive as they are sent, over plain HTTP, TLS and HTTP/2.

## Buffered

For a sequence known up front, `Sse` builds the whole body at once:

```rust
use rust_web_server::sse::{Sse, SseEvent};

fn handle_updates(_req: &Request, _conn: &ConnectionInfo) -> Response {
    Sse::new()
        .event("connected", "ready")
        .push(SseEvent::data(r#"{"count":1}"#).id("1").event_type("update"))
        .push(SseEvent::data(r#"{"count":2}"#).id("2").event_type("update"))
        .comment("keep-alive")
        .into_response()
}
```

### `Sse` builder

`Sse::new()` creates an empty builder. Every method takes `self` and returns `Self` for chaining. Call `.into_response()` at the end to get a `Response`.

| Method | Wire output |
|---|---|
| `.event(event_type, data)` | `event: <type>\ndata: <data>\n\n` |
| `.data(data)` | `data: <data>\n\n` (no `event:` field — client sees `"message"`) |
| `.push(SseEvent)` | fully configured event |
| `.retry(ms)` | `retry: <ms>\n\n` — overrides client reconnect timer |
| `.comment(text)` | `: <text>\n\n` — ignored by browsers, used as keep-alive ping |

`.into_response()` sets:

- `200 OK`
- `Content-Type: text/event-stream`
- `Cache-Control: no-cache`
- `X-Accel-Buffering: no` — disables nginx proxy buffering

## `SseEvent`

Build with `SseEvent::data(payload)` and chain optional setters:

```rust
let event = SseEvent::data(r#"{"token":"Hello"}"#)
    .id("42")
    .event_type("token")
    .retry(3000);
```

| Field | Type | Meaning |
|---|---|---|
| `data` | `String` (required) | Payload; multi-line strings split into multiple `data:` lines |
| `id` | `Option<String>` | Event ID; client sends `Last-Event-ID` on reconnect |
| `event_type` | `Option<String>` | Event type; client listens with `addEventListener("type", fn)` |
| `retry_ms` | `Option<u32>` | Reconnect delay in milliseconds |

### Multi-line data

Line breaks in the `data` string (`\n`, `\r\n` or a lone `\r`) are split into separate `data:` lines per the SSE specification; the client joins them with `\n`. Line breaks in `id` and the event type are dropped, so a value can't add fields of its own:

```rust
SseEvent::data("line one\nline two\nline three")
// Wire: data: line one\ndata: line two\ndata: line three\n\n
```

## Client-side JavaScript

```js
const source = new EventSource("/updates");

// Default handler — fires for events with no `event:` type field
source.onmessage = (e) => {
    console.log("message:", e.data);
};

// Named event handler
source.addEventListener("update", (e) => {
    const payload = JSON.parse(e.data);
    console.log("update count:", payload.count);
});

// Reconnection — browser retries automatically; Last-Event-ID is sent
source.onerror = () => console.warn("SSE connection lost, retrying…");
```
