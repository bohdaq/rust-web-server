---
title: Server-Sent Events
description: Push a stream of typed events to browser clients over a single HTTP connection with the Sse builder.
---

## Quick start

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

## `Sse` builder

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

Newlines in the `data` string are split into separate `data:` lines per the SSE specification. The client concatenates them with `\n`:

```rust
SseEvent::data("line one\nline two\nline three")
// Wire: data: line one\ndata: line two\ndata: line three\n\n
```

## Use case: streaming AI tokens

For live token-by-token output where events arrive over time, write the SSE headers and raw event lines directly to the TCP stream. The buffered `Sse` builder is for pre-known event sequences; live streaming requires a custom accept loop:

```rust
// 1. Emit SSE headers
response.headers.push(Header::new("Content-Type", "text/event-stream"));
response.headers.push(Header::new("Cache-Control", "no-cache"));
response.headers.push(Header::new("X-Accel-Buffering", "no"));
write_headers(&mut stream, &response);

// 2. Loop over model output and push each token
for token in model_stream {
    let event = SseEvent::data(&token).encode();
    stream.write_all(&event)?;
    stream.flush()?;
}
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

:::note[Buffered vs. live streaming]
`Sse::into_response()` buffers all events before writing. It suits pre-known sequences like progress steps or batch pushes. For continuous output (AI tokens, sensor feeds) write raw SSE frames directly to the open TCP connection instead.
:::
