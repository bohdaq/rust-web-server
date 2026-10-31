[Read Me](../README.md) > [Spec](.) > Proxy-Status, Cache-Status, problem+json

# Proxy-Status, Cache-Status and problem+json (18.17.0)

Planned 2026-10-31. Three IETF standards that tell a client (or the next
proxy, or an operator with `curl -i`) what rws did with a request, instead
of a bare `502 Bad Gateway` text body:

- **`Proxy-Status`** ([RFC 9209](https://www.rfc-editor.org/rfc/rfc9209)):
  on a proxied response, which proxy handled it and, on failure, why
  (`error=connection_refused`, `error=connection_timeout`,
  `error=http_response_timeout`, `error=destination_unavailable`…).
- **`Cache-Status`** ([RFC 9211](https://www.rfc-editor.org/rfc/rfc9211)):
  whether a cache answered (`hit`, `ttl=`) or forwarded and why
  (`fwd=uri-miss`, `fwd=stale`, `fwd=bypass`, `fwd=method`, `stored`,
  `detail=`).
- **problem+json** ([RFC 9457](https://www.rfc-editor.org/rfc/rfc9457)):
  errors rws itself produces (`401`, `403`, `404` of a route, `413`, `429`,
  `431`, `502`, `503`, `504`) as `application/problem+json`
  `{"type","title","status","detail","instance"}` instead of plain text.

Peers: Fastly, Cloudflare and Varnish send `Cache-Status`; Envoy and Fastly
send `Proxy-Status`; Spring, ASP.NET Core and Huma produce problem+json.
nginx, Caddy and Traefik send none of these (Traefik and Caddy have open
issues for `Cache-Status`).

## Design

**Identity.** Each header names the intermediary: `rws` by default,
`[server] proxy_name` (`RWS_CONFIG_PROXY_NAME`) to tell several hops apart
(a CDN in front, rws behind).

**Proxy-Status.** One error type per failure we already tell apart, set
where the 502/503/504 is made (`proxy::bad_gateway()` today knows nothing):
the forwarders return a failure kind → `ProxyError { kind, detail }` →
`Response` with the header.

| rws situation | `error=` |
|---|---|
| DNS lookup failed | `dns_error` |
| connect refused / reset | `connection_refused` / `connection_terminated` |
| connect timed out | `connection_timeout` |
| TLS handshake / certificate | `tls_protocol_error` / `tls_certificate_error` |
| no response head in time | `http_response_timeout` |
| unparseable upstream response | `http_protocol_error` |
| every backend down / ejected / drained | `destination_unavailable` |
| request too large for the route | `http_request_error` |
| circuit breaker open | `destination_unavailable` + `details="circuit open"` |

On success the header carries `received-status=<n>` only when
`proxy_status = "always"`; default `"errors"`. `next-hop` (the backend
address) is sent only with `proxy_status_next_hop = true` — it reveals
internal addresses. Applies to: config-file proxy routes (sync and async
forwarders), `ReverseProxy`, canary, ingress, Gateway API router,
`H2ReverseProxy`/gRPC, the LLM proxy and the MCP gateway.

**Cache-Status.** `CacheLayer` (memory and disk) and the mirror's disk
cache add `rws; hit; ttl=<secs left>` or `rws; fwd=uri-miss; stored`,
`fwd=stale`, `fwd=method` (non-GET/HEAD), `fwd=request` (`no-cache` /
`Authorization` from the client), `fwd=bypass` (not storable:
`Set-Cookie`, `Vary` outside the key, `no-store`), with `detail=` for the
reason. A response that already has `Cache-Status` from upstream keeps it
and gets ours prepended (RFC 9211 §2: newest first). Static files served
from the open-file cache aren't a "cache" in this sense — no header.

**problem+json.** One builder, `error::problem(status, detail)`, used by
every rws-made error. Default: content negotiation — problem+json when the
request's `Accept` names `application/problem+json` or `application/json`
(or the route is an API route: MCP, LLM proxy, admin API, which already
speak JSON), plain text otherwise, so browsers and existing scripts see
what they see today. `error_format = "problem"` forces it, `"text"` keeps
text always. `type` is `about:blank` (RFC 9457 §4.2.1) except for errors
with their own page: `https://rws8.tech/errors/<slug>` documented on one
docs page (`reference/errors.md`), so `type` links somewhere useful.
`AppError::into_response` gains `.problem()` for app code that wants the
same shape; `IntoResponse` stays as is.

## Compatibility

All additive: two new response headers on proxied/cached responses and a
body format chosen only for clients that ask for JSON. A client parsing the
old text body of a 502 while sending `Accept: application/json` would see
JSON — the CHANGELOG says so, and `error_format = "text"` restores it.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | `error::problem` + negotiation + `error_format`; every rws-made error through it (server 413/431/408, rate limit 429, auth 401/403, maintenance/drain 503, proxy 502/503/504, timeout 504); `reference/errors.md`; tests per producer | — |
| 2 | `ProxyError` kinds from the forwarders (sync, async, h2, gateway router, ingress, LLM, MCP gateway) → `Proxy-Status`; `proxy_name`, `proxy_status`, `proxy_status_next_hop`; e2e: `ci/e2e/proxy_policies.sh` asserts the error per failure (refused port, black-hole timeout, all ejected) | — |
| 3 | `Cache-Status` in `CacheLayer` (memory, disk) and the mirror cache; upstream header chaining; `ci/e2e/mirror.sh` + cache unit tests assert each value | — |
| 4 | Docs (`proxy/`, `features/caching.md`, `reference/errors.md`, env vars), CHANGELOG, release 18.17.0 | — |
