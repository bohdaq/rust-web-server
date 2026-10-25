# Ideas

Unordered, grouped by theme. Each needs a user, or a design decision, before it
moves to [ROADMAP.md](ROADMAP.md). Evidence links show where the idea comes from.
Updated 2026-10-25.

## TLS and certificates

- **Encrypted Client Hello (server side)** once rustls has a server API; needs ECH configs published in DNS HTTPS records. Caddy automates it since 2.10, nginx and HAProxy ship it.
- **Clustered ACME** — certificates and challenge tokens in Redis, shared by replicas (Ferron v3, HAProxy 3.3).
- **On-demand TLS** with an "ask" endpoint, for SaaS custom domains (Caddy).
- **CRL checking for inbound mTLS** (a top Traefik request, #3433).

## Protocol

- **Compression dictionaries** ([RFC 9842](https://www.rfc-editor.org/rfc/rfc9842.html), `dcb` / `dcz`) — Pingora has them.
- **103 Early Hints** from config or from upstream `Link` headers ([RFC 8297](https://www.rfc-editor.org/rfc/rfc8297.html)).
- **`Proxy-Status` / `Cache-Status` headers** (RFC 9209 / 9211) and **problem+json** bodies for rws-generated errors (RFC 9457).
- **`Forwarded` header** (RFC 7239) next to `X-Forwarded-For` (Caddy #3262).
- **HTTP/3 to upstreams** (HAProxy 3.3, experimental).
- **WebTransport / MASQUE** once the Rust h3 crates leave experimental status.
- **gRPC-web bridge** (Pingora); a **native gRPC server** only if mounting tonic through tower compatibility falls short.

## Proxy

- **Request mirroring**, **backend drain**, **query-parameter rewrite** (Traefik requests).
- **Disk cache with a purge API** — the cache is in-memory today.
- **Ingress-NGINX annotation compatibility** for clusters migrating off it ([ingress2gateway](https://kubernetes.io/blog/2026/03/20/ingress2gateway-1-0-release)).
- **Access logs for TCP / UDP proxies** (Traefik #7800).

## AI

- **LLM proxy middleware** — token accounting from `usage`, time-to-first-token metrics, token-based rate limits and budgets, provider fallback, virtual keys (LiteLLM, Kong, Envoy AI Gateway).
- **A2A server module** — JSON-RPC + SSE binding and an Agent Card at `/.well-known/agent-card.json` ([A2A 1.0](https://a2a-protocol.org/latest/specification/)); Agent Card rewriting in the proxy.
- **MCP Server Card**, behind a flag, once SEP-2127 settles the discovery path.
- **OpenAPI → MCP tools** conversion (Kong's MCP proxy does it).
- **MCP tools generated** from routes, ORM models or jobs.

## Framework

- **`{param}` path syntax** next to `:param` (what axum 0.8 users expect); typed paths.
- **`rws new`** scaffolding, generators and starter templates (Loco, Cot).
- **WebSocket testing in `TestClient`**; graceful shutdown that drains WebSocket / SSE tasks (open axum issues).
- **`Csv<T>`, content negotiation, NDJSON** responses.
- **GraphQL adapter**, likely async-graphql through tower compatibility.
- **Kafka** producer / consumer / outbox ([KAFKA_ROADMAP.md](KAFKA_ROADMAP.md)).
- **Demo apps** after `examples/` exists ([DEMOS_TODO.md](DEMOS_TODO.md)).

## Auth

- **HTTP Message Signatures** ([RFC 9421](https://www.rfc-editor.org/rfc/rfc9421.html)) as an `HttpSigLayer`.
- **Key rotation for HS256 and Basic** — several valid keys with a grace period (JWKS already rotates RS256 / ES256).
- **`groups` in `OidcClaims`**; exclusive XML canonicalization for SAML.

## Operations and distribution

- **`rwsctl`** over the admin API or MCP.
- **Wasm middleware** host (Wasmtime) with a small pass / modify / respond ABI (Traefik v3 has Wasm plugins).
- **systemd socket activation and `Type=notify`**, deb / rpm packages, a Windows service, nixpkgs / winget.
- **OpenSSF Scorecard**, reproducible builds, automated crates.io publishing.
- **kTLS and CPU affinity** (HAProxy 3.3, Ferron).

## Data / ML

- **Dynamic request batching** for inference endpoints; an example page with candle or ONNX Runtime.
