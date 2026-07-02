---
title: Request/Response Rewriting
description: Transform requests before dispatch and responses on the way back with the RewriteLayer middleware.
---

## Quick start

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::rewrite::RewriteLayer;

let app = App::new()
    .wrap(
        RewriteLayer::new()
            .request_header_set("X-Env", "production")
            .request_uri_strip_prefix("/api/v1")
            .response_header_set("Cache-Control", "no-store")
            .response_body_replace("http://staging.internal", "https://example.com"),
    );
```

## How it works

`RewriteLayer` clones the incoming `Request`, applies request rules in registration order, calls the next handler, then applies response rules in registration order on the returned `Response`.

Rules are applied sequentially. If you register `.request_header_set("X-Foo", "a")` and then `.request_header_set("X-Foo", "b")`, the handler receives `X-Foo: b`.

## Request rewriting

Applied before the request reaches any handler.

### `.request_header_set(name, value)`

Add or replace a request header. Case-insensitive name match — existing headers with the same name are removed first.

```rust
.request_header_set("X-Forwarded-Proto", "https")
.request_header_set("Authorization", "Bearer internal-token")
```

### `.request_header_remove(name)`

Remove a request header (case-insensitive). No-op if the header is absent.

```rust
.request_header_remove("X-Debug-Token")
```

### `.request_uri_set(uri)`

Replace the entire request URI.

```rust
.request_uri_set("/v2/canonical-path")
```

### `.request_uri_strip_prefix(prefix)`

Remove a path prefix from the URI. No-op if the prefix is not present. Normalizes to `"/"` if stripping leaves an empty path.

```rust
// /api/v1/users → /users
.request_uri_strip_prefix("/api/v1")
```

### `.request_uri_add_prefix(prefix)`

Prepend a prefix to the request URI.

```rust
// /users → /internal/users
.request_uri_add_prefix("/internal")
```

## Response rewriting

Applied after the handler returns, before the response is sent to the client.

### `.response_header_set(name, value)`

Add or replace a response header. Case-insensitive name match.

```rust
.response_header_set("X-Powered-By", "rws")
.response_header_set("Strict-Transport-Security", "max-age=31536000")
```

### `.response_header_remove(name)`

Remove a response header (case-insensitive). No-op if absent.

```rust
.response_header_remove("Server")
.response_header_remove("X-Powered-By")
```

### `.response_status(code, reason)`

Override the response status code and reason phrase.

```rust
// Treat all upstream 404s as 200 for a SPA fallback
.response_status(200, "OK")
```

### `.response_body_replace(from, to)`

Byte-level find-and-replace across all response body content ranges. Uses a linear non-overlapping scan.

```rust
// Rewrite staging URLs in proxied HTML responses
.response_body_replace("http://staging.internal:3000", "https://api.example.com")
```

## Composing with other middleware

`RewriteLayer` implements `Middleware` and stacks with all other middleware layers. Layers are applied in push order (first `.wrap()` call is outermost):

```rust
use rust_web_server::rate_limit::RateLimitLayer;

let app = App::new()
    .wrap(RateLimitLayer::global())      // outermost — checked first
    .wrap(
        RewriteLayer::new()
            .request_uri_strip_prefix("/api")
            .response_header_set("Cache-Control", "no-store"),
    );
```

## Full example: API gateway strip + label

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::rewrite::RewriteLayer;
use rust_web_server::proxy::ReverseProxy;

let rewrite = RewriteLayer::new()
    // Strip the public prefix before forwarding
    .request_uri_strip_prefix("/api/v2")
    // Tag requests for the upstream service
    .request_header_set("X-Gateway", "rws")
    // Hide internal server identity from clients
    .response_header_remove("Server")
    // Add security header
    .response_header_set("X-Content-Type-Options", "nosniff");

let app = App::new()
    .wrap(rewrite)
    .wrap(ReverseProxy::new("http://127.0.0.1:9000"));
```

:::note[Rule ordering]
Request rules and response rules are each applied in the order they were registered. Chain multiple `.request_*` and `.response_*` calls freely — they compose left-to-right.
:::
