# Kubernetes Gateway API controller (18.8.0, #19)

Written 2026-10-27. Goal of the release: rws as a **Conformant** Gateway API
implementation for **Gateway + HTTPRoute, GRPCRoute, TLSRoute, TCPRoute and
UDPRoute**, claiming every standard-channel extended feature, with the report accepted into
`kubernetes-sigs/gateway-api` and rws listed on the implementations page.
Experimental-channel features follow later. This file is the plan and the
progress log — update the status table as each phase lands.

## Decisions (user, 2026-10-27)

| Question | Decision |
|---|---|
| Kubernetes client | **kube-rs**, behind a new opt-in feature `gateway-api` (the default build gets nothing new) |
| Deployment model | **Both**: a shared rws Deployment serves every Gateway of its GatewayClass (default); per-GatewayClass opt-in to **provisioned** mode, where the controller creates a Deployment + Service per Gateway |
| Route types | **HTTPRoute + GRPCRoute + TLSRoute**, then (asked again once v1.6.3 showed them standard) **TCPRoute + UDPRoute** too — all five |
| Features | **Standard + extended** now; experimental-channel features later. At v1.6.3 the standard channel includes TLSRoute, TCPRoute, UDPRoute, BackendTLSPolicy and ListenerSet, so all of those are in scope |

## Targets

- **Gateway API v1.6.3** (newest, 2026-10-06), standard channel CRDs; the
  suite pinned to the `v1.6.3` tag. Conformance status needs a report for one
  of the two newest releases.
- **Profiles:** `GATEWAY-HTTP`, `GATEWAY-GRPC`, `GATEWAY-TLS`, `GATEWAY-TCP`,
  `GATEWAY-UDP`, all core tests passing.
- **Features to claim**, exactly as named in `pkg/features` at `v1.6.3`:
  - Gateway: `Gateway`, `GatewayPort8080`, `GatewayStaticAddresses`¹,
    `GatewayHTTPListenerIsolation`, `GatewayInfrastructure`¹,
    `GatewayAddressEmpty`¹, `ListenerSet`,
    `GatewayFrontendClientCertificateValidation`,
    `GatewayFrontendClientCertificateValidationInsecureFallback`.
  - HTTPRoute (27): `HTTPRoute`, `HTTPRouteDestinationPortMatching`,
    `HTTPRouteBackendRequestHeaderModification`, `HTTPRouteQueryParamMatching`,
    `HTTPRouteMethodMatching`, `HTTPRouteResponseHeaderModification`,
    `HTTPRoutePortRedirect`, `HTTPRouteSchemeRedirect`, `HTTPRoutePathRedirect`,
    `HTTPRouteHostRewrite`, `HTTPRoutePathRewrite`, `HTTPRouteRequestMirror`,
    `HTTPRouteRequestMultipleMirrors`, `HTTPRouteRequestPercentageMirror`,
    `HTTPRouteRequestTimeout`, `HTTPRouteBackendTimeout`, `HTTPRouteParentRefPort`,
    `HTTPRouteBackendProtocolH2C`, `HTTPRouteBackendProtocolWebSocket`,
    `HTTPRouteNamedRouteRule`, `HTTPRouteCORS`, `HTTPRoute303RedirectStatusCode`,
    `HTTPRoute307RedirectStatusCode`, `HTTPRoute308RedirectStatusCode`,
    `HTTPRouteRetry`, `HTTPRouteRetryBackendTimeout`, `HTTPRouteRetryConnectionError`.
  - GRPCRoute: `GRPCRoute`, `GRPCRouteNamedRouteRule`.
  - TLSRoute: `TLSRoute`, `TLSRouteModeTerminate`, `TLSRouteModeMixed`.
  - `TCPRoute`, `UDPRoute`, `ReferenceGrant`, `BackendTLSPolicy`,
    `BackendTLSPolicySANValidation`.

  ¹ Provisioned mode only — see "Reports" below. The Gateway feature names in
  `main` (e.g. `GatewayHTTPSListenerDetectMisdirectedRequests`,
  `GatewayBackendClientCertificate`) arrive with a later Gateway API release
  and are taken up then; the misdirected-request `421` is implemented anyway.

## Reports

The suite runs one `--gateway-class`, i.e. one mode per run, so there are
**two runs and two reports** (as Envoy Gateway does), both in the README
table: `standard-<version>-default-report.yaml` (shared mode, the default;
everything but the ¹ features) and `standard-<version>-provisioned-report.yaml`
(`--mode=provisioned`). Since each Gateway has its own Service in both
modes, static addresses and infrastructure propagation work in shared mode
too; the ¹ features move into the shared report once phase 8 shows them
passing there.

## Architecture

### Dependencies (feature `gateway-api`)

`kube` `~4.0` (`default-features = false`, `client`, `runtime`, `rustls-tls`,
`aws-lc-rs` — one rustls provider, as everywhere in rws; 4.0 is the last line
on MSRV 1.88, 4.2 needs 1.89) and `k8s-openapi` `0.28` (Service, EndpointSlice,
Secret, Deployment, ConfigMap). Gateway API objects are **our own typed serde
structs** for v1.6 (`src/gateway_api/types/`), read through
`kube::core::Object<Spec, Status>` with an `ApiResource` per kind — not the
`gateway-api` crate (its newest release needs Rust 1.92 and is days old).
Only the fields rws acts on are modeled; unknown fields are kept untouched
when status is patched (server-side apply on `/status` with our field
manager, `rws-gateway`).

### Control plane (`src/gateway_api/controller/`)

- Watches (kube-runtime reflectors): GatewayClass (ours: `controllerName:
  rws8.tech/gateway-controller`), Gateway, HTTPRoute, GRPCRoute, TLSRoute,
  ReferenceGrant, BackendTLSPolicy, Service, EndpointSlice, Secret
  (certificates referenced by listeners, CA ConfigMaps for BackendTLSPolicy),
  Namespace (for `allowedRoutes` selectors).
- A **pure translation step**: snapshot of all watched objects → a
  `GatewayConfig` (listeners per port, route table per listener, backend
  endpoint sets, certificates) **plus the status every object should have**
  (conditions `Accepted`, `Programmed`, `ResolvedRefs`, listener conditions,
  route `parents` with reasons). Deterministic, no I/O — unit-tested against
  the YAML fixtures the conformance suite itself uses.
- Debounced reconcile (100 ms) on any change: translate, hand the config to
  the data plane (atomic swap, like `ConfigDrivenApp`'s generations), write
  changed status only (server-side apply on `/status`, field manager
  `rws-gateway`, every condition with the object's `observedGeneration` —
  the suite has explicit `*ObservedGenerationBump` tests — and no write when
  nothing changed, so status doesn't thrash).
- **Every Gateway gets its own Service**, in both modes: the suite (and
  real users) reach each Gateway at its own address, and several Gateways
  listen on the same port with no hostname (the suite's base manifests put
  `same-namespace` and `all-namespaces` both on :80) — one shared address
  couldn't tell them apart. In shared mode those Services all select the one
  shared rws Deployment, each listener mapped to its own allocated container
  port (which is how the pods know the Gateway); in provisioned mode they
  select that Gateway's own Deployment. The Service carries
  `gateway.networking.k8s.io/gateway-name` and
  `app.kubernetes.io/managed-by: rws-gateway`, `spec.infrastructure` labels
  and annotations, and `spec.addresses` (static addresses); its
  load-balancer ingress is the Gateway's `status.addresses`. (Found while
  writing the phase 1 fixtures; it replaces the single shared Service the
  first draft had.)
- The controller runs on the main tokio runtime, like ACME; gateway mode
  uses the shared runtime at first (dynamic listeners on per-core
  `SO_REUSEPORT` accept loops are a later optimization).
- Leader election (a `coordination.k8s.io` Lease) so only one replica writes
  status; every replica serves traffic.

### Data plane (in the same process)

- **Listener manager:** binds and unbinds ports as Gateway listeners change
  (rws today binds one port + the redirect port at startup). Each listener
  port is served on an **allocated high container port** (from a configured
  range, e.g. 10000–10999), and the Service maps `port: <listener port>` →
  `targetPort: <allocated port>`: the nonroot distroless image can't bind
  ports below 1024, and listener ports can't collide with the health or
  metrics port. TCP and UDP listeners are relayed by the L4 proxies
  (`tcp_proxy`, `udp_proxy`) on their allocated ports. Per port: plain
  HTTP, TLS with an SNI resolver built from the listeners' certificates
  (reusing `tls::SniCertResolver`), or TLS passthrough. Same accept loops as
  `run_tls`/`run_plain`, so HTTP/1.1, HTTP/2 (TLS and h2c), timeouts, PROXY
  protocol and metrics all apply.
- **HTTP routing** (`GatewayRouter`, an `Application`): Gateway API matching
  precedence (exact > prefix length > method > headers > query params; then
  oldest route, then namespace/name), hostname intersection, per-rule filters:
  request/response header modification, redirects (scheme/host/port/path,
  301/302/303/307/308), URL rewrites (host, full path, prefix), CORS, request
  mirroring (new: a fire-and-forget copy to other backends, by percentage),
  timeouts (request and backend), retries (codes, connection errors — the #21
  machinery), weighted backends over EndpointSlice addresses, `appProtocol`
  h2c and WebSocket backends, BackendTLSPolicy (CA + SAN validation, client
  certificate).
- **GRPCRoute:** matching on service/method/headers over HTTP/2 (TLS and h2c),
  forwarded with `proxy::h2_upstream` (trailers, streaming both ways).
- **TLSRoute:** on passthrough listeners, read the ClientHello's SNI without
  terminating, pick a route by hostname, relay bytes to a backend (L4);
  `Terminate` mode decrypts with the listener's certificate and relays the
  plaintext; `Mixed` serves both on one listener.
- **TCPRoute / UDPRoute:** a listener's single route relays to its weighted
  backends (`tcp_proxy` / `udp_proxy`, made live-reconfigurable).
- **ListenerSet:** listeners attached to a Gateway from other objects (and
  namespaces, per `allowedListeners`) merge into that Gateway's listener set,
  with their own status.
- Misdirected requests (HTTP/2 or TLS SNI ≠ `Host`) → `421`; per-listener
  hostname isolation; per-listener client-certificate validation (frontend
  mTLS, with the insecure-fallback mode).

### Provisioned mode

A GatewayClass with `parametersRef` → an `RwsGatewayParameters`-like config
(a ConfigMap; no CRD of our own in this release) with `mode: provisioned`.
The controller then creates, per Gateway, a Deployment (the rws image in
data-plane mode, scoped to that one Gateway by flag) and a Service
(`LoadBalancer` by default) owned by the Gateway, propagating
`spec.infrastructure` labels/annotations and honouring `spec.addresses`
(static addresses → Service `loadBalancerIP` / `externalIPs`). The data-plane
pods run the same watch + translate code filtered to their Gateway; the
controller pod writes status.

### Packaging

- `rws --gateway-api` (binary flag; settings `RWS_CONFIG_GATEWAY_API_*`;
  the `gateway-api` feature is added to the release image's build):
  controller + shared data plane. Release image unchanged (one binary).
- Helm: `gatewayAPI.enabled` installs the GatewayClass, RBAC (watch the
  kinds above, patch their status, manage Deployments/Services in
  provisioned mode, Leases) and runs the image with the flag. The Gateway API
  CRDs themselves are the cluster's (documented: install the standard
  channel release first).

## Testing

- Unit: types round-trip the upstream examples; translation and status
  against fixtures; matching precedence table tests; filters through
  `TestClient`.
- `ci/e2e/gateway_api.sh`: a kind cluster (CI only — downloads of kind are
  blocked locally) with the v1.6 CRDs, MetalLB on the Docker network so
  Gateway addresses are reachable from the runner, rws built into an image
  and installed with the Helm chart, then the official suite:
  `go test ./conformance -run TestConformance -args --gateway-class=rws
  --conformance-profiles=GATEWAY-HTTP,GATEWAY-GRPC,GATEWAY-TLS,GATEWAY-TCP,GATEWAY-UDP
  --supported-features=<the list above> --report-output=… --organization=bohdaq
  --project=rust-web-server --url=https://rws8.tech --version=<tag>
  --contact=@bohdaq` — once per mode (`--mode=provisioned` with a second
  GatewayClass). The job uploads the reports as artifacts; a claimed feature
  that fails fails the job.
- Without a cluster: `kube::Client::new(service, namespace)` takes any
  `tower::Service`, so a fake API server returning canned list/watch
  responses tests the reflectors → translation → status loop in `cargo test`.
- Status fixtures come from the suite's own tests (`conformance/tests/*.go`
  state the expected conditions and reasons: `Accepted`, `ResolvedRefs`,
  `NotAllowedByListeners`, `NoMatchingListenerHostname`, `RefNotPermitted`,
  `BackendNotFound`, …); the YAML they apply is vendored under
  `src/gateway_api/testdata/` with its Apache-2.0 header.

## Phases and status

| # | Phase | Status |
|---|---|---|
| 0 | Spec (this file), roadmap #19 updated | done |
| 1 | `gateway-api` feature: kube deps, typed v1.6 structs, reflectors, pure translation + status for GatewayClass/Gateway/HTTPRoute, fake-API-server tests; leader election as a flag (loop in phase 2) | done — 19 tests (translation against 10 suite manifests + base, the controller loop against a fake API server); MSRV kept by pinning `enum-ordinalize` 4.3.2; one design change found: a Service per Gateway (see Control plane) |
| 2 | Listener manager (dynamic high ports, TLS per listener), per-Gateway Services, HTTPRoute core matching/forwarding over EndpointSlices, Lease leader election; kind + MetalLB + suite in CI running GATEWAY-HTTP core | in progress — **2a done**: each Gateway's Service (container ports allocated from 10000–10999, kept in the Services themselves so replicas and restarts agree; owner reference; infrastructure labels/annotations), applied by server-side apply when it differs; EndpointSlice watch; routes compiled with backends resolved to ready endpoints (named Service ports, IPv6); served listeners carry their container port and TLS material. 27 tests. **2b done**: data plane (a port per served listener, bound/updated/unbound as the configuration changes, TLS per listener with SNI over the Secrets' certificates) and router (listener by hostname specificity, rule by the spec's precedence, backend by weight, the client's `Host` kept — `proxy::forward_pooled_with_host`; 500 for unresolved/zero-weight backends, 503 for no endpoints); tested with real sockets. **2c (part 1) done**: Lease leader election (only the holder writes status/Services; every replica serves), `RWS_CONFIG_GATEWAY_API_*` settings / `[gateway_api]`, `gateway_api::start_from_env()` from the binary's async mains, docs page deployment/kubernetes-gateway-api. Helm: `gatewayAPI.*` values (GatewayClass, ServiceAccount, ClusterRole/Binding, Lease Role, env incl. downward-API pod name/namespace), linted in helm.yml. Next: kind + MetalLB + suite job |
| 3 | HTTPRoute extended filters (headers, redirects, rewrites, CORS, timeouts, retries, query/method/port matching, named rules, parentRef port, h2c/WebSocket backends) | — |
| 4 | Request mirroring (single, multiple, percentage) | — |
| 5 | GRPCRoute (core + extended) | — |
| 6 | TLSRoute (passthrough, terminate, mixed), TCPRoute, UDPRoute; ListenerSet | — |
| 7 | Gateway extended: port 8080, listener isolation, 421, hostname precedence, frontend/backend client certificates, BackendTLSPolicy (+ SAN) | — |
| 8 | Provisioned mode: per-Gateway Deployment/Service, infrastructure propagation, static addresses, empty address | — |
| 9 | Helm chart + docs (deployment/kubernetes-gateway-api) + CHANGELOG; report README; implementation listing PR prepared for the user to open | — |

## Open items

- The `parametersRef` format for provisioned mode (ConfigMap now; a CRD of
  our own would be a later, separate decision).
- Leader election library: kube-runtime has none built in — a small Lease
  loop of our own.
- The listing PR is a public submission from the user's account: prepared
  here, opened by the user.
