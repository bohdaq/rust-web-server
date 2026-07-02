---
title: Service Discovery
description: Keep backend pools up to date automatically with static lists, environment variables, file polling, or DNS A-record lookups.
---

`BackendPool` maintains a thread-safe list of `"host:port"` addresses and can refresh that list on a background thread. All clones of a `BackendPool` share the same underlying `RwLock<Vec<String>>`, so the live backend list is always consistent across the process.

## Discovery sources

| Source | Constructor | Refresh |
|--------|-------------|---------|
| Fixed list | `BackendPool::static(...)` | Never |
| Environment variables | `BackendPool::env_prefix(...)` | On poll interval |
| File | `BackendPool::file(...)` | On poll interval |
| DNS A-record | `BackendPool::dns(...)` | On poll interval |

### Static list

```rust
use rust_web_server::service_discovery::BackendPool;

let pool = BackendPool::r#static(vec![
    "10.0.0.1:8080".into(),
    "10.0.0.2:8080".into(),
]);

// Immediately available — no .start() required
println!("{:?}", pool.backends());
```

### Environment variable prefix

Scans `PREFIX_0`, `PREFIX_1`, `PREFIX_2`, … in order, stopping at the first variable that is absent:

```rust
// Reads MY_SVC_BACKEND_0, MY_SVC_BACKEND_1, etc.
let pool = BackendPool::env_prefix("MY_SVC_BACKEND")
    .poll_interval_secs(60);
pool.start();
```

Set the environment variables before starting:

```sh
export MY_SVC_BACKEND_0=api-1.internal:8080
export MY_SVC_BACKEND_1=api-2.internal:8080
```

### File-based discovery

One `host:port` per line. Blank lines and lines starting with `#` are ignored:

```text
# backends.txt
api-1.internal:8080
api-2.internal:8080
# api-3.internal:8080  <- temporarily disabled
```

```rust
let pool = BackendPool::file("backends.txt")
    .poll_interval_secs(30);
pool.start();
```

### DNS A-record lookup

Resolves the hostname to all IP addresses returned by the OS (via `ToSocketAddrs`). Each IP becomes a `"ip:port"` entry:

```rust
// Resolves api.service.consul every 30 s
let pool = BackendPool::dns("api.service.consul", 8080)
    .poll_interval_secs(30);
pool.start();
```

:::note[DNS caching]
The OS resolver may cache results independently of the poll interval. Set a low TTL on your DNS records and configure the OS resolver accordingly if you need fast propagation.
:::

## Starting background refresh

`.start()` performs an immediate synchronous refresh, then spawns a daemon thread that calls `.refresh()` every `poll_interval_secs` seconds. For `Static` sources `.start()` is a no-op.

```rust
pool.start(); // call once at application startup
```

## Reading the current backend list

```rust
let backends: Vec<String> = pool.backends();
```

`backends()` takes a read lock and returns a snapshot. It never blocks for longer than the lock acquisition.

## Manual updates

For control planes that push backend lists externally:

```rust
pool.update(vec![
    "new-backend-1:8080".into(),
    "new-backend-2:8080".into(),
]);
```

## Integrating with ReverseProxy

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::service_discovery::BackendPool;
use rust_web_server::proxy::ReverseProxy;

let pool = BackendPool::dns("api.internal", 8080)
    .poll_interval_secs(30);
pool.start();

// Snapshot the backend list at startup; wire the pool into a handler
// or refresh the proxy on each request for dynamic resolution.
let backends = pool.backends();
let app = App::new()
    .wrap(ReverseProxy::new(backends));
```

:::note[Dynamic re-wiring]
`ReverseProxy` holds its backend list at construction time. For fully dynamic routing, read `pool.backends()` inside a custom controller or middleware on each request and call `proxy_http1` directly from `rust_web_server::proxy`.
:::

## Poll interval

```rust
pool.poll_interval_secs(30)  // default: 30 s; only meaningful for File and Dns sources
```

The poll interval is set before `.start()`. Changing it after `.start()` has no effect on the running background thread.
