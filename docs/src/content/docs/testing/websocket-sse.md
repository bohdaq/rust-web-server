---
title: WebSocket and SSE in tests
description: Drive a WebSocket route's handler and read a Server-Sent Events stream from TestClient, with no socket.
---

`TestClient` drives two kinds of long-lived responses in-process (since 18.29.0): a WebSocket route's upgrade handler, over an in-memory duplex, and a `text/event-stream` body, read as it arrives. Both wait with timeouts, so a handler that never answers fails the test instead of hanging it.

## WebSocket

`client.websocket(path).connect()` sends the opening handshake through the app. When the route answers with `WebSocket::upgrade`'s `101`, the handler runs on a thread of its own — the same handler the server would run — and you get a `TestWebSocket`:

```rust
use rust_web_server::app::App;
use rust_web_server::test_client::TestClient;
use rust_web_server::websocket::{Message, WebSocket};

#[test]
fn echo() {
    let app = App::with_state(()).get("/echo", |request, _params, _conn, _state| {
        WebSocket::upgrade(request, |mut ws| {
            while let Ok(Some(Message::Text(text))) = ws.recv() {
                let _ = ws.send_text(&text);
            }
        })
    });
    let client = TestClient::new(app);

    let mut ws = client.websocket("/echo").connect().expect("a 101");
    ws.send_text("hi");
    assert_eq!("hi", ws.recv_text());
    assert_eq!(Some(1000), ws.close(1000, "done")); // the handler's close code
}
```

Anything but an upgrade comes back as the `Err` response, so refusals are testable too:

```rust
let refused = client.websocket("/admin/live").connect().err().expect("refused");
assert_eq!(401, refused.status());
```

| Method | What it does |
|---|---|
| `.header(name, value)` | add a handshake header (`Authorization`, `Cookie`, `Sec-WebSocket-Protocol`, …) |
| `.timeout(d)` | how long `recv`/`close` wait for the handler (default 5 s) |
| `send_text` / `send_binary` | send a message, masked as a browser does |
| `ping(payload)` / `send(frame)` | a ping, or any `Frame` |
| `send_raw(bytes)` | bytes as they are, to test protocol errors (an unmasked frame gets close code 1002) |
| `recv()` | `Ok(Some(Message))`; `Ok(None)` once the handler sent a close (answered for you); `Err` on a timeout or a handler that returned without closing. Pings are answered and skipped, fragments joined |
| `recv_text()` / `recv_binary()` | the next message, panicking if it's anything else |
| `recv_frame()` | the next raw frame (pings and pongs included) |
| `close_code()` | the handler's close code, once it closed |
| `close(code, reason)` | the closing handshake; waits for the handler to return, returns its close code |
| `handler_finished(timeout)` | whether the handler returned |
| `handshake()` | the `101` response and its headers |

Dropping a `TestWebSocket` ends the connection: the handler's `recv` returns an error. A handler that reaches for `tokio::runtime::Handle::current()` finds the test's runtime when the test is `#[tokio::test]`.

## Server-Sent Events

`TestResponse::sse()` reads a `text/event-stream` body — streamed by `Sse::stream` (`stream_pipe`) as it arrives, or buffered by `Sse::into_response` at once — and parses it into `SseFrame { event, data, id, retry }`:

```rust
use std::time::Duration;
use rust_web_server::app::App;
use rust_web_server::sse::{Sse, SseEvent};
use rust_web_server::test_client::TestClient;

#[test]
fn ticks() {
    let app = App::with_state(()).get("/ticks", |_req, _params, _conn, _state| {
        let (events, response) = Sse::stream();
        std::thread::spawn(move || {
            for n in 1..=3 {
                let _ = events.send(SseEvent::data(n.to_string()).event_type("tick").id(n.to_string()));
            }
        });
        response
    });
    let client = TestClient::new(app);

    let events = client.get("/ticks").send().sse_events(3, Duration::from_secs(5));
    assert_eq!(3, events.len());
    assert_eq!("tick", events[0].event);
    assert_eq!("1", events[0].data);
    assert_eq!(Some("1"), events[0].id.as_deref());
}
```

`sse_events(n, timeout)` returns up to `n` events within `timeout` in all — fewer when the stream ends or time runs out, so assert on the count. For finer control keep the `TestSse` from `.sse()`:

| Method | What it does |
|---|---|
| `next_event(timeout)` | the next event, or `None` (stream ended, or timed out) |
| `events(n, timeout)` | up to `n` events |
| `collect_until_end(timeout)` | every event until the stream ends (waits the whole `timeout` if it never does) |
| `is_ended()` / `error()` | whether the stream ended, and why if it was an error |
| `comments()` | comment lines seen (keep-alives) |

Parsing follows the HTML Standard's rules: lines end in CRLF, LF or CR; `data` lines join with `\n`; the event type defaults to `message`; a block without `data` isn't dispatched; `retry` must be digits. `SseParser` is the same parser on its own (`parser.feed(bytes)` returns the events those bytes completed). It refuses an event or line over 8 MiB (`sse::MAX_EVENT_BYTES`) instead of buffering without bound.

A streamed body is read on a thread of its own; it ends with the stream, or at the first read after the `TestSse` is dropped (for `Sse::stream`, the next event or keep-alive comment). `TestResponse::is_streamed()` tells whether a response has a streamed body, and `take_stream()` hands you its reader for anything that isn't SSE.
