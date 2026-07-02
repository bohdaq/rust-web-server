---
title: HTTP/3 / QUIC
description: The default build serves HTTP/3 over QUIC on the same port number as TCP, with automatic client negotiation.
---

## Feature requirements

HTTP/3 is included in the default build. No flags are needed:

```bash
cargo build
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem
```

HTTP/3 requires a valid TLS certificate. Without one, the QUIC listener is
silently skipped and the server continues to serve HTTP/2 and HTTP/1.1 over TLS.

## How it works

The `http3` feature (which implies `http2`) starts two listeners on the same
port number:

- A TCP listener for HTTP/2 and HTTP/1.1 (via `Server::run_tls`).
- A UDP listener for QUIC / HTTP/3 (via `Server::run_quic`).

Both share the same port. Clients that support HTTP/3 connect via UDP; others
fall back to TCP transparently. `main()` runs both listeners concurrently:

```rust
tokio::join!(
    server.run_tls(app.clone()),
    server.run_quic(app.clone()),
    server.run_redirect(app.clone()), // optional HTTP→HTTPS redirect
);
```

## Dependencies

HTTP/3 is built on the following crates:

- `quinn` — async QUIC transport
- `h3` — HTTP/3 framing over QUIC streams
- `h3-quinn` — bridge between `quinn` and `h3`

All parsing and protocol handling is contained in `src/h3_handler/mod.rs`.

## SNI from the QUIC handshake

The SNI hostname is extracted from the QUIC connection's TLS handshake data
and placed in `ConnectionInfo::sni_hostname`, exactly as it is for HTTP/2:

```rust
// src/h3_handler/mod.rs
let sni_hostname: Option<String> = conn
    .handshake_data()
    .and_then(|d| d.downcast::<quinn::crypto::rustls::HandshakeData>().ok())
    .and_then(|d| d.server_name.clone());
```

Virtual host routing via `Router::with_host()` and direct access in handlers
via `connection.sni_hostname` work the same way for HTTP/3 as for HTTP/2.

## Alt-Svc advertisement

HTTP/1.1 and HTTP/2 TLS responses include:

```
Alt-Svc: h3=":7878"
```

This header tells clients — including browsers — that HTTP/3 is available on
the same port number over UDP. No client configuration is required; browsers
such as Chrome and Firefox will upgrade to HTTP/3 automatically on subsequent
requests after seeing this header.

## Forbidden headers

Like HTTP/2, HTTP/3 (RFC 9114 §4.2) prohibits connection-level headers.
The same set is stripped automatically before sending H3 responses:

- `connection`
- `keep-alive`
- `transfer-encoding`
- `upgrade`
- `proxy-connection`
- `te`

## Request pipeline

`h3_handler::handle_connection` mirrors the HTTP/2 handler:

1. Extracts SNI from the QUIC handshake.
2. Wraps the `quinn::Connection` in an `h3::server::Connection`.
3. Resolves each request stream with `resolver.resolve_request()`.
4. Assembles a `Request`, calls `app.execute()`, applies gzip compression, and
   sends the `Response` back as H3 frames.

Your `Application` implementation is called identically for HTTP/1.1, HTTP/2,
and HTTP/3.

:::note[Firewall requirements]
HTTP/3 uses UDP. Ensure your firewall or cloud security group allows UDP
traffic on the server port (default 7878) in addition to TCP.
:::
