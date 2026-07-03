---
title: WebSocket Proxy
description: Proxy WebSocket upgrade requests to backend servers and relay all frames bidirectionally with round-robin load balancing.
---

`WsProxy` listens for incoming HTTP/1.1 connections, validates that each connection is a WebSocket upgrade, forwards the upgrade handshake to a backend, and then tunnels all subsequent WebSocket frames bidirectionally.

## Basic usage

```rust
use rust_web_server::ws_proxy::WsProxy;

// Plain backends, round-robin
WsProxy::new(["ws://chat-backend:9000", "ws://chat-backend:9001"])
    .connect_timeout_ms(3_000)
    .bind("0.0.0.0:8080")
    .unwrap();

// TLS backend (requires http-client or http2 feature)
WsProxy::new(["wss://chat.example.com"])
    .bind("0.0.0.0:8080")
    .unwrap();
```

`.bind()` blocks the calling thread indefinitely. Run it in a dedicated thread or process.

## Connection flow

For each incoming TCP connection, `WsProxy` follows this sequence:

1. **Read** the initial HTTP request (up to 8 192 bytes).
2. **Validate** that the request contains a WebSocket `Upgrade` header. Non-upgrade requests receive `400 Bad Request` and are closed.
3. **Connect** to the selected backend using the configured connect timeout.
4. **TLS handshake** (wss:// only) — establishes a `rustls` connection using the WebPKI root store.
5. **Forward** the HTTP upgrade request to the backend (rewriting the `Host` header).
6. **Verify** the backend responds with `HTTP/1.1 101 Switching Protocols`.
7. **Send** `101 Switching Protocols` to the client, completing the client-side handshake.
8. **Relay** all subsequent bytes bidirectionally — two threads for `ws://`, single-thread polling loop for `wss://`.

For plain backends, relay threads use `std::io::copy` and shut down their half of the connection when the remote side closes.

## Round-robin backend selection

An `AtomicUsize` counter cycles through backends evenly across connections with no locking:

```rust
WsProxy::new([
    "ws-backend-1:9000",
    "ws-backend-2:9000",
    "ws-backend-3:9000",
])
```

## Timeout configuration

```rust
WsProxy::new(backends)
    .connect_timeout_ms(5_000)    // TCP connect to backend (default: 5 000 ms)
    .read_timeout_ms(30_000)      // idle read timeout on client connections (default: 30 000 ms)
```

The read timeout applies to the initial HTTP upgrade read from the client. After the tunnel is established, there is no application-level read timeout — the connection stays open until either side closes it.

## Config-file usage

Activate `WsProxy` from `rws.config.toml` with a `[[ws_proxy]]` section:

```toml
# Plain TCP backends
[[ws_proxy]]
bind = "0.0.0.0:8080"
backends = ["ws://ws-backend-1:9000", "ws://ws-backend-2:9000"]
connect_timeout_ms = 3000

# TLS backend (requires http-client or http2 feature)
[[ws_proxy]]
bind = "0.0.0.0:8443"
backends = ["wss://chat.example.com"]
```

Multiple `[[ws_proxy]]` sections can run independently on separate ports.

## Backend URL schemes

`WsProxy` accepts three backend URL forms:

| Scheme | Transport | Default port |
|--------|-----------|--------------|
| `host:port` | plain TCP | (explicit) |
| `ws://host:port` | plain TCP | 80 |
| `wss://host:port` | TLS | 443 |

```rust
// Plain backends — all three forms are equivalent for port 9000:
WsProxy::new(["chat:9000", "ws://chat:9000"])

// TLS backends (port defaults to 443):
WsProxy::new(["wss://chat.example.com"])
WsProxy::new(["wss://chat.example.com:8443"])
```

`wss://` requires the `http-client` or `http2` Cargo feature (both include `rustls` + `webpki-roots`). Without these features, `wss://` backends return `502 Bad Gateway` with a clear error message.

## TLS relay internals

Plain `ws://` backends use two threads — one per direction — each running `std::io::copy`. This is the most efficient approach for plain TCP because `TcpStream::try_clone()` lets each thread independently own one direction.

`wss://` backends cannot use this pattern because `rustls::StreamOwned` cannot be cloned. Sharing it between two threads via `Arc<Mutex<>>` would deadlock: the read thread holds the mutex while blocking for data, preventing the write thread from ever sending client data to the backend.

Instead, `wss://` uses a **single-thread polling relay**:
- Both streams are set to a 5 ms read timeout.
- The loop reads from the client, writes to the TLS backend; then reads from the TLS backend, writes to the client.
- When neither side has data, the loop sleeps 1 ms to avoid busy-waiting.

For interactive WebSocket applications (chat, games, collaborative editing), the maximum added latency from the polling interval is imperceptible.

## Inbound TLS (clients connecting over wss://)

The above covers TLS to the *upstream backend*. For TLS-terminated inbound connections (clients using `wss://` to reach the proxy itself), place an rws instance with `--tls-cert-file` in front:

```
wss:// client ──► rws (TLS termination, port 443) ──► WsProxy (plain, port 8080) ──► backend
```

## Use cases

- Chat servers and real-time messaging (Socket.IO, Phoenix Channels, Action Cable)
- Live data feeds and dashboards
- Multiplayer game backends
- Collaborative editing services
- Any service that upgrades HTTP to the WebSocket protocol

:::note[One backend per connection]
Backend selection happens once at connection time, not per-frame. All frames for a given WebSocket session are routed to the same backend for the lifetime of that connection.
:::
