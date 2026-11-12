---
title: Service mesh (Gateway API)
description: rws as a routing-only service mesh — HTTPRoutes and GRPCRoutes attached to Services route the traffic between pods, through an rws sidecar in each pod.
---

:::note[Stage 1: routing only]
rws passes the Gateway API v1.7.0 conformance suite's MESH-HTTP and MESH-GRPC profiles (core and every mesh extended feature).
:::

:::caution[No security layer]
This is the Gateway API **mesh profile** without the security half of a service mesh: there is no mutual TLS between pods, no workload identity and no authorization policy. Traffic between pods is as encrypted as your applications make it. Design and progress: [`spec/GATEWAY_MESH.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/GATEWAY_MESH.md).
:::

With the mesh on, an HTTPRoute or GRPCRoute whose `parentRefs` name a **Service** (`group: ""`, `kind: Service`) routes the requests pods send to that Service — matches, header and URL filters, redirects, weighted backends, mirrors, timeouts and retries, as on a Gateway:

```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata: {name: echo-split, namespace: shop}
spec:
  parentRefs:
  - {group: "", kind: Service, name: echo}
  rules:
  - matches: [{path: {type: PathPrefix, value: /v2}}]
    backendRefs: [{name: echo-v2, port: 80}]
  - backendRefs: [{name: echo-v1, port: 80}]
```

## Install

With the [Helm chart](/deployment/kubernetes/), on top of the Gateway API controller (the Gateway API CRDs must be installed first):

```sh
helm install rws oci://ghcr.io/bohdaq/charts/rws --namespace rws-system --create-namespace \
  --set gatewayAPI.enabled=true --set gatewayAPI.mesh.enabled=true
kubectl label namespace shop mesh.rws8.tech/injection=enabled
```

New pods in labelled namespaces get the sidecar; restart existing ones (`kubectl rollout restart deployment -n shop`). To select namespaces differently, replace the selector rather than add to it (Helm merges maps):

```sh
--set gatewayAPI.mesh.injection.namespaceSelector.matchLabels=null \
--set-json 'gatewayAPI.mesh.injection.namespaceSelector.matchExpressions=[{"key":"team","operator":"In","values":["shop"]}]'
```

The chart adds:

- a Service `<release>-mesh` for the configuration endpoint (port 15010) and the injection webhook (443 → 15017);
- a MutatingWebhookConfiguration for pod creation in the selected namespaces, `failurePolicy: Fail` by default (a pod that can't get its sidecar isn't created rather than bypassing the routes);
- the webhook's certificate, generated once and kept across upgrades.

Each injected pod gets two init containers, ahead of its own:

- `rws-mesh-init` (`ghcr.io/bohdaq/rws-mesh-init`): sets the iptables rules and exits. It runs as root with `NET_ADMIN` and `NET_RAW`, so a namespace enforcing the `restricted` Pod Security Standard rejects meshed pods.
- `rws-mesh`: the sidecar, rws as a native sidecar (`restartPolicy: Always`, Kubernetes 1.29 or later), as UID 1337 with a read-only root filesystem. It listens on 127.0.0.1:15001 for redirected connections and on port 15000 for its startup probe.

A pod opts out with the annotation `mesh.rws8.tech/inject: "false"`; `hostNetwork` pods are never injected.

## How a request is routed

- Each meshed pod runs an rws sidecar (`RWS_CONFIG_GATEWAY_API_MODE=mesh-sidecar`). An init container's iptables rules send the pod's **outbound** TCP to it; inbound traffic is untouched.
- The **address the client connected to** — the Service's ClusterIP and port — picks the routes, not the `Host` header. A connection to a pod IP, to a Service without routes, or that doesn't speak HTTP/1.1 or h2c goes to its original destination byte for byte.
- `parentRef.port` limits a route to that Service port; without it the route covers every port of the Service.
- Route `hostnames` are ignored: the destination already chose the Service.
- A request no rule matches gets `404`.
- Backends are reached at their ready endpoints, as listed in the Services' EndpointSlices.

**Consumer and producer routes.** A route in the Service's own namespace (a *producer* route) applies to every client. A route in another namespace (a *consumer* route) applies only to clients in its own namespace, and for them instead of the producer routes. A consumer route may send to the Service it's attached to without a ReferenceGrant.

## The controller and the sidecars

The rws Gateway API controller does the watching (`RWS_CONFIG_GATEWAY_API_MESH=true`):

- It reports `Accepted` and `ResolvedRefs` on each route's Service parents (`NoMatchingParent` when the Service, or the `port` the parentRef names, doesn't exist).
- It serves sidecars their configuration at `GET /mesh/v1/snapshot` on `RWS_CONFIG_GATEWAY_API_MESH_PORT` (15010): the mesh routes, the Services they name and those Services' EndpointSlices, as JSON. A sidecar asks with `?version=<n>` and gets the next version as soon as there is one (a long poll, at most 30 s). Every controller replica answers from its own watches.

Sidecars need no Kubernetes API access. Until a sidecar has its first snapshot, its pod's traffic goes through unchanged.

The endpoint has no authentication in stage 1. It holds routes and the addresses of Services and pods, which pods can already learn from DNS and traffic, and nothing secret.
