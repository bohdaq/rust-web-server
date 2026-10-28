---
title: Kubernetes Gateway API
description: rws as a Kubernetes Gateway API controller — every route kind, the standard-channel extended features, shared and provisioned modes, and how to install it.
---

:::note[Conformance]
rws passes the Gateway API v1.6.3 conformance suite (standard channel) for the GATEWAY-HTTP, GATEWAY-GRPC, GATEWAY-TLS, GATEWAY-TCP and GATEWAY-UDP profiles with every standard-channel extended feature, in both deployment modes; CI runs the suite on every change (`.github/workflows/gateway-api.yml`). Experimental-channel features (HTTPRoute retries, destination-port matching, mixed TLS termination) aren't claimed yet. Design and history: [`spec/GATEWAY_API.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/GATEWAY_API.md).
:::

The Linux release binaries and the container image (`ghcr.io/bohdaq/rws`, `bohdaq/rws`) are built with the `gateway-api` feature; macOS and Windows binaries aren't (build those with `--features gateway-api`). With it and `RWS_CONFIG_GATEWAY_API_ENABLED=true`, rws watches the cluster's Gateway API objects, serves the Gateways whose GatewayClass names its controller, and reports what it accepted in their status. Every replica serves traffic; one, holding the `rws-gateway-controller` Lease, writes status and manages Services.

## What it serves

- **GatewayClass, Gateway, HTTPRoute, ReferenceGrant** status: acceptance, listener conditions (protocol, route kinds, certificates, conflicts), attached-route counts, route parent conditions (listener allowance by namespace, hostname intersection, `sectionName`/`port`), backend resolution (Service kind, ReferenceGrants, existence), `observedGeneration`.
- **A Service per Gateway** (`rws-<gateway>`, `LoadBalancer` by default), in the Gateway's namespace: one port per listener port, each mapped to a container port from `RWS_CONFIG_GATEWAY_API_PORT_RANGE`. A Service only selects pods in its own namespace, so it has no selector: rws writes its EndpointSlice (`rws-<gateway>-ipv4`, and `-ipv6` when pods have IPv6 addresses) with its own serving pods — those in its namespace matching `RWS_CONFIG_GATEWAY_API_POD_SELECTOR` — and their readiness. The Service's load-balancer address becomes the Gateway's `status.addresses`; `spec.infrastructure` labels and annotations are put on it.
- **HTTP and HTTPS listeners**, the latter with the certificate from the listener's TLS Secret (by SNI between listeners).
- **HTTPRoute routing**: hostnames, `Exact`/`PathPrefix` paths, exact header and query-parameter matches, methods, the spec's precedence between routes and rules, weighted backends over the Services' ready endpoints. The client's `Host` is kept.
- **HTTPRoute filters**, applied in the order listed: `RequestHeaderModifier` and `ResponseHeaderModifier` (`set` replaces, `add` appends, `remove` drops — names case-insensitive), `RequestHeaderModifier` on a `backendRef` (only for requests sent to that backend), `RequestRedirect` (scheme, hostname, port, `ReplaceFullPath`/`ReplacePrefixMatch` path, status 301/302/303/307/308; the port defaults to the scheme's well-known port when the scheme is set, else the listener's, and a well-known port isn't written into `Location`), `URLRewrite` (hostname, path) and `CORS` (preflights answered with `204` by rws, actual requests get `Access-Control-Allow-Origin` and the exposed headers; `*` in an origin matches one or more labels). An `ExtensionRef` or unknown filter answers `500`, as the spec requires.

- **Timeouts and retries**: `timeouts.request` bounds the whole exchange, retries included, and `timeouts.backendRequest` each attempt; past either the client gets `504`, and `0s` turns a timeout off. `retry.attempts` is the number of retries (the first try isn't counted) on the listed `retry.codes`, on connection errors and on backend timeouts, `retry.backoff` apart (25 ms when unset).

- **Backend protocols**: a Service port with `appProtocol: kubernetes.io/h2c` is reached over cleartext HTTP/2 (prior knowledge); WebSocket and other `Upgrade` requests are forwarded on a connection of their own, and a backend's `101` turns the client's connection into a byte relay with it. The suite's `retry` field is experimental-channel: rws honours it, but doesn't claim the feature yet.

The GatewayClass's `status.supportedFeatures` lists what rws claims; the conformance job checks the same list.

- **Mirroring**: each `RequestMirror` filter sends a copy of the request, as the rule's other request filters left it, to one ready endpoint of its Service, for `percent` or `fraction` of requests (all when neither is set); the copy's response is dropped and never delays the client's.

- **GRPCRoute**: attached like HTTPRoutes (HTTP and HTTPS listeners carry both), matched on `method` (`Exact` service and/or method; `RegularExpression` isn't supported) and headers, with the header-modifier and mirror filters, weighted backends reached over h2c with trailers passed through. A call no rule matches gets HTTP `404`, which gRPC clients report as `UNIMPLEMENTED`.

- **TLSRoute** on `TLS` listeners: in `Passthrough` mode rws reads the ClientHello's SNI, picks the route whose hostname matches best and relays the still-encrypted stream (the backend presents its own certificate); in `Terminate` mode (the default) it decrypts with the listener's certificate and relays the plaintext over TCP.
- **TCPRoute** on `TCP` listeners and **UDPRoute** on `UDP` listeners: bytes, or datagrams, relayed to a backend of the route's first rule by weight. When several routes attach to one listener, the oldest serves it (TLSRoutes: the best hostname match first). Each UDP client gets its own socket to the backend, closed after 60 s without traffic.

- **ListenerSet**: a Gateway whose `spec.allowedListeners` admits a ListenerSet's namespace (`Same`, `All`, `Selector`; the default admits none) serves its listeners after its own, older sets first, on the Gateway's Service. A set's listener that conflicts with an earlier one (same port and hostname, or another protocol on the port) isn't accepted; the set is accepted while any of its listeners is, and the Gateway counts the attached sets in `status.attachedListenerSets`. Routes reach a set's listeners only with a `ListenerSet` parentRef.

- **Misdirected requests**: over HTTPS, a request whose `Host` belongs to a different listener than the one its SNI chose (and whose certificate it was given) is answered `421 Misdirected Request`.
- **Client certificates** (`spec.tls.frontend`): the validation for a listener's port (`perPort`, else `default`) names ConfigMaps whose `ca.crt` holds the CA certificates (another namespace needs a ReferenceGrant). In `AllowValidOnly` mode (the default) the handshake requires a client certificate those CAs signed; `AllowInsecureFallback` asks for one but lets the connection through either way, and the Gateway reports `InsecureFrontendValidationMode`. A reference that doesn't resolve leaves the listener unaccepted (`NoValidCACertificate`) — rws never serves a port without the validation it was asked for. The controller also watches ConfigMaps for this.

- **BackendTLSPolicy**: a Service (or one of its ports, by `sectionName`) a policy targets is reached over TLS, trusting the CA certificates in the referenced ConfigMaps' `ca.crt` (or the platform's roots with `wellKnownCACertificates: System`), with `hostname` as SNI and as the name checked — or, when `subjectAltNames` are listed, any of those (`Hostname` or `URI`) instead. Between two policies for the same target the older wins and the other is `Conflicted`; a port's own policy beats one for the whole Service. A policy that can't be used (a missing or wrong-kind CA reference) turns its backends' requests into `500`s rather than plaintext. Status is reported per Gateway whose routes reach a targeted Service. A Gateway's `spec.tls.backend.clientCertificateRef` (a TLS Secret, or one in another namespace a ReferenceGrant opens) is the client certificate its connections to these backends present; a reference that doesn't resolve is reported in the Gateway's `ResolvedRefs`.

- **Static addresses** (`spec.addresses`): the first `IPAddress` is asked of the load balancer as the Service's `loadBalancerIP`; the Gateway is programmed once every requested address is the Service's (else `AddressNotUsable`). Another address type isn't accepted (`UnsupportedAddress`). An address with only a type asks for any address.


## Deployment modes

- **shared** (`RWS_CONFIG_GATEWAY_API_MODE=shared`, the default): the pods of the rws Deployment that runs the controller serve every Gateway. One Deployment, scaled as one.
- **provisioned** (`provisioned`; Helm `gatewayAPI.mode: provisioned`): each Gateway gets its own Deployment, `rws-gw-<namespace>-<name>`, in the controller's namespace, with the controller's image (`RWS_CONFIG_GATEWAY_API_IMAGE`) and ServiceAccount; the Gateway's `spec.infrastructure` labels and annotations go on its pods. Those pods (started with `RWS_CONFIG_GATEWAY_API_GATEWAY=<namespace>/<name>`) watch the cluster like the controller but serve only their Gateway and never write; the controller's own pods serve nothing. Gateways don't share pods, so one's load or failure doesn't reach another. A Gateway's Deployment is deleted with it (by the controller: an object can't own one in another namespace).

Either way the Gateway's Service lists the serving pods in its EndpointSlice.

## Settings

| Setting | Default | Meaning |
|---|---|---|
| `RWS_CONFIG_GATEWAY_API_ENABLED` | `false` | Run the controller |
| `RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME` | `rws8.tech/gateway-controller` | The GatewayClasses' `controllerName` to handle |
| `RWS_CONFIG_GATEWAY_API_POD_SELECTOR` | `app.kubernetes.io/name=rws` | Labels of the rws pods, selected by every Gateway's Service |
| `RWS_CONFIG_GATEWAY_API_PORT_RANGE` | `10000-10999` | Container ports for listeners |
| `RWS_CONFIG_GATEWAY_API_SERVICE_TYPE` | `LoadBalancer` | Type of each Gateway's Service |
| `RWS_CONFIG_GATEWAY_API_LEASE_NAMESPACE` | the pod's namespace | Where the leader Lease lives |

Also as `[gateway_api]` in `rws.config.toml` ([Config file](/configuration/config-file/)).

## Permissions

The controller needs to list and watch GatewayClasses, Gateways, HTTPRoutes, GRPCRoutes, TLSRoutes, TCPRoutes, UDPRoutes, ReferenceGrants, BackendTLSPolicies, ListenerSets, Services, EndpointSlices, Namespaces, Secrets and ConfigMaps; patch the status of GatewayClasses, Gateways and every route kind; create and patch Services and EndpointSlices; read its own namespace's Pods; and get, create and update Leases in its namespace. The Gateway API CRDs (standard channel, v1.6) must be installed in the cluster first.

## Benchmarks

rws 18.8.1 against five other Gateway API implementations, each at its chart defaults with one data-plane replica, in one kind cluster on one GitHub Actions runner (run [40681376798](https://github.com/bohdaq/rust-web-server/actions/runs/40681376798)). The same HTTPRoute sends every request to the same nginx backend; [oha](https://github.com/hatoo/oha) loads it for 20 s per row with keep-alive and no compression, from the runner. CPU is what the gateway's pods (controller and data plane) used per request, from the kubelet. Runner hardware varies between runs, so compare rows of this run only. The harness is `bench/gateway/run.sh` (workflow `gateway-bench.yml`, manual).

Versions: Envoy Gateway v1.9.2, NGINX Gateway Fabric 2.7.2, Traefik (chart 41.7.1), kgateway v2.4.6, Istio 1.30.5.

Requests per second, and p99 latency in ms (64 connections unless noted):

| Scenario | rws | kgateway | Istio | Traefik | Envoy Gateway | NGINX GF |
|---|---:|---:|---:|---:|---:|---:|
| HTTP/1.1, 1 KiB | **38 501** · 5.8 | 20 225 · 9.5 | 16 274 · 9.3 | 17 539 · 14 | 16 685 · 15 | 10 951 · 20 |
| HTTP/1.1, 1 KiB, 256 conns | **39 230** · 23 | 21 103 · 36 | 17 795 · 23 | 18 484 · 45 | 17 757 · 67 | 10 814 · 86 |
| HTTP/1.1, 1 MiB | **2 467** · 71 | 2 009 · 93 | 2 057 · 186 | 1 879 · 81 | 1 556 · 191 | 841 · 200 |
| HTTPS, 1 KiB | **34 792** · 5.3 | 19 085 · 10 | 15 946 · 6.7 | 17 941 · 13 | 16 372 · 16 | 10 121 · 22 |
| HTTP/2 + TLS, 1 KiB | **32 120** · 5.5 | 17 226 · 11 | 14 063 · 7.6 | 13 158 · 14 | 14 817 · 15 | 9 419 · 25 |
| Header modifiers + rewrite, 1 KiB | **36 821** · 5.0 | 19 573 · 8.9 | 16 272 · 6.7 | 18 004 · 14 | 16 907 · 16 | —¹ |

CPU µs per request:

| Scenario | rws | kgateway | Istio | Traefik | Envoy Gateway | NGINX GF |
|---|---:|---:|---:|---:|---:|---:|
| HTTP/1.1, 1 KiB | **40** | 106 | 116 | 96 | 80 | 117 |
| HTTP/1.1, 1 KiB, 256 conns | **19** | 82 | 66 | 175 | 171 | 191 |
| HTTP/1.1, 1 MiB | 928 | 1182 | 1254 | **597** | 1098 | 1365 |
| HTTPS, 1 KiB | **33** | 73 | 64 | 165 | 121 | 171 |
| HTTP/2 + TLS, 1 KiB | **63** | 152 | 209 | 178 | 188 | 292 |
| Header modifiers + rewrite, 1 KiB | **27** | 129 | 77 | 96 | 116 | —¹ |

Every gateway answered every request with 200 except as noted. Memory (working set of the gateway's pods after the runs): rws 42–118 MiB (mimalloc keeps freed memory, so it stays at its peak after the 1 MiB run), NGINX Gateway Fabric 60–74, the others 70–200.

¹ NGINX Gateway Fabric answered 404 through the rewriting route in this setup, so the row isn't comparable; it also had 160 cancelled requests over HTTP/2.

What this doesn't measure: many routes or Gateways, configuration churn, connection setup (every scenario reuses connections), and real backends.
