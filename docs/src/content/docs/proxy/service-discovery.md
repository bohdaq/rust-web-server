---
title: Service Discovery
description: Keep backend pools up to date automatically with static lists, environment variables, file polling, DNS A/SRV lookups, Consul, Docker labels, or an etcd watch stream.
---

`BackendPool` maintains a thread-safe list of `"host:port"` addresses and can refresh that list on a background thread. All clones of a `BackendPool` share the same underlying `RwLock<Vec<String>>`, so the live backend list is always consistent across the process.

## Discovery sources

| Source | Constructor | Refresh |
|--------|-------------|---------|
| Fixed list | `BackendPool::static(...)` | Never |
| Environment variables | `BackendPool::env_prefix(...)` | On poll interval |
| File | `BackendPool::file(...)` | On poll interval |
| DNS A-record | `BackendPool::dns(...)` | On poll interval |
| DNS SRV record | `BackendPool::dns_srv(...)` | On poll interval |
| Consul health API | `BackendPool::consul(...)` | On poll interval |
| Docker container labels | `BackendPool::docker(...)` | On poll interval |
| etcd v3 watch | `BackendPool::etcd(...)` | Live — dedicated watch stream, not polling |

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

### DNS SRV lookup (weighted DNS)

Resolves an SRV record (e.g. a Kubernetes headless Service's per-port record: `_http._tcp.my-svc.default.svc.cluster.local`) via a hand-rolled DNS query — no third-party DNS crate. Only the lowest-priority tier of answers is kept (RFC 2782: clients try that tier first); within it, each `target:port` is repeated `weight.clamp(1, 20)` times so a plain round-robin consumer still favors higher-weight targets proportionally — that repetition *is* "weighted DNS" here, since a flat `Vec<String>` backend list has no other way to express weight:

```rust
let pool = BackendPool::dns_srv("_http._tcp.my-svc.default.svc.cluster.local")
    .poll_interval_secs(15);
pool.start();
```

### Consul

Queries a Consul agent's `/v1/health/service/:name` endpoint, which already applies health-check filtering server-side (`passing=true`) — only instances passing every check are returned. `Service.Address` is preferred; if a service registered without one, falls back to `Node.Address`:

```rust
// addr is host:port of the Consul agent, e.g. a local agent or a sidecar.
let pool = BackendPool::consul("127.0.0.1:8500", "api")
    .poll_interval_secs(10);
pool.start();
```

### Docker container labels

Queries the local Docker Engine API over its Unix domain socket (`/var/run/docker.sock` by default) for running containers carrying a given label, using the label's **value** as the backend address directly:

```rust
// A container run with: docker run -l rws.backend=10.0.0.5:8080 ...
let pool = BackendPool::docker("rws.backend")
    .poll_interval_secs(10);
pool.start();

// Non-default socket path:
let pool = BackendPool::docker_with_socket("rws.backend", "/custom/docker.sock");
```

This is deliberately not "guess the address from published ports" — a container can be reachable via its bridge-network IP, a published host port, an overlay-network IP, or a sidecar, and there's no single correct answer without knowing the deployment topology. Requiring an explicit label value sidesteps that ambiguity entirely. Unix-only; on other platforms `discover()` logs a warning and returns an empty list.

### etcd v3 watch

Unlike every other source, `EtcdWatch` is not driven by the poll loop at all once `.start()` is called:

```rust
let pool = BackendPool::etcd(vec!["127.0.0.1:2379".into()], "/services/api/");
pool.start();
```

`.start()` still performs one synchronous `refresh()` first — a one-shot `/v3/kv/range` listing of the key prefix, so `pool.backends()` is populated immediately and the pool is fully usable even if you never call `.start()` at all. But instead of the generic sleep-and-poll thread, `.start()` spawns a dedicated thread holding a long-lived connection to etcd's gRPC-gateway `/v3/watch` endpoint, applying `PUT`/`DELETE` events to the backend list incrementally as they arrive — no polling interval after that. On disconnect it reconnects (with a short backoff) and re-lists from scratch before resuming the watch, so it never silently stops tracking changes.

Each key's value is used as the backend address directly (`etcdctl put /services/api/1 10.0.0.5:8080`, one key per instance under the shared prefix). Plain HTTP only — etcd deployments that require TLS need a plaintext sidecar/proxy in front for now.

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
pool.poll_interval_secs(30)  // default: 30 s; File, Dns, DnsSrv, Consul, and Docker sources only
```

The poll interval is set before `.start()`. Changing it after `.start()` has no effect on the running background thread. `EtcdWatch` ignores it entirely — after the initial `refresh()`, updates arrive from the live watch stream, not on a timer.
