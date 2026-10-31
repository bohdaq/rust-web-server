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

### `CacheLayer::disk(dir, max_bytes)`

Keeps entries in files under `dir` (created if missing), bounded to `max_bytes` in total, oldest dropped first (since 18.11.0). Entries survive a restart: when the layer is built it indexes the directory and removes expired, unreadable and half-written files. Everything else (what is cached, for how long, `Age`, `Vary`) is the same as the memory cache.

```rust
let layer = CacheLayer::disk("/var/cache/rws/api", 512 * 1024 * 1024)?.ttl(600);
```

- One file per entry, named by a hash of the cache key; it is written to a temporary file and renamed, so a reader never sees a half-written entry.
- Reads and writes are file I/O, so a disk-cached route runs on the blocking pool rather than the event loop. Use it for responses that are expensive to produce, not for ones a memory cache would serve in microseconds.
- A disk error (full disk, permissions) is logged and the response is simply not cached.
- In a container, put `dir` on a writable volume (the image's `/srv` is read-only).

### `.ttl(secs)`

The longest an entry is kept. A response can ask for less with `Cache-Control`: `s-maxage` (meant for shared caches like this one), else `max-age`, is used when it's shorter than the `ttl`; a response that says neither is kept for the `ttl`. `max-age=0` isn't stored at all.

```rust
let layer = CacheLayer::memory(500).ttl(300); // 5 minutes at most
```

| Response says | Kept for (with `ttl(300)`) |
|---|---|
| nothing | 300 s |
| `Cache-Control: max-age=60` | 60 s |
| `Cache-Control: max-age=60, s-maxage=120` | 120 s |
| `Cache-Control: max-age=3600` | 300 s |
| `Cache-Control: max-age=0` | not stored |

Since 18.11.0. Before, every cacheable response was kept for the full `ttl` whatever its `max-age`.

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
| Response `Cache-Control: no-cache` (static files by default) | No — it may only be reused after revalidation, which this cache doesn't do |
| Response `Vary` names a request header that isn't in the key | Not stored — unless it changed nothing: `Origin` without `Access-Control-Allow-Origin`, or `Accept-Encoding` with an unencoded body. `Vary: *` is never stored |
| Request with `Authorization`, `Cookie` or `Range` | No — bypasses cache entirely (the response may be per user or partial; since 17.113.1) |
| Response with `Set-Cookie` | No (since 17.113.1) |
| Streamed response (`stream_file` / `stream_pipe`) | No — the body isn't held in memory (since 17.113.1) |
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

A disk cache evicts by total size instead: expired entries first, then the oldest, until the files fit in `max_bytes`; an entry bigger than `max_bytes` on its own isn't stored.

When the memory store is at capacity and a new entry must be inserted, the **oldest** entry (by insertion time) is evicted. Expired entries are purged before every insert, so a full store of expired entries makes room before falling back to oldest-first eviction.

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

## Purging

`purge` removes part of a cache (memory or disk) and returns how many entries went. A target matches the request path and query as cached, every `Vary` variant included:

```rust
use rust_web_server::cache::Purge;

cache_handle.purge(&Purge::Exact("/products/42".into()));
cache_handle.purge(&Purge::Prefix("/products/".into()));
cache_handle.purge(&Purge::All); // same as clear()
```

### HTTP `PURGE`

`.purge_token(token)` makes the layer answer the `PURGE` method itself, nginx/Varnish style (config file: `purge_token_env`):

```bash
curl -X PURGE -H "Authorization: Bearer $TOKEN" https://example.com/products/42
curl -X PURGE -H "Authorization: Bearer $TOKEN" 'https://example.com/products/*'
# {"purged":3}
```

A trailing `*` purges a prefix. A wrong or missing token gets `401`; the request never reaches the application. Without a token set, `PURGE` passes through to the application like any other method. rws accepts `PURGE` as a request method since 18.11.0 (before, it was a `400`).

### Admin API and MCP

Named caches go in `cache::registry`: the binary registers its own as `app`, and every config route's cache as `route:<name>` (rebuilt on each reload). The admin API lists them at `GET /admin/api/v1/caches` and purges with `DELETE /admin/api/v1/caches/<name>?path=…` or `?prefix=…` (see [Admin API](/deployment/admin/)); the admin UI has a purge field on its Controls page. The `cache_clear` MCP tool takes the same optional `path` / `prefix`. Library apps call `cache::registry::register(name, layer.clone())` to add theirs.

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
