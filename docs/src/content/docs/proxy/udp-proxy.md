---
title: UDP Proxy
description: Forward UDP datagrams to backend servers with round-robin load balancing for DNS, syslog, and other request-reply UDP protocols.
---

`UdpProxy` receives UDP datagrams from clients and forwards each one to a backend server, then relays the backend's reply back to the original sender. This request-reply model covers protocols such as DNS, NTP, RADIUS, and syslog.

## Basic usage

```rust
use rust_web_server::udp_proxy::UdpProxy;

// Forward DNS queries round-robin across two resolvers
UdpProxy::new(["8.8.8.8:53", "8.8.4.4:53"])
    .reply_timeout_ms(2_000)
    .bind("0.0.0.0:53")
    .unwrap();
```

`.bind()` blocks the calling thread indefinitely. Run it in a dedicated thread or process.

## Per-datagram thread model

Each received datagram is handled entirely in its own OS thread so the main receive loop is never blocked waiting for a backend reply:

1. Main loop calls `recv_from()` on the bound socket.
2. A new thread is spawned for each datagram.
3. The thread creates an ephemeral `UdpSocket` bound to `0.0.0.0:0`.
4. The thread sends the datagram to the selected backend.
5. The thread waits up to `reply_timeout` for a reply.
6. The reply is sent back to the original client address via the shared socket.

No session state is kept between datagrams. Each datagram is independent.

## Round-robin backend selection

An `AtomicUsize` counter selects backends without locking, cycling through the list evenly across datagrams:

```rust
UdpProxy::new([
    "resolver-1:53",
    "resolver-2:53",
    "resolver-3:53",
])
```

## Timeouts and buffer size

```rust
UdpProxy::new(backends)
    .reply_timeout_ms(5_000)    // wait for backend reply (default: 5 000 ms)
    .buffer_size(65_536)        // per-datagram buffer bytes (default: 65 536)
```

If the backend does not reply within `reply_timeout_ms`, the datagram is silently dropped — the client receives no response for that query. This matches the behaviour of most UDP protocols, which implement their own retransmission logic.

## Config-file usage

Activate `UdpProxy` from `rws.config.toml` with a `[[udp_proxy]]` section:

```toml
[[udp_proxy]]
bind = "0.0.0.0:53"
backends = ["dns1.internal:53", "dns2.internal:53"]
read_timeout_ms = 500
```

Multiple `[[udp_proxy]]` sections are supported — each starts an independent listener.

## Use cases

| Protocol | Typical port |
|----------|-------------|
| DNS forwarding | 53 |
| NTP relay | 123 |
| syslog relay | 514 |
| SNMP proxying | 161 |
| RADIUS | 1812 / 1813 |
| DTLS passthrough | any |

:::note[No TCP fallback]
DNS responses larger than the buffer size (usually 512 bytes over plain UDP) may be truncated. The resolver will set the TC (truncation) bit; the client is then expected to retry over TCP. Use `TcpProxy` on port 53 alongside `UdpProxy` to handle both transports.
:::
