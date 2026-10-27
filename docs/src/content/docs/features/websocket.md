---
title: WebSocket
description: WebSocket routes with whole messages, pings and the closing handshake handled, plus RFC 6455 frame-level I/O — no third-party WebSocket crates.
---

## WebSocket routes

Since 18.5.0 a WebSocket endpoint is an ordinary route. The handler gets a
`WebSocketConnection` once the `101 Switching Protocols` response is out,
and runs on a thread of its own for the connection's life:

```rust
use rust_web_server::app::App;
use rust_web_server::websocket::Message;

let app = App::with_state(()).websocket("/chat/:room", |mut ws, params, _state| {
    let room = params.get("room").unwrap_or("lobby").to_string();
    while let Ok(Some(message)) = ws.recv() {
        let _ = match message {
            Message::Text(text) => ws.send_text(&format!("{room}: {text}")),
            Message::Binary(data) => ws.send_binary(&data),
        };
    }
});
```

`Router::websocket(pattern, |ws, params| ...)` does the same in a `Router`,
and inside any handler `WebSocket::upgrade(request, |ws| ...)` returns the
`101` response and runs the closure with the connection. A request that
isn't a WebSocket upgrade gets `426 Upgrade Required`.

`ws.recv()` returns `Ok(Some(Message::Text | Message::Binary))` per message,
`Ok(None)` after the closing handshake, and `Err` when the client
disappeared or broke the protocol. Along the way it

- joins fragmented messages (text is checked to be UTF-8 once whole),
- answers pings with pongs and drops pongs,
- answers the client's close frame, and finishes one you started with
  `ws.close(code, reason)`,
- fails the connection with the right close code: 1002 for an unmasked
  frame, reserved bits or a bad fragment, 1007 for invalid UTF-8, 1009 for
  a message over `max_message_size` (16 MiB by default).

To send while `recv` waits, take a `WebSocketSender` — cloneable, usable
from any thread:

```rust
.websocket("/ticker", |mut ws, _params, _state| {
    let sender = ws.sender();
    std::thread::spawn(move || {
        for n in 1.. {
            if sender.send_text(&format!("tick {n}")).is_err() { return; }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    while let Ok(Some(_)) = ws.recv() {}
})
```

`sender.send_text`, `send_binary`, `ping` and `close` all return `Err` once
the connection is closed.

Notes:

- HTTP/1.1 only, plain or TLS. Browsers open WebSockets over HTTP/1.1
  unless the server offers RFC 8441 (WebSocket over HTTP/2), which rws
  doesn't, so this is what they use.
- Each connection holds a thread while it's open: tokio's blocking pool
  in async builds (up to 512 by default), a new thread in the `http1`
  build.
- No idle timeout: send pings yourself if connections can go quiet
  behind a proxy that drops them.
- `Server::process` called directly, on a stream it can't hand over,
  answers an upgrade with `500`.

`examples/realtime.rs` has both routes above. `ci/e2e/realtime.sh` drives
them from Node's WebSocket client over plain HTTP and TLS, on the shared
and per-core runtimes: echo with a 400 KB message, server-pushed messages
from another thread, and clean closes started by either side.

## Frame-level API

The building blocks below are for connections you manage yourself, in a
custom accept loop:

```rust
use std::io::{Read, Write};
use std::net::TcpListener;
use rust_web_server::request::Request;
use rust_web_server::websocket::{WebSocket, Frame};

let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

for stream in listener.incoming() {
    let mut stream = stream.unwrap();

    // Read the HTTP upgrade request.
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).unwrap();
    let request = Request::parse(&buf[..n]).unwrap();

    if WebSocket::is_upgrade_request(&request) {
        // Send 101 Switching Protocols.
        let mut response = WebSocket::handshake_response(&request).unwrap();
        stream.write_all(&response.generate()).unwrap();

        // Enter the frame loop.
        loop {
            match WebSocket::read_frame(&mut stream) {
                Ok(Frame::Text(msg)) => {
                    WebSocket::write_frame(&mut stream, Frame::Text(msg)).unwrap();
                }
                Ok(Frame::Ping(data)) => {
                    WebSocket::write_frame(&mut stream, Frame::Pong(data)).unwrap();
                }
                Ok(Frame::Close(code, reason)) => {
                    WebSocket::send_close(&mut stream, code.unwrap_or(1000), &reason).ok();
                    break;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
}
```

`read_frame` returns frames as they come — fragments, pings and all; use
`WebSocketConnection` (above) for whole messages.

## Detecting upgrade requests

```rust
WebSocket::is_upgrade_request(request: &Request) -> bool
```

Returns `true` when the request has all three required headers:

- `Upgrade: websocket`
- `Connection: Upgrade` (or any value containing "upgrade")
- `Sec-WebSocket-Key: <base64>`

## Handshake response

```rust
WebSocket::handshake_response(request: &Request) -> Result<Response, String>
```

Builds an HTTP `101 Switching Protocols` response with the correct
`Sec-WebSocket-Accept` value. The accept value is computed as:

```
base64(SHA-1(client_key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))
```

Both SHA-1 and base64 are implemented from scratch in `src/websocket/mod.rs`
with no external dependencies.

If the request includes a `Sec-WebSocket-Protocol` header, the first listed
sub-protocol is echoed back in the response.

## Frame types

`Frame` is an enum covering all RFC 6455 opcodes:

| Variant | Opcode | Description |
|---|---|---|
| `Frame::Text(String)` | `0x1` | A complete UTF-8 text message |
| `Frame::Binary(Vec<u8>)` | `0x2` | A binary message |
| `Frame::Ping(Vec<u8>)` | `0x9` | Ping — respond with a matching Pong |
| `Frame::Pong(Vec<u8>)` | `0xA` | Pong |
| `Frame::Close(Option<u16>, String)` | `0x8` | Close with optional status code and reason |
| `Frame::Continuation { fin, data }` | `0x0` | Continuation fragment for fragmented messages |

## Reading frames

```rust
WebSocket::read_frame(stream: &mut impl Read) -> Result<Frame, String>
```

Reads one complete frame. Client-to-server masking (required by RFC 6455) is
handled automatically — the payload is unmasked before being returned. Extended
payload lengths (16-bit and 64-bit) are supported.

A frame whose declared payload is larger than `WebSocket::MAX_FRAME_PAYLOAD`
(16 MiB) is an error, returned before any of the payload is read, and the
payload is read as it arrives rather than allocated at the size the client
claims (since 18.1.0; before, a frame claiming an enormous length made the
process abort). For another limit:

```rust
WebSocket::read_frame_limited(stream: &mut impl Read, max_payload: u64) -> Result<Frame, String>
```

## Writing frames

```rust
WebSocket::write_frame(stream: &mut impl Write, frame: Frame) -> Result<(), String>
```

Writes one frame. Server-to-client frames are sent unmasked per the RFC.
Payload length encoding (7-bit, 16-bit, 64-bit) is chosen automatically.

## Convenience methods

```rust
// Send a text message
WebSocket::send_text(&mut stream, "hello")?;

// Send a close frame
WebSocket::send_close(&mut stream, 1000, "normal closure")?;

// Reply to a Ping with a Pong
WebSocket::send_pong(&mut stream, ping_payload)?;
```

:::note[WebSocket proxy]
`src/ws_proxy/mod.rs` provides `WsProxy` — a standalone WebSocket proxy that
forwards upgrade requests and relays raw bytes bidirectionally between the
client and a backend server.
:::
