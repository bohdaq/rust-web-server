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

## Cleartext HTTP/2 (h2c)

Without TLS, a client can still speak HTTP/2 by sending the HTTP/2
connection preface straight away ("prior knowledge") — gRPC clients and
`grpcurl -plaintext` do. Since 18.3.0 rws's plain-HTTP listener recognises
the preface and serves the connection as HTTP/2; anything else is HTTP/1.1
as before. The `Upgrade: h2c` handshake (removed from HTTP/2 in RFC 9113)
isn't supported.

```bash
curl --http2-prior-knowledge http://localhost:7878/healthz
grpcurl -plaintext -proto echo.proto localhost:7878 echo.Echo/Unary
```

## Request pipeline

`rws` has its own HTTP/2 server (`h2_server`, since 18.0.0 — the `h2` crate is
now only the HTTP/2 client of the proxies). Per connection it:

1. Exchanges the connection preface and SETTINGS (at most 250 concurrent
   streams, header lists up to 64 KiB — larger ones get `431`).
2. Reads frames, decodes HPACK header blocks and assembles each stream's
   `Request` (body included), validating it as RFC 9113 §8.3 requires.
3. Calls your `Application` — inline for a quick bodiless request the app
   says it can answer inline (`runs_inline`, as on HTTP/1.1), otherwise in its
   own task.
4. Encodes the `Response` (headers seen before become one-byte HPACK indexes)
   and sends it within the client's flow-control windows; file and streamed
   bodies go out in chunks. Everything ready at once is written in one go.

It passes h2spec's 146 conformance tests and is fuzzed (frames and HPACK).
Abusive clients are cut off: rapid stream resets, CONTINUATION floods and
oversized frames end the connection with `GOAWAY`.

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
    .get("/data", |_req, _params, _conn, state| async move {
        let rows: Vec<u8> = state.db.fetch_all_json().await;
        Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, rows)
    });
```

:::note[SNI in HTTP/2]
The SNI hostname is extracted from the TLS handshake and available in
`ConnectionInfo::sni_hostname` for every HTTP/2 request, the same as for
HTTP/1.1 TLS connections.
:::

## Limits and conformance

Each HTTP/2 connection advertises `SETTINGS_MAX_CONCURRENT_STREAMS = 250`
(the same limit as Go's `net/http`), so one client can't open unbounded
simultaneous streams, and `SETTINGS_MAX_HEADER_LIST_SIZE = 64 KiB`, so one
request can't make the server buffer megabytes of headers (HTTP/3 uses the
same 64 KiB field-section limit; an HTTP/1.1 request head is capped at
64 KiB, else `431`). CI runs [h2spec](https://github.com/summerwind/h2spec)
against a live server on every commit and requires all 146 tests to pass
with none skipped (`ci/e2e/h2spec.sh`).
