---
title: Service mesh (Gateway API)
description: rws as a routing-only service mesh — HTTPRoutes and GRPCRoutes attached to Services route the traffic between pods, through an rws sidecar in each pod.
---

:::note[Stage 1: routing only]
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
