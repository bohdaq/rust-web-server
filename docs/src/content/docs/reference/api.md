---
title: API Reference
description: Quick-reference index of all major public types in rust-web-server, grouped by area.
---

Full rustdoc documentation is published at [docs.rs/rust-web-server](https://docs.rs/rust-web-server).

This page is a quick-reference index — click through to docs.rs for the complete method-level API.

## Application layer

| Type / Trait | Module | Description |
|---|---|---|
| `App` | `app` | Zero-config application with all built-in controllers enabled |
| `AppWithState<S>` | `state` | State-aware app with `.get/.post/.put/.patch/.delete` route builders |
| `AsyncAppWithState<S>` | `async_state` | Async handler variant — requires `http2` feature |
| `WithMiddleware<A>` | `middleware` | Wraps any `Application` with a layered middleware stack |
| `McpServer` | `mcp` | MCP Streamable HTTP server (JSON-RPC 2.0 over `POST /mcp`) |
| `Application` | `application` | Trait implemented by all app types: `execute(&Request, &ConnectionInfo) -> Result<Response, String>` |

## HTTP primitives

| Type / Trait | Module | Description |
|---|---|---|
| `Request` | `request` | Parsed HTTP request: method, URI, version, headers, body bytes |
| `Response` | `response` | HTTP response: status code, headers, body as `Vec<ContentRange>` |
| `Header` | `header` | Name/value pair; constants for all standard header names |
| `MimeType` | `mime_type` | MIME type string constants |
| `Range` / `ContentRange` | `range` | Body content-range helpers used by `Response` |

## Routing

| Type / Trait | Module | Description |
|---|---|---|
| `Router` | `router` | Path-pattern router with `:param` named segments and `*wildcard` |
| `PathParams` | `router` | Extracted path parameters; `.get(name) -> Option<&str>` |
| `ConnectionInfo` | `server` | Client/server IP, port, and SNI hostname per connection |

## Server

| Type / Trait | Module | Description |
|---|---|---|
| `Server` | `server` | TCP listener, thread pool, TLS acceptor, and request loop |

## Middleware

| Type / Trait | Module | Description |
|---|---|---|
| `RateLimiter` | `rate_limit` | Sliding-window per-key rate limiter; `rate_limit::global()` for the singleton |
| `CacheLayer` | `cache` | In-memory TTL response cache |
| `MetricsLayer` | `metrics` | Per-route Prometheus counters and latency histograms |
| `OtelLayer` | `otel` | W3C `traceparent` propagation and OTLP span export |
| `RewriteLayer` | `rewrite` | Request/response header, URI, body, and status rewriting |
| `BasicAuthLayer` | `auth` | HTTP Basic authentication middleware |
| `JwtLayer` | `auth` | HS256 Bearer JWT validation middleware |
| `IpFilter` | `ip_filter` | IP address allowlist or denylist middleware |
| `CsrfLayer` | `csrf` | CSRF double-submit cookie protection middleware |
| `OidcAuth` | `sso` | OAuth2 / OIDC SSO integration middleware |
| `CanaryLayer` | `canary` | Weighted traffic splitting (canary deployments) |
| `CircuitBreaker` | `circuit_breaker` | Circuit breaker state machine (closed / open / half-open) |

## Proxy

| Type / Trait | Module | Description |
|---|---|---|
| `ReverseProxy` | `proxy` | HTTP/1.1 reverse proxy middleware |
| `H2ReverseProxy` | `proxy` | HTTP/2 upstream reverse proxy middleware (`http2` feature) |
| `GrpcProxy` | `proxy` | gRPC proxy (wraps `H2ReverseProxy`, filters on `application/grpc*`) |
| `TcpProxy` | `tcp_proxy` | Standalone L4 TCP proxy with round-robin backend selection |
| `UdpProxy` | `udp_proxy` | Standalone UDP datagram proxy |
| `WsProxy` | `ws_proxy` | WebSocket proxy with bidirectional byte relay |
| `BackendPool` | `service_discovery` | Dynamic backend pool with periodic refresh |

## Real-time protocols

| Type / Trait | Module | Description |
|---|---|---|
| `Sse` / `SseEvent` | `sse` | Server-Sent Events builder |
| `WebSocket` | `websocket` | WebSocket handshake and frame codec |

## Database / ORM

| Type / Trait | Module | Description |
|---|---|---|
| `DbPool` | `model` | Pre-created connection pool with `Mutex<Vec<DbConnection>>` |
| `DbConnection` | `model` | Single database connection for SQLite, PostgreSQL, or MySQL |
| `Repository<T, ID>` | `model` | CRUD repository trait (`find_by_id`, `save`, `delete`, …) |
| `QueryBuilder<T>` | `model` | Fluent query builder with `where_eq`, `fetch_all`, `count`, etc. |

## Sessions

| Type / Trait | Module | Description |
|---|---|---|
| `SessionStore` | `session` | In-memory TTL session store keyed by a session ID string |

## Infrastructure

| Type / Trait | Module | Description |
|---|---|---|
| `TestClient` | `test_client` | In-process test client — dispatches requests without a TCP socket |
| `Container` | `di` | `TypeId`-keyed dependency injection container |
| `Scheduler` | `scheduler` | Background task scheduler for recurring or one-off jobs |
| `JobQueue` | `jobs` | In-memory background job queue with retry-with-backoff (`jobs` feature) |
| `PersistentJobQueue` | `jobs` | Crash-safe job queue backed by the model layer (`jobs` + a `model-*` feature) |
| `KubernetesIngressWatcher` | `ingress` | Polls the Kubernetes API for Ingress resources and maintains a live route table |

## Typed extractors and errors

| Type / Trait | Module | Description |
|---|---|---|
| `Body` | `extract` | Raw request body bytes (never fails) |
| `BodyText` | `extract` | UTF-8 request body (returns 400 on invalid UTF-8) |
| `Query` | `extract` | Parsed query string as `HashMap<String, String>` |
| `RequestHeaders` | `extract` | All request headers with case-insensitive `.get(name)` |
| `IntoResponse` | `error` | Trait for mapping error enums to `Response` values |
| `AppError` | `error` | Built-in error variants: `BadRequest`, `NotFound`, `Internal`, etc. |

:::note[Feature flags]
Some types are only compiled when the corresponding Cargo feature is enabled:

- `AsyncAppWithState` — `http2`
- `H2ReverseProxy`, `GrpcProxy` — `http2`
- `DbPool`, `DbConnection`, `Repository`, `QueryBuilder` — `model-sqlite`, `model-postgres`, or `model-mysql`
- `AsyncClient` (HTTP client) — `http2`

See `Cargo.toml` for the full feature list.
:::
