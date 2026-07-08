---
title: Canary Deployments
description: Split traffic proportionally across multiple backend versions with smooth weighted round-robin, live weight updates, and BackendPool-sourced groups for safe, gradual rollouts.
---

`CanaryLayer` implements `Middleware` and distributes requests across backend URLs according to configurable weights. It is the primary tool for blue/green deployments, A/B testing, and incremental rollouts.

## How weights work

Each `WeightedBackend` entry carries a relative integer weight. Selection uses the same *smooth* weighted round-robin (SWRR) algorithm nginx uses: every entry accumulates a running `current_weight` by its configured weight on each request, the entry with the highest `current_weight` is picked, and that entry's `current_weight` is then reduced by the total weight. Weights `5, 1, 1` select roughly `A A B A C A A` (repeating) — **never** five `A`s in a row.

This matters because a naive approach — pre-expand the backend list so a `weight=5` backend appears five times, then advance a counter through it — *does* burst: five consecutive requests would all land on the same instance before rotating to the next. SWRR spreads the same 5:1:1 ratio evenly across the sequence instead.

A backend with `weight=0` is excluded from selection entirely — whether set at construction or via a live [`update()`](#live-weight-updates) call.

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

Start with a 90/10 split and shift traffic in stages as confidence grows. Since [`update()`](#live-weight-updates) changes weights without a restart, a real rollout script calls it in place rather than rebuilding the app between stages — shown here as three separate constructions only to make each stage's config explicit:

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

## Live weight updates

`CanaryLayer` is `Clone`, and every clone shares the same underlying state — keep a handle before wrapping the layer into an `Application`, and call `.update()` on it whenever traffic should shift:

```rust
use rust_web_server::canary::{CanaryLayer, WeightedBackend};
use rust_web_server::middleware::WithMiddleware;

let layer = CanaryLayer::new(vec![
    WeightedBackend::new("http://v1:3000", 9),
    WeightedBackend::new("http://v2:3001", 1),
]);
let handle = layer.clone(); // `layer` moves into `.wrap(...)` below
let app = WithMiddleware::new(App::new()).wrap(layer);

// From a rollout script, an admin endpoint, or a scheduled task — no restart:
handle.update(
    vec![
        WeightedBackend::new("http://v1:3000", 1),
        WeightedBackend::new("http://v2:3001", 1),
    ],
    vec![], // pools — empty here, see below
);
```

`.update(backends, pools)` fully replaces the configuration in one atomic swap — it's the runtime equivalent of building a fresh layer with `CanaryLayer::from_parts(backends, pools)`. Every existing clone of the layer (including one already wrapped into a running server) sees the new weights starting with its very next request.

## Dynamic backends via BackendPool

A group's members don't have to be one fixed URL — `WeightedPool` sources them from a [`BackendPool`](/proxy/service-discovery/) instead, so "10% of traffic to canary" keeps working as canary pods come and go, without editing the canary config each time:

```rust
use rust_web_server::canary::{CanaryLayer, WeightedBackend, WeightedPool};
use rust_web_server::service_discovery::BackendPool;

let canary_pool = BackendPool::dns("canary.internal", 8080).poll_interval_secs(15);
canary_pool.start();

// Fixed stable backend, dynamically-discovered canary group.
let app = App::new().wrap(
    CanaryLayer::new(vec![WeightedBackend::new("http://stable:8080", 9)])
        .add_pool(canary_pool, 1),
);
```

`.add_pool(pool, weight)` is a builder method — call it (repeatedly, for more than one dynamic group) before `.wrap(...)`. To build a layer purely from dynamic groups, use `CanaryLayer::with_pools(vec![WeightedPool::new(pool, weight), ...])` instead of `CanaryLayer::new(...)`.

The pool's *weight* controls how often that group is picked relative to other groups — which specific pool member answers is a plain round-robin over `pool.backends()` at selection time, independent of the cross-group SWRR sequence. `BackendPool` addresses are bare `"host:port"` strings with no scheme, so pool-sourced targets are always contacted over plain HTTP/1.1 — mix in a TLS `WeightedBackend` group alongside a pool if a fixed TLS endpoint is also needed.

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

If the selected backend is unavailable, the layer falls through a ranked order — the primary SWRR pick first, then every other group ranked by its own (post-selection) weight state — until one succeeds or every candidate has failed, then returns `502 Bad Gateway`. This ranking is read-only: computing it doesn't perturb the shared SWRR state further, so a request that has to fail over several times doesn't skew the sequence subsequent unrelated requests see. A `WeightedPool` group expands into *all* of its current live members (starting from that group's own round-robin cursor) before falling through to the next group, so a request exhausts a chosen dynamic group fully before giving up on it.

## TLS backends

Use `https://`, `h2s://`, or `grpcs://` in a backend's URL to contact it over TLS instead of plain HTTP/1.1 — the default port becomes 443 instead of 80. This requires the `http-client` or `http2` feature (both pull in `rustls`); mix TLS and plain-HTTP backends freely in the same rotation:

```rust
CanaryLayer::new(vec![
    WeightedBackend::new("http://stable-backend:8080", 9),
    WeightedBackend::new("https://canary-backend:8443", 1),
])
```

Without the `http-client`/`http2` feature enabled, a TLS backend fails cleanly (the next backend in the rotation is tried, or `502 Bad Gateway` if none succeed) rather than hanging or panicking.

## WeightedBackend reference

```rust
// url can be "http://host:port", "https://host:port", "host:port" (plain,
// port 80), "h2://host:port", "h2s://host:port", "grpc://host:port", or
// "grpcs://host:port"
WeightedBackend::new(url: impl Into<String>, weight: u32)
```

| Field | Type | Notes |
|-------|------|-------|
| `url` | `String` | Backend address; scheme determines plain vs. TLS and the default port (`http`/`h2`/`grpc` → 80, `https`/`h2s`/`grpcs` → 443); no scheme defaults to plain HTTP on port 80 |
| `weight` | `u32` | Relative share of traffic; `0` = excluded |

## WeightedPool reference

```rust
WeightedPool::new(pool: BackendPool, weight: u32)
```

| Field | Type | Notes |
|-------|------|-------|
| `pool` | `BackendPool` | Dynamic group — see [Service Discovery](/proxy/service-discovery/) for the available discovery sources |
| `weight` | `u32` | Relative share of traffic for the group as a whole; `0` = excluded |
