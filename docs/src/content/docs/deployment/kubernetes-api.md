---
title: Using the Kubernetes API
description: ConfigMaps and Secrets kept current by a watch, k8s:// secret references, service discovery from EndpointSlices, and leader election — the k8s feature.
---

The `k8s` feature (18.35.0) gives an app running in Kubernetes the API itself: configuration that changes without a restart, secrets read where they live, the pods behind a Service, and one replica at a time for singleton work. It builds on [kube-rs](https://kube.rs), which `rust_web_server::k8s` re-exports (`kube`, `k8s_openapi`, `schemars`).

```toml
rust-web-server = { version = "18.35", features = ["k8s"] }
```

## A client

```rust
use rust_web_server::k8s::client;

let kube = client::client().await?;          // in a pod: its service account; elsewhere: ~/.kube/config
let ns = client::current_namespace();        // POD_NAMESPACE, the service account's, the kubeconfig's, else "default"
```

`client::from_kubeconfig(path)` reads a given file; `client::offline()` is a client that never connects (for an operator's `--manifests`, which describes its controllers without a cluster); `client::identity()` is this replica's name (`POD_NAME`, else `HOSTNAME`). [`rws gen deploy`](/deployment/kubernetes/#generate-the-manifests) sets `POD_NAME` and `POD_NAMESPACE` from the downward API.

## ConfigMaps and Secrets

`config::ConfigWatch` keeps one ConfigMap's or Secret's data current through a watch — no volume mount, no restart, and a change arrives in a second or two:

```rust
use rust_web_server::k8s::config::ConfigWatch;

let settings = ConfigWatch::config_map(kube.clone(), &ns, "app-settings").await?;   // fails if it can't be read now
let page_size: usize = settings.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(20);
settings.on_change(|data| println!("now: {data:?}"));

// RWS_CONFIG_* keys become rws's own settings, re-read after each change:
settings.apply_to_env("RWS_CONFIG_");

let credentials = ConfigWatch::secret(kube.clone(), &ns, "db-credentials").await?;
```

| Method | What |
|---|---|
| `get(key)`, `snapshot()` | The value now, every key now |
| `on_change(f)` | Called with the new data after each change |
| `apply_to_env(prefix)` | Keys with `prefix` as environment variables, now and on change, then `server_config::invalidate()` |
| `exists()` | `false` once the object was deleted (the last data is kept) |

Binary data is included when it's UTF-8. A clone shares the watch; it stops when the last clone is dropped.

### `k8s://` secret references

```rust
let password = rust_web_server::k8s::config::resolve(&kube, "k8s://prod/db-credentials#password").await?;
```

`k8s://<namespace>/<secret>#<key>` (or `k8s://<secret>#<key>` for the current namespace) reads one key of a Secret, as `vault://`, `aws-sm://` and `azkv://` read [other stores](/features/secrets/).

## Service discovery

The ready endpoints of a Service, from its EndpointSlices, kept current in a [`BackendPool`](/proxy/service-discovery/) — for a `ReverseProxy`, or to reach each pod of a headless Service:

```rust
use rust_web_server::k8s::discovery;

let pool = discovery::service_endpoints(kube.clone(), &ns, "orders", Some("http")).await?;
pool.backends();    // ["10.42.0.17:8080", "10.42.1.9:8080"]
```

Endpoints whose `ready` condition is `false` are left out; `Some("http")` picks the Service port of that name (`None`: the first). IPv6 addresses come as `[addr]:port`.

## Leader election

`leader::LeaderElector` holds a `coordination.k8s.io/v1` Lease so one replica at a time does singleton work — a scheduler, a cleanup loop:

```rust
use rust_web_server::k8s::leader::LeaderElector;

let elector = LeaderElector::new(kube.clone(), &ns, "report-scheduler");
let mut leader = elector.handle();
tokio::spawn(elector.run());

loop {
    leader.wait_until_leader().await;
    // … work while leader.is_leader() …
}
```

The holder renews every 5 s; another replica takes over when a renewal is 15 s old (`.timing(duration, renew)`). Writes carry the Lease's `resourceVersion`, so two replicas can't both win. `release()` hands it over at once (on shutdown). Controllers get this built in: [`Operator::leader_election`](/deployment/kubernetes-operators/#leader-election).

## Permissions

The pod's service account needs:

| For | Rules |
|---|---|
| `ConfigWatch::config_map` / `secret`, `k8s://` | `get`, `list`, `watch` on `configmaps` / `secrets` in the namespace |
| `discovery::service_endpoints` | `list`, `watch` on `endpointslices` (`discovery.k8s.io`) |
| `LeaderElector` | `get`, `create`, `update` on `leases` (`coordination.k8s.io`) |

`manifests::Rbac` writes them: see [Manifests](/deployment/kubernetes-operators/#manifests).
