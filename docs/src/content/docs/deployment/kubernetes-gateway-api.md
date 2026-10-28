---
title: Kubernetes Gateway API
description: rws as a Kubernetes Gateway API controller — work in progress for 18.8.0, what works today and how to try it.
---

:::caution[In progress]
The Gateway API controller is being built for 18.8.0 (the plan, decisions and progress are in [`spec/GATEWAY_API.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/GATEWAY_API.md)). The target is conformance with Gateway API v1.6 for HTTPRoute, GRPCRoute, TLSRoute, TCPRoute and UDPRoute. This page describes what works today.
:::

With the `gateway-api` build feature and `RWS_CONFIG_GATEWAY_API_ENABLED=true`, rws watches the cluster's Gateway API objects, serves the Gateways whose GatewayClass names its controller, and reports what it accepted in their status. Every replica serves traffic; one, holding the `rws-gateway-controller` Lease, writes status and manages Services.

## What works today

- **GatewayClass, Gateway, HTTPRoute, ReferenceGrant** status: acceptance, listener conditions (protocol, route kinds, certificates, conflicts), attached-route counts, route parent conditions (listener allowance by namespace, hostname intersection, `sectionName`/`port`), backend resolution (Service kind, ReferenceGrants, existence), `observedGeneration`.
- **A Service per Gateway** (`rws-<gateway>`, `LoadBalancer` by default): one port per listener port, each mapped to a container port from `RWS_CONFIG_GATEWAY_API_PORT_RANGE`; the Service's load-balancer address becomes the Gateway's `status.addresses`. `spec.infrastructure` labels and annotations are put on it.
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

- **BackendTLSPolicy**: a Service (or one of its ports, by `sectionName`) a policy targets is reached over TLS, trusting the CA certificates in the referenced ConfigMaps' `ca.crt` (or the platform's roots with `wellKnownCACertificates: System`), with `hostname` as SNI and as the name checked — or, when `subjectAltNames` are listed, any of those (`Hostname` or `URI`) instead. Between two policies for the same target the older wins and the other is `Conflicted`; a port's own policy beats one for the whole Service. A policy that can't be used (a missing or wrong-kind CA reference) turns its backends' requests into `500`s rather than plaintext. Status is reported per Gateway whose routes reach a targeted Service.

The provisioned mode comes in the next phase.

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

The controller needs to list and watch GatewayClasses, Gateways, HTTPRoutes, GRPCRoutes, TLSRoutes, TCPRoutes, UDPRoutes, ReferenceGrants, BackendTLSPolicies, ListenerSets, Services, EndpointSlices, Namespaces, Secrets and ConfigMaps; patch the status of GatewayClasses, Gateways and every route kind; create and patch Services; and get, create and update Leases in its namespace. The Gateway API CRDs (standard channel, v1.6) must be installed in the cluster first.
