---
title: Kubernetes Ingress
description: Use KubernetesIngressWatcher and IngressRouter to dynamically route traffic from Kubernetes Ingress rules.
---

`KubernetesIngressWatcher` polls the Kubernetes API server for Ingress resources and maintains a live route table in memory. `IngressRouter` implements `Application` and uses that table to forward requests to the correct upstream service.

## How it works

1. `KubernetesIngressWatcher` polls `GET /apis/networking.k8s.io/v1/namespaces/{ns}/ingresses` at a configurable interval (default 30 s).
2. Each Ingress rule is parsed into an `IngressRule` containing `host`, `path`, `service_name`, `service_port`, and `namespace`.
3. `IngressRouter` matches incoming requests (host header + URI prefix) against the live rule table and proxies to `{service_name}.{namespace}.svc.cluster.local:{service_port}` over HTTP/1.1.

## Environment variables

| Variable | Required | Example | Description |
|---|---|---|---|
| `RWS_K8S_API_SERVER` | Yes | `http://localhost:8001` | Base URL of the Kubernetes API (plain HTTP only) |
| `RWS_K8S_TOKEN` | No | `eyJhbGci…` | Bearer token for API authentication |
| `RWS_K8S_NAMESPACE` | No | `production` | Namespace to watch (default: `default`; use `all` for all namespaces) |

:::note[Plain HTTP only]
The watcher currently requires `http://` access to the Kubernetes API. The recommended approach for in-cluster use is `kubectl proxy`:

```sh
kubectl proxy &
export RWS_K8S_API_SERVER=http://localhost:8001
export RWS_K8S_TOKEN=
export RWS_K8S_NAMESPACE=default
```

Direct TLS access to `https://kubernetes.default.svc` is not yet supported — `KubernetesIngressWatcher::from_service_account()` returns an error explaining this.
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
    .namespace("production")   // override namespace
    .poll_interval_secs(15);   // poll every 15 s instead of 30

let app = IngressRouter::new(watcher)
    .connect_timeout_ms(2_000)  // TCP connect to upstream (default 5 000 ms)
    .read_timeout_ms(60_000);   // response read timeout   (default 30 000 ms)
```

## Path matching

`IngressRule::matches` implements prefix matching:

- If `host` is non-empty, the incoming `Host` header must match (case-insensitive).
- The request URI must start with `path`, or `path` must be `"/"` (catch-all).

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

:::caution[Coming Soon]
Direct in-cluster TLS access to `https://kubernetes.default.svc` using the mounted service account certificate bundle (`/var/run/secrets/kubernetes.io/serviceaccount/ca.crt`). Once available, `KubernetesIngressWatcher::from_service_account()` will read both the token and CA cert automatically.
:::
