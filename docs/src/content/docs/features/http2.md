---
title: HTTP/2
description: Serve HTTP/2 and HTTP/1.1 on the same TLS port with automatic ALPN negotiation, no extra configuration required.
---

## Feature requirements

HTTP/2 support is included in the `http2` feature and the default `http3`
feature:

```bash
# HTTP/2 + TLS only (no QUIC)
cargo build --no-default-features --features http2

# Default build — also includes HTTP/3
cargo build
```

HTTP/2 is only available over TLS. A valid certificate must be configured.
See [HTTPS / TLS](/features/https-tls) for setup instructions.

## ALPN negotiation

No extra configuration is required to serve HTTP/2. The server advertises both
`h2` and `http/1.1` in the TLS handshake via ALPN. The client's preference
determines which protocol is selected for each connection:

```
alpn_protocols = ["h2", "http/1.1"]
```

Both protocols share the same port. Browsers, `curl`, and other HTTP/2-capable
clients negotiate HTTP/2 automatically.

## Request pipeline

`h2_handler::handle_connection` manages the HTTP/2 connection lifecycle:

1. Completes the HTTP/2 connection preface (`h2::server::handshake`).
2. Accepts streams in a loop (`conn.accept()`).
3. For each stream, reads H2 headers and body, assembles a `Request`, and calls
   `app.execute(&request, &connection)` — the same `Application` trait used by
   HTTP/1.1.
4. Translates the returned `Response` back into H2 response headers and a DATA
   frame, then sends them.

Your application code is unchanged between HTTP/1.1 and HTTP/2.

## Forbidden headers

The HTTP/2 specification (RFC 9113 §8.2.2) prohibits certain connection-level
headers in responses. The following headers are stripped automatically before
sending any H2 response:

- `connection`
- `keep-alive`
- `transfer-encoding`
- `upgrade`
- `proxy-connection`
- `te`

You do not need to remove these headers from `Response` objects yourself.

## Alt-Svc advertisement

When built without HTTP/3, HTTP/1.1 TLS responses include:

```
Alt-Svc: h2=":7878"
```

This tells HTTP/1.1 clients that HTTP/2 is available on the same port. With
the default `http3` build the header advertises `h3` instead.

## HTTP/2 reverse proxy

`H2ReverseProxy` provides an upstream proxy that connects to backends over
HTTP/2. It is available as middleware and wraps the normal
`Middleware` / `Application` stack:

```rust
use rust_web_server::proxy::H2ReverseProxy;

let proxy = H2ReverseProxy::new(vec!["h2://backend:8080".to_string()]);
let app = App::new().wrap(proxy);
```

`H2ReverseProxy` bridges between the synchronous middleware calling
convention and the async tokio runtime via an internal `block_on_isolated`
helper — a scoped OS thread running its own single-threaded tokio runtime,
rather than `tokio::task::block_in_place`. This works under any tokio
runtime flavor (including `current_thread`, where `block_in_place` would
panic) and even with no runtime active at all.

## Async handlers

The `http2` feature enables `App::with_async_state(S)`, which accepts `async fn`
handlers. This lets you `await` futures (database calls, outbound HTTP requests,
etc.) directly inside route handlers:

```rust
let app = App::with_async_state(my_state)
    .get("/data", |state, _req, _params, _conn| async move {
        let rows = state.db.fetch_all().await?;
        Ok(Response::json(&rows))
    });
```

:::note[SNI in HTTP/2]
The SNI hostname is extracted from the TLS handshake and available in
`ConnectionInfo::sni_hostname` for every HTTP/2 request, the same as for
HTTP/1.1 TLS connections.
:::
