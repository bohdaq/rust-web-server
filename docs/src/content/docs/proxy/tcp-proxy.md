---
title: TCP Proxy
description: Forward raw TCP connections to backend servers with round-robin load balancing and no protocol overhead.
---

`TcpProxy` is a Layer-4 reverse proxy. It accepts raw TCP connections and tunnels bytes bidirectionally to a backend server. Because it operates below the HTTP layer, it is protocol-agnostic: database wire formats, raw TLS passthrough, custom binary protocols, and any other TCP-based traffic all flow through unchanged.

## Basic usage

```rust
use rust_web_server::tcp_proxy::TcpProxy;

// Proxy raw TCP on port 5432 across two PostgreSQL backends
TcpProxy::new(["backend-1:5432", "backend-2:5432"])
    .connect_timeout_ms(3_000)
    .bind("0.0.0.0:5432")
    .unwrap();
```

`.bind()` blocks the calling thread indefinitely. Run it in a dedicated thread or process.

## Round-robin selection

Backend selection uses an `AtomicUsize` counter that increments on every accepted connection. The counter wraps around the backend list, distributing connections evenly with no locking:

```rust
TcpProxy::new([
    "db-primary:5432",
    "db-replica-1:5432",
    "db-replica-2:5432",
])
```

## Relay model

Each accepted connection spawns a new OS thread that calls `relay()`. Inside `relay()`, two more threads are spawned — one per direction — each running `std::io::copy` in a loop:

```
client ──► thread A: io::copy(client → backend) ──► backend
client ◄── thread B: io::copy(backend → client) ◄── backend
```

When either side closes its half of the connection (`shutdown(Write)`), the other direction drains and the relay thread exits cleanly.

## Timeouts and options

```rust
TcpProxy::new(backends)
    .connect_timeout_ms(5_000)  // TCP connect to backend (default: 5 000 ms)
```

There is no read timeout at the L4 level — idle connections are limited only by the OS TCP keepalive settings. Configure `net.ipv4.tcp_keepalive_*` on the host if you need to reclaim stuck connections.

## No TLS termination

`TcpProxy` forwards raw bytes. If the client sends a TLS `ClientHello`, that handshake is relayed to the backend as-is. The backend must speak TLS directly. For TLS termination before proxying, use an rws instance configured with `--tls-cert-file` in front of `TcpProxy`.

## Config-file usage

Activate `TcpProxy` from `rws.config.toml` using a `[[tcp_proxy]]` section:

```toml
[[tcp_proxy]]
bind = "0.0.0.0:5432"
backends = ["db-primary:5432", "db-replica:5432"]
connect_timeout_ms = 3000
```

Multiple `[[tcp_proxy]]` sections are supported — each starts a separate listener on its own thread.

## Use cases

| Protocol | Typical port |
|----------|-------------|
| PostgreSQL | 5432 |
| MySQL / MariaDB | 3306 |
| Redis | 6379 |
| SMTP | 25 / 587 |
| IMAP | 143 / 993 |
| Raw TLS passthrough | any |
| Custom binary RPC | any |

:::note[Connection count]
Each live connection holds two threads plus two socket file descriptors on the rws process. Size the thread pool and OS `ulimit -n` accordingly for high-connection workloads.
:::
