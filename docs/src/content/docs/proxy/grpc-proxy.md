---
title: gRPC Proxy
description: Route gRPC traffic to HTTP/2 backends — every call type streaming, status trailers kept, pooled connections, private CAs and client certificates.
---

`GrpcProxy` is a middleware that forwards requests with `Content-Type: application/grpc*` to HTTP/2 backends, passing all other requests through to the next layer. It wraps `H2ReverseProxy` and adds a content-type filter so that gRPC and non-gRPC traffic can share the same application.

:::note[Feature requirement]
`GrpcProxy` and `H2ReverseProxy` require the `http2` Cargo feature (enabled by default in the `http3` build). Compile with `--features http2` or `--features http3` to include them.
:::

## Basic usage

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::GrpcProxy;

let app = App::new()
    .wrap(GrpcProxy::new(["grpc-backend:50051"]));
```

Only requests whose `Content-Type` header starts with `application/grpc` are intercepted. All other requests fall through to the inner application.

## Content-type filter

`GrpcProxy` matches the following content types (prefix match):

| Value | Example |
|-------|---------|
| `application/grpc` | bare gRPC |
| `application/grpc+proto` | Protocol Buffers encoding |
| `application/grpc+json` | JSON-encoded gRPC |
| `application/grpc-web` | gRPC-Web (forwarded as is — put [`GrpcWebLayer`](#grpc-web-from-browsers) in front to translate it) |

Any request whose `Content-Type` does not start with `application/grpc` is passed to `next.execute()` unchanged.

## Scoping to a path prefix

Use `.path_prefix()` to further narrow which gRPC services are proxied:

```rust
use rust_web_server::proxy::GrpcProxy;

// Only proxy requests to the MyService gRPC service
GrpcProxy::new(["grpc-service:50051"])
    .path_prefix("/svc.MyService")
```

Requests to other paths are passed through even if their content type is `application/grpc`.

## Round-robin load balancing

Backend selection uses an `AtomicUsize` counter that increments on every forwarded request. Multiple backends are tried in order if the first fails:

```rust
GrpcProxy::new([
    "grpc-backend-1:50051",
    "grpc-backend-2:50051",
    "grpc-backend-3:50051",
])
```

## Combining gRPC and HTTP/2 traffic

Use `GrpcProxy` together with `H2ReverseProxy` to route mixed traffic on the same port:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::{GrpcProxy, H2ReverseProxy};

let app = App::new()
    // gRPC traffic to the gRPC backend
    .wrap(GrpcProxy::new(["grpc-svc:50051"]))
    // All other HTTP/2 traffic to a separate backend
    .wrap(H2ReverseProxy::new(["api-svc:8080"]).path_prefix("/api"));
```

## TLS backends (`grpcs://`)

Pass `grpcs://` (or `https://`) backend URLs to reach gRPC services that require TLS — every managed cloud gRPC endpoint (GCP, AWS, Azure) uses TLS:

```rust
GrpcProxy::new([
    "grpcs://grpc-svc.prod.svc.cluster.local:443",
    "grpcs://grpc-svc-2.prod.svc.cluster.local:443",
])
```

When no port is given, `grpcs://` defaults to 443. You can also write `https://` — both are treated identically.

| Scheme | Transport | Default port |
|--------|-----------|--------------|
| `host:port` | plain TCP | (explicit) |
| `grpc://host:port` | plain TCP | 80 |
| `grpcs://host:port` | TLS | 443 |
| `https://host:port` | TLS | 443 |

Certificates are verified against the WebPKI roots unless you trust a CA of
your own, and a backend that requires a client certificate (mutual TLS) gets
the one you configure (both since 18.3.0, also on `H2ReverseProxy` and
`ReverseProxy`):

```rust
GrpcProxy::new(["grpcs://billing.internal:50051"])
    .tls_ca_file("/etc/rws/internal-ca.pem")?          // trust this CA instead of the public roots
    .tls_client_cert("/etc/rws/client.pem", "/etc/rws/client.key")?;  // present this certificate
```

In `rws.config.toml` they are the upstream's `tls_ca_file`,
`tls_client_cert_file` and `tls_client_key_file`.

## Streaming and trailers

Since 18.3.0 every kind of gRPC call works through the proxy:

- **Unary and server streaming** — the response head is passed on as soon as
  it arrives and each message as the backend sends it.
- **Client streaming and bidirectional** — the request body is forwarded as
  it arrives, while the response streams back, so messages cross in both
  directions on one call.
- **Status** — the backend's trailers (`grpc-status`, `grpc-message`,
  `grpc-status-details-bin`) reach the client after the last message, over
  HTTP/2 and HTTP/3; an error answered with headers only (trailers-only) keeps
  its status too. `te: trailers` is forwarded, as gRPC servers require.
- **Errors** — a backend that fails mid-call resets the client's stream
  (`INTERNAL_ERROR`), so the client sees an error rather than a truncated
  success.

HTTP/1.1 has no trailers, so gRPC needs HTTP/2 or HTTP/3 between the client
and rws (any gRPC client negotiates it).

`.read_timeout_ms()` applies to each wait for the backend, so a stream that
goes quiet for longer than that (30 s by default) is ended with an error —
raise it for long-lived streams that can be idle.

## H2ReverseProxy

`GrpcProxy` is a thin filter on top of `H2ReverseProxy`. Use `H2ReverseProxy` directly when you want to proxy all HTTP/2 requests regardless of content type:

```rust
use rust_web_server::proxy::H2ReverseProxy;

// Plain TCP backend
let app = App::new()
    .wrap(H2ReverseProxy::new(["h2://backend:8080"])
        .connect_timeout_ms(3_000)
        .read_timeout_ms(60_000));

// TLS backend (h2s:// or https://)
let app = App::new()
    .wrap(H2ReverseProxy::new(["h2s://api.example.com:443"])
        .connect_timeout_ms(3_000)
        .read_timeout_ms(60_000));
```

`.read_timeout_ms()` (default 30 s) bounds each wait for the upstream — the TLS and HTTP/2 handshakes, the response head and every body frame — so a backend that goes silent gets a `502`, while a slow response that keeps sending isn't cut off.

Supported backend URL schemes for `H2ReverseProxy`:

| Scheme | Transport | Default port |
|--------|-----------|--------------|
| `host:port` | plain TCP | (explicit) |
| `h2://host:port` | plain TCP | 80 |
| `h2s://host:port` | TLS + ALPN `h2` | 443 |
| `https://host:port` | TLS + ALPN `h2` | 443 |

## gRPC-web from browsers

Browsers can't read HTTP trailers, so they speak gRPC-web: the status
travels at the end of the body. `GrpcWebLayer` (since 18.3.0) translates it
for the gRPC backend, as Envoy's `grpc_web` filter does — put it in front of
`GrpcProxy`:

```rust
use rust_web_server::proxy::{GrpcProxy, GrpcWebLayer};

let app = App::new()
    .wrap(GrpcWebLayer::new())
    .wrap(GrpcProxy::new(["grpc-svc:50051"]));
```

- `application/grpc-web[+proto|+json]` and `application/grpc-web-text`
  (base64) requests go to the backend as `application/grpc…` with
  `te: trailers`, the `-text` body decoded as it arrives.
- The response comes back as gRPC-web: the content type renamed, the
  trailers (`grpc-status`, `grpc-message`) appended as the final frame of the
  body (flag `0x80`), base64 for `-text`. Messages stream as the backend sends
  them, over HTTP/1.1, HTTP/2 and HTTP/3.
- A trailers-only error keeps its status in the response headers.
- Plain gRPC requests pass through untouched.

The config file's `grpc` action does this on its own. A page on another
origin needs CORS: allow the `content-type`, `x-grpc-web` and
`x-user-agent` request headers and expose `grpc-status` and `grpc-message`
([CORS](/features/cors-security/)). CI checks it from headless Chromium
(`ci/e2e/grpc_web.sh`).

## Connections

Since 18.3.0 each backend gets one HTTP/2 connection, reused and multiplexed
by every request (up to the backend's stream limit) instead of a new
handshake per request; a connection the backend closes is replaced on the
next request. Requests are retried on another backend only before anything
was sent, and never when the request body was streaming.

The connections live on a small runtime of their own (two threads named
`rws-h2-upstream`), so the proxy works the same under the shared, per-core
or no tokio runtime (a test client).

Responses that aren't gRPC, up to 1 MiB and with a `content-length`, are
buffered (and keep their length); the rest stream.
