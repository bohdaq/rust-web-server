# Ideas

Unordered, grouped by theme. Each needs a user, or a design decision, before it
moves to [ROADMAP.md](ROADMAP.md). Evidence links show where the idea comes from.
Updated 2026-10-25.

## TLS and certificates


## Protocol

- **WebTransport / MASQUE** once the Rust h3 crates leave experimental status.
- **Native gRPC server**, only if mounting tonic through tower compatibility falls short.

## Proxy

- **Ingress-NGINX annotation compatibility** for clusters migrating off it ([ingress2gateway](https://kubernetes.io/blog/2026/03/20/ingress2gateway-1-0-release)).
- **Access logs for TCP / UDP proxies** (Traefik #7800).

## AI

- **MCP Server Card**, behind a flag, once SEP-2127 settles the discovery path.
- **MCP tools generated** from routes, ORM models or jobs.

## Framework

- **WebSocket testing in `TestClient`**; graceful shutdown that drains WebSocket / SSE tasks (open axum issues).
- **`Csv<T>`, content negotiation, NDJSON** responses.
- **GraphQL adapter**, likely async-graphql through tower compatibility.
- **Kafka** producer / consumer / outbox ([KAFKA_ROADMAP.md](KAFKA_ROADMAP.md)).
- **Demo apps** after `examples/` exists ([DEMOS_TODO.md](DEMOS_TODO.md)).

## Auth

- **HTTP Message Signatures** ([RFC 9421](https://www.rfc-editor.org/rfc/rfc9421.html)) as an `HttpSigLayer`.
- **`groups` in `OidcClaims`**; exclusive XML canonicalization for SAML.

## Operations and distribution

- **OpenSSF Scorecard**, reproducible builds, automated crates.io publishing.
- **kTLS and CPU affinity** (HAProxy 3.3, Ferron).

## Data / ML

- **Dynamic request batching** for inference endpoints; an example page with candle or ONNX Runtime.
