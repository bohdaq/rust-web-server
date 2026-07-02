---
title: Canary Deployments
description: Split traffic proportionally across multiple backend versions with weighted round-robin for safe, gradual rollouts.
---

`CanaryLayer` implements `Middleware` and distributes requests across backend URLs according to configurable weights. It is the primary tool for blue/green deployments, A/B testing, and incremental rollouts.

## How weights work

Each `WeightedBackend` entry carries a relative integer weight. The rotation is expanded at construction time: a backend with `weight=3` appears three times in the internal ring, so it receives three times as many requests as one with `weight=1`. Selection is lock-free — an `AtomicUsize` counter advances on every request.

A backend with `weight=0` is excluded from the rotation entirely.

## Basic usage

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::canary::{CanaryLayer, WeightedBackend};
use rust_web_server::middleware::WithMiddleware;

// 75 % of traffic → stable, 25 % → canary
let app = WithMiddleware::new(App::new())
    .wrap(
        CanaryLayer::new(vec![
            WeightedBackend::new("http://stable:8080", 3),
            WeightedBackend::new("http://canary:8080", 1),
        ])
    );
```

## Gradual rollout example

Start with a 90/10 split and shift traffic in stages as confidence grows:

```rust
// Stage 1 — 90 % stable, 10 % v2
CanaryLayer::new(vec![
    WeightedBackend::new("http://v1:3000", 9),
    WeightedBackend::new("http://v2:3001", 1),
]);

// Stage 2 — 50/50
CanaryLayer::new(vec![
    WeightedBackend::new("http://v1:3000", 1),
    WeightedBackend::new("http://v2:3001", 1),
]);

// Stage 3 — full cut-over (weight=0 removes v1 from rotation)
CanaryLayer::new(vec![
    WeightedBackend::new("http://v1:3000", 0),
    WeightedBackend::new("http://v2:3001", 1),
]);
```

## Scoping to a path prefix

Use `.path_prefix()` to canary-route only a subset of requests. Requests whose URI does not start with the prefix are passed through to the next middleware or the inner application:

```rust
CanaryLayer::new(vec![
    WeightedBackend::new("http://stable:8080", 9),
    WeightedBackend::new("http://canary:8080", 1),
])
.path_prefix("/api/v2")
```

## Timeout configuration

```rust
CanaryLayer::new(backends)
    .connect_timeout_ms(2_000)   // default: 5 000 ms
    .read_timeout_ms(10_000)     // default: 30 000 ms
```

## Failover behaviour

If the selected backend is unavailable, the layer tries every distinct backend in the rotation before returning `502 Bad Gateway`. The same `(host, port)` pair is never retried twice in a single request — deduplication is applied even when a backend appears multiple times due to its weight.

:::note[Plain HTTP only]
`CanaryLayer` contacts backends over plain HTTP/1.1. For HTTPS backends, place a TLS-terminating rws instance in front and use its plain HTTP port as the backend address.
:::

## WeightedBackend reference

```rust
// url can be "http://host:port", "host:port", or "h2://host:port"
WeightedBackend::new(url: impl Into<String>, weight: u32)
```

| Field | Type | Notes |
|-------|------|-------|
| `url` | `String` | Backend address; `http://` prefix is optional |
| `weight` | `u32` | Relative share of traffic; `0` = excluded |
