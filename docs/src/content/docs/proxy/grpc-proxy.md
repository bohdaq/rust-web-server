---
title: gRPC Proxy
description: Route gRPC traffic to HTTP/2 backends with content-type filtering and round-robin load balancing.
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
| `application/grpc-web` | gRPC-Web |

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

TLS certificate verification uses the system WebPKI trust store (`webpki-roots`). Self-signed or private CA certificates are not supported without a custom build. TLS backends require the `http2` Cargo feature (already required for `GrpcProxy`).

## gRPC trailers

gRPC uses HTTP/2 trailers to carry `grpc-status` and `grpc-message` at the end of a response stream. The current implementation forwards DATA frames as-is, but HTTP/2 trailers are not yet propagated from upstream to the client.

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

Supported backend URL schemes for `H2ReverseProxy`:

| Scheme | Transport | Default port |
|--------|-----------|--------------|
| `host:port` | plain TCP | (explicit) |
| `h2://host:port` | plain TCP | 80 |
| `h2s://host:port` | TLS + ALPN `h2` | 443 |
| `https://host:port` | TLS + ALPN `h2` | 443 |

`H2ReverseProxy` uses `tokio::task::block_in_place` to bridge the synchronous middleware interface into the async tokio runtime required for HTTP/2 upstream connections.
