---
title: WebSocket Proxy
description: Proxy WebSocket upgrade requests to backend servers and relay all frames bidirectionally with round-robin load balancing.
---

`WsProxy` listens for incoming HTTP/1.1 connections, validates that each connection is a WebSocket upgrade, forwards the upgrade handshake to a backend, and then tunnels all subsequent WebSocket frames bidirectionally.

## Basic usage

```rust
use rust_web_server::ws_proxy::WsProxy;

// Proxy all WebSocket connections on port 8080 to two chat backends
WsProxy::new(["chat-backend:9000", "chat-backend:9001"])
    .connect_timeout_ms(3_000)
    .bind("0.0.0.0:8080")
    .unwrap();
```

`.bind()` blocks the calling thread indefinitely. Run it in a dedicated thread or process.

## Connection flow

For each incoming TCP connection, `WsProxy` follows this sequence:

1. **Read** the initial HTTP request (up to 8 192 bytes).
2. **Validate** that the request contains a WebSocket `Upgrade` header. Non-upgrade requests receive `400 Bad Request` and are closed.
3. **Connect** to the selected backend using the configured connect timeout.
4. **Forward** the HTTP upgrade request to the backend (rewriting the `Host` header).
5. **Verify** the backend responds with `HTTP/1.1 101 Switching Protocols`.
6. **Send** `101 Switching Protocols` to the client, completing the client-side handshake.
7. **Relay** all subsequent bytes bidirectionally in two threads (one per direction).

The relay threads use `std::io::copy` and shut down their half of the connection when the remote side closes.

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
[[ws_proxy]]
bind = "0.0.0.0:8080"
backends = ["ws-backend-1:9000", "ws-backend-2:9000"]
connect_timeout_ms = 3000
```

Multiple `[[ws_proxy]]` sections can run independently on separate ports.

## TLS WebSocket (wss://)

`WsProxy` accepts plain HTTP/1.1 only. To proxy `wss://` clients, place an rws instance configured with TLS (`--tls-cert-file`) in front and point its downstream at this proxy:

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
