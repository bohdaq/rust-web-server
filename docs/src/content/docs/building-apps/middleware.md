---
title: Middleware
description: Add cross-cutting behaviour to any Application by implementing the Middleware trait and stacking layers with .wrap().
---

## The Middleware trait

`Middleware` is defined in `src/middleware/mod.rs`:

```rust
pub trait Middleware: Send + Sync {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String>;
}
```

Call `next.execute(request, connection)` to pass the request to the next layer (or the inner application). Return a `Response` directly to short-circuit the chain.

## Wrapping an application

`.wrap(layer)` attaches a middleware layer to any `Application`. Layers run in registration order — the first `.wrap()` call is the outermost layer (runs first on the way in, last on the way out):

```rust
use rust_web_server::app::App;
use rust_web_server::middleware::{RateLimitLayer, WithMiddleware};
use rust_web_server::core::New;

let app = App::new()
    .wrap(RateLimitLayer);  // outermost: checked before any route
```

Chain multiple layers:

```rust
use rust_web_server::middleware::MetricsLayer;

let app = App::new()
    .wrap(RateLimitLayer)  // checked first
    .wrap(MetricsLayer);   // checked second
```

`App::with_state(S)` and `Router`-based apps all expose the same `.wrap()` method.

## Built-in middleware layers

| Layer | Module | Description |
|---|---|---|
| `RateLimitLayer` | `middleware` | Enforces the process-wide sliding-window rate limit; returns `429` when the budget for the client IP is exceeded. Configured via `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` / `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS`. |
| `MetricsLayer` | `metrics` | Records per-route `rws_route_requests_total` counters and `rws_route_duration_seconds` histograms in Prometheus format. |
| `CacheLayer` | `cache` | In-process response cache; caches `GET` responses by URI. |
| `OtelLayer` | `otel` | Emits OpenTelemetry spans and propagates W3C `traceparent` context. |
| `RewriteLayer` | `rewrite` | Applies configurable request/response rewrite rules (header set/remove, URI prefix manipulation, response body find-and-replace). |
| `ReverseProxy` | `proxy` | HTTP/1.1 reverse proxy; forwards the request to a backend and returns its response. |
| `H2ReverseProxy` | `proxy` | HTTP/2 upstream proxy (requires `http2` feature). |
| `GrpcProxy` | `proxy` | Wraps `H2ReverseProxy`; only proxies requests with `Content-Type: application/grpc*`. |
| `BasicAuthLayer` | `auth` | HTTP Basic authentication; calls a user-supplied verifier closure. |
| `JwtLayer` | `auth` | JWT Bearer token verification; rejects requests without a valid token. |
| `IpFilter` | `ip_filter` | Allow-list or deny-list based on client IP address or CIDR range. |
| `BlocklistLayer` | `blocklist` | Runtime-updatable IP blocklist; unlike `IpFilter`, the list can be modified without restart. |
| `CanaryLayer` | `canary` | Routes a configured percentage of traffic to a canary backend. |
| `RetryLayer` | `circuit_breaker` | Retries failing requests up to a configured limit with backoff. |
| `CsrfLayer` | `csrf` | CSRF token validation for state-mutating requests. |
| `LogLayer` | `request_log` | Structured per-request access logging (combined or JSON format). |
| `MaintenanceLayer` | `maintenance` | Returns `503 Service Unavailable` when maintenance mode is active. |

### Quick usage examples

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::middleware::{RateLimitLayer, MetricsLayer};
use rust_web_server::ip_filter::IpFilter;
use rust_web_server::auth::JwtLayer;
use rust_web_server::request_log::LogLayer;

let app = App::new()
    .wrap(LogLayer)
    .wrap(IpFilter::deny(["1.2.3.4", "10.0.0.0/8"]))
    .wrap(JwtLayer)
    .wrap(RateLimitLayer)
    .wrap(MetricsLayer);
```

## Writing a custom middleware layer

Implement the `Middleware` trait on a unit struct (or a struct carrying configuration):

```rust
use rust_web_server::middleware::Middleware;
use rust_web_server::application::Application;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;

/// Adds an X-Request-Id header to every response.
pub struct RequestIdLayer;

impl Middleware for RequestIdLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        // Call into the rest of the chain / inner application
        let mut response = next.execute(request, connection)?;

        // Mutate the response on the way out
        let id = format!("{}-{}", connection.server.ip, connection.server.port);
        response.headers.push(rust_web_server::header::Header {
            name: "X-Request-Id".to_string(),
            value: id,
        });

        Ok(response)
    }
}
```

Register it like any built-in layer:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;

let app = App::new().wrap(RequestIdLayer);
```

### Short-circuit example

Return a response without calling `next` to block the request:

```rust
use rust_web_server::middleware::Middleware;
use rust_web_server::application::Application;
use rust_web_server::error::{AppError, IntoResponse};
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;

/// Rejects all non-GET/HEAD requests to /public/*.
pub struct ReadOnlyPublicLayer;

impl Middleware for ReadOnlyPublicLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let is_write = request.method != METHOD.get && request.method != METHOD.head;
        let is_public = request.request_uri.starts_with("/public/");

        if is_write && is_public {
            return Ok(AppError::Forbidden.into_response());
        }

        next.execute(request, connection)
    }
}
```

:::note[Layer ordering]
Layers push in order: `app.wrap(A).wrap(B)` runs A before B on the request path and B before A on the response path. Put logging and tracing outermost, authentication next, and business-logic concerns innermost.
:::
