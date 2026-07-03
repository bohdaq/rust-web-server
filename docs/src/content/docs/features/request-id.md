---
title: Request ID / Correlation ID
description: Give every request/response pair a stable ID so log lines can be correlated across services.
---

Distributed tracing ([`OtelLayer`](/features/tracing/)) creates a span per request, but doesn't give handlers a simple, stable identifier to put in their own log lines — and doesn't propagate one across service boundaries unless the caller already sends a W3C `traceparent`. `RequestIdLayer` fills that gap with a plain string ID, present on both the request (so your handler can read and log it) and the response (so the caller can log the same value).

No new Cargo feature or dependency — always available.

## Quick start

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::request_id::RequestIdLayer;

let app = App::new().wrap(RequestIdLayer::new());
```

- If the incoming request already has an `X-Request-Id` header — set by an upstream gateway, load balancer, or calling service — that exact value is kept and echoed back unchanged. One ID follows the request across every hop instead of getting a new one at each service.
- Otherwise, a fresh ID is generated with `generate_request_id()` and injected into the request **before** your handler runs, so handlers can read it like any other header.
- Either way, the same value is always set on the response.

## Reading it in a handler

Use the `RequestId` extractor:

```rust
use rust_web_server::extract::{FromRequest, RequestId};
use rust_web_server::request::Request;

fn handler(request: &Request) {
    let id = RequestId::from_request(request).unwrap();
    println!("[{}] handling request", id.as_str());
}
```

`RequestId::from_request` never fails — if `RequestIdLayer` isn't wrapping the app (or somehow the header is missing), `id.as_str()` is just `""`.

Or read the header directly, like any other:

```rust
use rust_web_server::request::Request;
use rust_web_server::request_id::DEFAULT_HEADER;

fn handler(request: &Request) {
    let id = request.get_header(DEFAULT_HEADER.to_string()).map(|h| h.value.as_str()).unwrap_or("");
}
```

## Custom header name

```rust
use rust_web_server::request_id::RequestIdLayer;

let layer = RequestIdLayer::new().header("X-Correlation-Id");
```

## Ordering with other middleware

`RequestIdLayer` composes like any other layer — `.wrap()` calls apply in push order, first-pushed is outermost. Register it **first** if you want every other middleware in the stack (including `OtelLayer` or your own access-logging middleware) to see the same ID:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::request_id::RequestIdLayer;
use rust_web_server::otel::OtelLayer;

let app = App::new()
    .wrap(RequestIdLayer::new()) // outermost — runs first, sees the response last
    .wrap(OtelLayer);
```

## The ID itself

`generate_request_id()` produces a UUID-v4-*shaped* string (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`) — familiar in logs, but **not** a spec-compliant UUID (version/variant bits aren't forced) and **not** cryptographically random. It's built from a monotonic counter mixed with the current timestamp via a splitmix64 finalizer, the same non-crypto technique this crate already uses for other unique-but-not-secret IDs (e.g. session IDs).

:::caution[Not a security token]
Don't use `generate_request_id()` (or the ID `RequestIdLayer` generates) as a session ID, CSRF token, password reset token, or anywhere uniqueness must be adversarially guaranteed. For that, use the `crypto` feature's `generate_token()`, which is backed by a CSPRNG.
:::

## Relationship to `OtelLayer`

`X-Request-Id` and `OtelLayer`'s W3C `traceparent` solve related but different problems: `X-Request-Id` is a simple, human-readable, application-level correlation string meant for grepping log files; `traceparent` carries the OpenTelemetry trace/span context used by distributed tracing backends (Jaeger, Tempo, etc.). Use both together — they don't conflict, and `RequestIdLayer` doesn't touch OTEL's own headers.
