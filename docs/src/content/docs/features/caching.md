---
title: Response Caching
description: In-memory, capacity-bounded response cache middleware with TTL, Vary-header support, and manual invalidation.
---

`CacheLayer` is a middleware that short-circuits the inner application for cacheable `GET` responses, serving them directly from an in-memory store until their TTL elapses. It lives in `src/cache/mod.rs` and implements the standard `Middleware` trait, so it composes with every application variant.

## Basic setup

```rust
use rust_web_server::app::App;
use rust_web_server::cache::CacheLayer;
use rust_web_server::core::New;

let app = App::new()
    .wrap(CacheLayer::memory(1000).ttl(60));
```

This caches up to 1 000 GET responses for 60 seconds each.

## Constructor and builder methods

### `CacheLayer::memory(capacity)`

Creates a new in-memory cache bounded to `capacity` entries. The default TTL is **60 seconds**.

```rust
let layer = CacheLayer::memory(500);
```

### `.ttl(secs)`

Overrides the time-to-live for all cached entries.

```rust
let layer = CacheLayer::memory(500).ttl(300); // 5 minutes
```

### `.vary_by_header(name)`

Includes a request header in the cache key so that different values of that header produce separate cache entries. The name match is case-insensitive. Chain multiple calls to vary by more than one header.

```rust
let layer = CacheLayer::memory(500)
    .vary_by_header("Accept-Language")
    .vary_by_header("Accept");
```

Without `.vary_by_header`, all requests to the same URI share a single cache entry regardless of any request headers.

## What is cached

| Condition | Cached? |
|-----------|---------|
| Method is GET | Yes (if other conditions pass) |
| Method is POST, PUT, DELETE, etc. | No — bypasses cache entirely |
| Response status 2xx (200, 201, 203, 204, 206, …) | Yes |
| Response status 3xx, 4xx, 5xx | No |
| Response `Cache-Control: no-store` | No |
| Response `Cache-Control: private` | No |
| Request `Cache-Control: no-cache` | Bypass hit, call handler, **store** fresh response |

## Cache key

The cache key is the request URI combined with the values of every header named in `.vary_by_header()`. Two requests to the same path but with different `Accept-Language` values produce separate entries when `vary_by_header("Accept-Language")` is configured.

## `Age` header

On cache hits, the response receives an `Age` header indicating how many seconds have elapsed since the entry was inserted:

```
Age: 42
```

If the response already carries an `Age` header, its value is replaced.

## Eviction

When the store is at capacity and a new entry must be inserted, the **oldest** entry (by insertion time) is evicted. Expired entries are purged before every insert, so a full store of expired entries makes room before falling back to oldest-first eviction.

## Cache statistics

`CacheLayer` is cheaply `Clone`-able — all clones share the same backing store. Keep a handle to query statistics or invalidate the cache while the other clone is used as middleware:

```rust
let cache = CacheLayer::memory(1000).ttl(60);
let cache_handle = cache.clone(); // shared reference

let app = App::new().wrap(cache);

// Later, from a handler or admin endpoint:
let hits   = cache_handle.hits();   // u64
let misses = cache_handle.misses(); // u64
let size   = cache_handle.size();   // current entry count
```

### `cache.clear()`

Evicts all entries immediately:

```rust
cache_handle.clear();
```

## Full example

```rust
use rust_web_server::app::App;
use rust_web_server::cache::CacheLayer;
use rust_web_server::core::New;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

let cache = CacheLayer::memory(200)
    .ttl(120)
    .vary_by_header("Accept-Language");

let cache_handle = cache.clone();

let app = App::with_state(cache_handle)
    .get("/api/products", |_req, _params, _conn, cache| {
        // This response will be cached for 120 s per Accept-Language value.
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r
    })
    .get("/admin/cache/stats", |_req, _params, _conn, cache| {
        let body = format!(
            "hits={} misses={} size={}",
            cache.hits(), cache.misses(), cache.size()
        );
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r
    })
    .wrap(cache.clone()); // middleware layer applied outermost
```

:::note[Middleware ordering]
`.wrap()` layers are applied in push order — the first `.wrap()` call is the outermost layer. Place `CacheLayer` last (innermost wrap position) if you want rate limiting or auth to run before the cache is consulted.
:::
