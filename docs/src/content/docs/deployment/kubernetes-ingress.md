---
title: Kubernetes Ingress
description: Use KubernetesIngressWatcher and IngressRouter to dynamically route traffic from Kubernetes Ingress rules.
---

`KubernetesIngressWatcher` watches the Kubernetes API server for Ingress resources and maintains a live route table in memory. `IngressRouter` implements `Application` and uses that table to forward requests to the correct upstream service.

## How it works

1. `KubernetesIngressWatcher` does a full `GET /apis/networking.k8s.io/v1/namespaces/{ns}/ingresses` on an interval (default 30 s, configurable) as a resync safety net, **and** opens a long-lived `?watch=true` streaming connection alongside it — any `ADDED`/`MODIFIED`/`DELETED` event on that stream triggers an immediate re-list instead of waiting for the next interval. On a quiet cluster the watch connection just sits blocked on a read, costing nothing; if the API server doesn't support watch for some reason, the interval resync alone keeps things working, just at up-to-30s latency instead of near-instant.
2. Each Ingress rule is parsed into an `IngressRule` containing `host`, `path`, `path_type`, `service_name`, `service_port`, and `namespace`.
3. `IngressRouter` matches incoming requests (host header + path, per `path_type` — see [Path matching](#path-matching)) against the live rule table and proxies to `{service_name}.{namespace}.svc.cluster.local:{service_port}` over HTTP/1.1.

## Environment variables

| Variable | Required | Example | Description |
|---|---|---|---|
| `RWS_K8S_API_SERVER` | Yes (for `from_env()`) | `http://localhost:8001` | Base URL of the Kubernetes API (plain HTTP) |
| `RWS_K8S_TOKEN` | No | `eyJhbGci…` | Bearer token for API authentication |
| `RWS_K8S_NAMESPACE` | No | `production` | Namespace to watch (default: `default`; use `all` for all namespaces) |

:::note[Two ways to reach the API server]
`from_env()` talks plain `http://` to the Kubernetes API — the simplest option, and the only one that needs no extra Cargo feature. The recommended setup for local development or a sidecar is `kubectl proxy`:

```sh
kubectl proxy &
export RWS_K8S_API_SERVER=http://localhost:8001
export RWS_K8S_TOKEN=
export RWS_K8S_NAMESPACE=default
```

For a pod that wants to talk to the in-cluster API server directly — no sidecar — use [`from_service_account()`](#connecting-directly-from-inside-a-pod) instead, which connects over TLS to `https://kubernetes.default.svc` and requires the `http-client` or `http2` feature.
:::

## Quick start

```rust
use rust_web_server::ingress::{IngressRouter, KubernetesIngressWatcher};
use rust_web_server::server::Server;

fn main() {
    let watcher = KubernetesIngressWatcher::from_env()
        .expect("Set RWS_K8S_API_SERVER before starting");
    watcher.start(); // spawns background polling thread

    let app = IngressRouter::new(watcher);
    let (listener, pool) = Server::setup().unwrap();
    Server::run(listener, pool, app);
}
```

## Configuration options

```rust
let watcher = KubernetesIngressWatcher::from_env()
    .unwrap()
    .namespace("production")     // override namespace
    .ingress_class("rws")        // only watch Ingress objects with spec.ingressClassName: rws
    .poll_interval_secs(15);     // resync every 15 s instead of 30 (watch still runs alongside)

let app = IngressRouter::new(watcher)
    .connect_timeout_ms(2_000)  // TCP connect to upstream (default 5 000 ms)
    .read_timeout_ms(60_000);   // response read timeout   (default 30 000 ms)
```

### IngressClass filtering

On a single-controller cluster, `.ingress_class(...)` doesn't need to be set — the watcher picks up every Ingress object regardless of `spec.ingressClassName` by default, matching the pre-filtering behavior this crate has always had. On a **multi-controller** cluster (e.g. `rws` running alongside `nginx-ingress`), set `.ingress_class("rws")` so this watcher only builds routes from Ingress objects meant for it — an Ingress with no `ingressClassName` at all never matches a configured filter, so it needs one explicitly to be picked up.

## Path matching

`IngressRule::matches` implements the two Kubernetes `pathType` values that carry real routing semantics:

- If `host` is non-empty, the incoming `Host` header must match (case-insensitive).
- Any query string on the request URI is ignored for path matching.
- **`Prefix`** (the default when `pathType` is absent, and how `ImplementationSpecific` is also treated) matches on whole path *segments*, not raw bytes: rule path `/foo` matches `/foo`, `/foo/`, and `/foo/bar`, but **not** `/foobar`.
- **`Exact`** requires the request path to equal the rule's path exactly (ignoring the query string).

The first matching rule wins. Rules are evaluated in the order they are returned by the API server.

Upstream addresses are resolved as:

```
{service_name}.{namespace}.svc.cluster.local:{service_port}
```

## RBAC requirements

The pod's service account needs read access to Ingress resources. Apply the following manifests:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: rws-ingress
  namespace: default
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: rws-ingress-reader
rules:
  - apiGroups: ["networking.k8s.io"]
    resources: ["ingresses"]
    verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: rws-ingress-reader
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: rws-ingress-reader
subjects:
  - kind: ServiceAccount
    name: rws-ingress
    namespace: default
```

## Deployment example

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rws-ingress-controller
spec:
  replicas: 1
  selector:
    matchLabels:
      app: rws-ingress-controller
  template:
    metadata:
      labels:
        app: rws-ingress-controller
    spec:
      serviceAccountName: rws-ingress
      containers:
        - name: rws
          image: your-registry/rust-web-server:latest
          ports:
            - containerPort: 7878
          env:
            - name: RWS_K8S_API_SERVER
              value: "http://localhost:8001"
            - name: RWS_K8S_TOKEN
              value: ""
            - name: RWS_K8S_NAMESPACE
              value: "default"
        # kubectl proxy sidecar exposes the API over plain HTTP
        - name: kubectl-proxy
          image: bitnami/kubectl:latest
          args: ["proxy", "--port=8001", "--address=127.0.0.1"]
```

## Connecting directly from inside a pod

`from_service_account()` (requires the `http-client` or `http2` feature — both already pull in `rustls`) reads the token, namespace, and CA certificate from the files Kubernetes mounts into every pod at `/var/run/secrets/kubernetes.io/serviceaccount/`, and finds the API server via the `KUBERNETES_SERVICE_HOST`/`KUBERNETES_SERVICE_PORT` environment variables every pod also has injected automatically — the same mechanism every other Kubernetes client library uses. No `kubectl proxy` sidecar needed:

```rust
use rust_web_server::ingress::{IngressRouter, KubernetesIngressWatcher};

let watcher = KubernetesIngressWatcher::from_service_account()
    .expect("not running inside a pod, or the http-client/http2 feature isn't enabled");
watcher.start();

let app = IngressRouter::new(watcher);
```

The connection is TLS, trusting **only** the cluster's own CA certificate (`.../ca.crt`) — not the public root store `http_client` uses elsewhere in this crate, since the API server's certificate is signed by that private, cluster-specific CA. Building without the `http-client`/`http2` feature leaves `from_service_account()` returning a clear error pointing back at the `kubectl proxy` + `from_env()` path above, rather than silently failing at connect time.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["http-client"] }
```
