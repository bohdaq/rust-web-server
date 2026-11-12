# Gateway API mesh profile, stage 1 (routing only)

Written 2026-11-12. Goal: rws passes the Gateway API **Mesh profile**
(`MESH-HTTP`, `MESH-GRPC`) at v1.7.0 and is listed for it — routing between
services by HTTPRoute and GRPCRoute attached to a Service, with no mTLS. The
plan and the progress log; update the status table as steps land.

Stage 2 (identity, a CA, mTLS, authorization policies) is not part of this
plan: it would follow only if users ask for it.

## What the suite checks

- The suite's own namespaces: `gateway-conformance-mesh` (label
  `gateway-conformance: mesh`) and `gateway-conformance-mesh-consumer`
  (`gateway-conformance: mesh-consumer`), with `echo-v1`/`echo-v2`
  Deployments and Services (ports 80→8080 http, 8080 http, 443→8443,
  9090 tcp, 7070 gRPC with `appProtocol: kubernetes.io/h2c`).
- A test runs the echo image's client inside an `echo` pod (`kubectl exec`
  equivalent) and checks the response: which backend answered, headers,
  status. No route status is checked.
- Routes attach to a Service with `parentRefs: [{group: "", kind: Service,
  name, port?}]`.
- **What a route applies to** (`mesh-frontend*`, `mesh-ports`,
  `mesh-consumer-route`):
  - the request's **original destination** — the Service's ClusterIP and
    port the client connected to — not its `Host` header
    (`MeshClusterIPMatching`);
  - a request to a pod IP matches nothing and goes through unchanged;
  - `parentRef.port`, when set, limits the route to that Service port;
  - a route in another namespace than its Service (a **consumer route**)
    applies only to clients in the route's namespace, and to them instead of
    the Service's own (producer) routes (`MeshConsumerRoute`).
- Tests: `mesh-basic`, `mesh-frontend`, `mesh-frontend-hostname`,
  `mesh-ports`, `mesh-split`, `mesh-consumer-route`, plus the mesh copies of
  the HTTPRoute tests (redirects 303/307/308, matching, named rule, query
  params, redirect host/path/port/scheme, request/backend header modifier,
  rewrite path, weights) and GRPCRoute (header modifier, weights).

## Design

A module `gateway_api::mesh` in rws-gateway-api, behind the `gateway-api`
feature; it reuses `types`, `routes`, `router` and `filters`.

**Controller (the existing rws gateway controller Deployment).**
- Watches Services and HTTPRoutes/GRPCRoutes whose parentRefs name a
  Service, and compiles a **mesh table** per client namespace: for each
  (ClusterIP, port) the routes that apply (consumer routes of that namespace
  first, else the Service's own).
- Serves the table on an HTTP endpoint (`/mesh/v1/config?namespace=<ns>`,
  with an ETag and long-poll), unauthenticated in stage 1: it holds routing
  only — Service addresses and route rules, which every pod can already read
  from DNS and traffic. Stated in the docs.
- Writes route `status.parents` for Service parents with rws's
  `controllerName` (`Accepted`, `ResolvedRefs`).
- A **mutating admission webhook** (rws-k8s `Webhooks`) injects the sidecar
  into pods of namespaces matching a configurable selector (Helm
  `mesh.namespaceSelector`, CI: `gateway-conformance in (mesh,
  mesh-consumer)`). Pods opt out with the annotation `mesh.rws8.tech/inject:
  "false"`.

**Sidecar** (rws in `RWS_CONFIG_GATEWAY_API_MODE=mesh-sidecar`).
- A native sidecar (init container with `restartPolicy: Always`, Kubernetes
  ≥ 1.29) so it is up before the app's containers.
- An init container `rws-mesh-init` (its own small image: iptables, NET_ADMIN
  + NET_RAW) redirects the pod's **outbound** TCP (nat `OUTPUT`) to the
  sidecar's port 15001, except traffic of the sidecar's UID (1337) and to
  localhost. Inbound traffic is untouched.
- For each connection: the original destination (`SO_ORIGINAL_DST`), then
  - a (ClusterIP, port) with routes: HTTP/1.1 or h2c (prior knowledge, for
    gRPC) is parsed and routed by the gateway router (matches, filters,
    weights, redirects, mirrors, timeouts); the chosen backend is sent to its
    Service ClusterIP and port (kube-proxy picks the pod; the sidecar's own
    traffic isn't redirected);
  - anything else (no routes, a pod IP, a TCP or TLS port): relayed to the
    original destination byte for byte.
- Config from the controller's endpoint (long-poll); until the first answer
  everything is relayed unchanged.

## Testing

- Unit: the mesh table (frontend by ClusterIP, ports, consumer vs producer
  precedence, pod IPs), original-destination parsing, the iptables rule
  list, the injection patch.
- Socket tests: the sidecar listener with a fixed table against echo
  backends (HTTP/1.1 and h2c).
- `ci/e2e/gateway_api.sh` mode `mesh`: kind, rws with `mesh.enabled`, the
  suite with `--conformance-profiles=MESH-HTTP,MESH-GRPC`, its own report.
- CI job `gateway-api (mesh)`, separate from the gateway reports until it is
  fully green.

## Steps and status

| Step | What | Status |
|---|---|---|
| M0 | This spec | done |
| M1 | Mesh table: Service parentRefs in `translate`, consumer/producer precedence, route status for Service parents | |
| M2 | Sidecar: outbound listener, original destination, routed or relayed, h2c; config long-poll | |
| M3 | Injection webhook + init image + Helm `mesh.*` values | |
| M4 | CI job, suite green, report; docs page, CHANGELOG | |
