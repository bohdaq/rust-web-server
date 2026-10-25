---
title: Architecture
description: How rws handles a request — listeners, the HTTP/1.1, HTTP/2 and HTTP/3 front ends, the Application trait, middleware, the static-file and proxy fast paths, and configuration.
---

`rws` is one crate: the server, the reverse proxy and the application
framework share one request type and one handler trait. HTTP/1.1 parsing,
routing, middleware, JSON, WebSocket and SSE are written in-crate; HTTP/2 uses
the `h2` crate, HTTP/3 `h3` + `quinn`, TLS `rustls`.

## Builds

| Build | Runtime | Listeners |
|-------|---------|-----------|
| `http3` (default) | tokio | HTTP/1.1 and HTTP/2 over TLS (ALPN), HTTP/3 over QUIC on the same port, optional HTTP → HTTPS redirect |
| `http2` | tokio | as above without QUIC |
| `http1` | none — a thread pool | plain HTTP/1.1 |

Without a certificate the async builds serve plain HTTP/1.1 on the same port.

## A request's path

```
TcpListener / QUIC endpoint
  └─ connection task (async builds) or pool thread (http1)
       ├─ TLS handshake, ALPN → HTTP/1.1 | HTTP/2      (10 s deadline)
       ├─ read head (≤ 64 KiB, 30 s from its first byte), then the body
       │     (Content-Length or chunked; Expect: 100-continue; body limit → 413)
       ├─ Request::parse → Request + ConnectionInfo (addresses, SNI)
       ├─ Application::execute(&request, &connection) → Response
       │     middleware layers → router / controllers / proxy
       ├─ gzip if the client accepts it and it pays off
       └─ write head + body (one write, writev, sendfile, or chunked stream)
```

- **HTTP/1.1** connections are keep-alive loops with pipelining; one handler
  serves plain TCP and TLS. Every read has a 30 s idle timeout.
- **HTTP/2 and HTTP/3** translate each stream into the same `Request`, call the
  same `Application`, and translate the `Response` back into frames. Large
  bodies stream in 64 KiB chunks within flow-control limits.
- **Where handlers run.** Handlers are synchronous. In the async builds a
  request runs on tokio's blocking pool, unless the application says it is
  cheap (`Application::runs_inline` — `App` does for GET and HEAD), in which
  case it runs on the connection task. Async handlers
  (`App::with_async_state`) and config-file proxy routes without middleware
  are awaited without tying up a thread.

## Applications and middleware

Everything that answers requests implements one trait:

```rust
pub trait Application {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String>;
}
```

- `App` — the built-in controllers (health, metrics, static files, …).
- `App::with_state(S)` / `App::with_async_state(S)` — your routes plus shared state.
- `Router` — path routing with `:params` and `*wildcards`, usable inside any application.
- `McpServer`, `ConfigDrivenApp` (the config-file proxy) — also applications.

Middleware wraps an application (`app.wrap(layer)`); the first layer added is
the outermost. Rate limiting, auth, CORS, caching, metrics, tracing, rewriting
and the reverse proxy itself are all `Middleware`, so one model covers a web
app, a gateway and a mix of both.

## Static files

A GET for a regular file takes a fast path: the open file handle, its size
and modification time, small files' contents (≤ 64 KiB) and their gzip
output are cached per file version, so a hit costs one `fstat`. An edit in
place is served at once (strict mode, the default). Files over 256 KiB
stream from disk; on plain TCP, downloads use `sendfile` with the response
head in the same TCP segment. Responses carry `ETag` and `Last-Modified` and
answer `304` to revalidation.

## Reverse proxy

Upstream HTTP/1.1 connections are pooled per backend (64 idle, 60 s). A
config-file route without middleware is forwarded entirely on the event loop;
others go through the blocking path with the same pool. Responses over 1 MB,
chunked or SSE are streamed through instead of buffered. Health checks,
load-balancing strategies, retries and circuit breakers sit around the pool.
See [Reverse proxy](/proxy/reverse-proxy/) and [Config-driven proxy](/proxy/config-driven/).

## Configuration

Settings are layered — built-in defaults, environment variables,
`rws.config.toml`, command-line flags — and all end up as `RWS_CONFIG_*`
environment variables at startup. Request handling reads a typed snapshot
(`ServerConfig::current()`), rebuilt when the configuration changes; `SIGHUP`
reloads CORS, rate limits, log format, proxy routes and TLS certificates
without a restart. See [Configuration](/configuration/overview/).
