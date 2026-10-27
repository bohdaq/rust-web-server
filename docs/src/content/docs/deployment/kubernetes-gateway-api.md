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

Filters (header changes, redirects, rewrites, mirroring, CORS), timeouts and retries, GRPCRoute, TLSRoute, TCPRoute and UDPRoute come in the following phases.

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

The controller needs to list and watch GatewayClasses, Gateways, HTTPRoutes, ReferenceGrants, Services, EndpointSlices, Namespaces and Secrets; patch the status of GatewayClasses, Gateways and HTTPRoutes; create and patch Services; and get, create and update Leases in its namespace. The Gateway API CRDs (standard channel, v1.6) must be installed in the cluster first.
