---
title: WebSocket
description: Add WebSocket endpoints with RFC 6455 frame-level I/O built in — no third-party WebSocket crates required.
---

## Overview

`rust-web-server` includes a complete WebSocket implementation in
`src/websocket/mod.rs`. It covers upgrade detection, the RFC 6455 opening
handshake (SHA-1 + base64 key derivation), and frame-level read/write for all
standard opcodes.

## Integration pattern

WebSocket connections require taking over the raw TCP stream after the 101
upgrade response. They are handled in a custom accept loop rather than through
the normal `Controller` / `Application` pipeline (which has no access to the
underlying stream).

```rust
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
        let response = WebSocket::handshake_response(&request).unwrap();
        stream.write_all(&response.generate_response()).unwrap();

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

## Complete echo server example

```rust
use std::net::TcpListener;
use std::io::Read;
use rust_web_server::request::Request;
use rust_web_server::websocket::{WebSocket, Frame};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    println!("WebSocket echo server listening on ws://127.0.0.1:7878");

    for stream in listener.incoming() {
        std::thread::spawn(|| {
            let mut stream = stream.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap();

            let request = Request::parse(&buf[..n]).unwrap();
            if !WebSocket::is_upgrade_request(&request) { return; }

            let handshake = WebSocket::handshake_response(&request).unwrap();
            use std::io::Write;
            stream.write_all(&handshake.generate_response()).unwrap();

            loop {
                match WebSocket::read_frame(&mut stream) {
                    Ok(Frame::Text(msg)) => {
                        WebSocket::write_frame(&mut stream, Frame::Text(msg)).unwrap();
                    }
                    Ok(Frame::Binary(data)) => {
                        WebSocket::write_frame(&mut stream, Frame::Binary(data)).unwrap();
                    }
                    Ok(Frame::Ping(data)) => {
                        WebSocket::send_pong(&mut stream, data).unwrap();
                    }
                    Ok(Frame::Close(code, reason)) => {
                        WebSocket::send_close(&mut stream, code.unwrap_or(1000), &reason).ok();
                        break;
                    }
                    _ => break,
                }
            }
        });
    }
}
```

:::note[WebSocket proxy]
`src/ws_proxy/mod.rs` provides `WsProxy` — a standalone WebSocket proxy that
forwards upgrade requests and relays raw bytes bidirectionally between the
client and a backend server.
:::
