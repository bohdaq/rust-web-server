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

Sent on every proxied response (decided 2026-10-31: standards by default),
with `received-status=<n>` on success; `proxy_status = "errors"` limits it
to failures, `"off"` drops it. `next-hop` (the backend address) is sent only
with `proxy_status_next_hop = true` — it reveals internal addresses. Applies to: config-file proxy routes (sync and async
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
every rws-made error. Default (decided 2026-10-31): always problem+json;
`error_format = "text"` brings back the plain-text bodies. The static
404 page (`404.html` or the built-in one) is the site's content, not an
rws-made error, and stays HTML. `type` is `about:blank` (RFC 9457 §4.2.1)
except for errors with their own page: `https://rws8.tech/errors/<slug>`
documented on one docs page (`reference/errors.md`), so `type` links
somewhere useful. `AppError` gains `.problem()` for app code that wants the
same shape; `IntoResponse` stays as is.

## Compatibility

Changed defaults, listed under "Changed" in the CHANGELOG with the switch
back: rws-made error bodies are JSON (`error_format = "text"`), and proxied
responses carry `Proxy-Status` (`proxy_status = "errors"` or `"off"`).
`Cache-Status` is a new header only. Status codes don't change.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | **done 2026-10-31**: `error::problem` / `problem_with_text` / `from_text` (`src/error/problem.rs`), `error_format` (`RWS_CONFIG_ERROR_FORMAT`, `ServerConfig::error_format_text`; `text` reproduces every old body byte for byte, an empty old body stays bodiless). Through it: server 400/408/413/417/431 (sync + async), proxy/config proxy/canary/ingress/Gateway API router (>= 400 only) 502/503/504/404/421/500, timeout 504, forward-auth 502, maintenance 503, misconfigured auth/plugin 503, WASM plugin 500, authz 401/403, cache PURGE 401, App 501, SSO/SAML 500, extractor 400, and `AppError` (so IP filter, blocklist, CSRF, Basic/JWT 401, rate limit 429). Left as they are: static 404 page, admin/MCP/LLM/OAuth/validation JSON errors, h2 headers-only 413/431. Docs `reference/errors.md`; tests: `error::tests::rws_errors_are_problem_details_unless_text_is_asked_for`, server 400 test updated | done |
| 2 | **done 2026-10-31**: `proxy::status` (`ProxyError` + `classify` of the forwarders' failure text, `received`/`failed`/`failed_with`, RFC 8941 quoting, appended after an upstream's `Proxy-Status`); settings `proxy_status` (always/errors/off), `proxy_name`, `proxy_status_next_hop`. Wired: `DynamicProxy` sync + async + HTTP/3 (`settle` records the failure), `prepare` (destination_unavailable / configuration), `ReverseProxy`, `H2ReverseProxy`, `H3ReverseProxy`, canary, ingress, config gRPC route, Gateway API router (`Attempt::Failed` carries error + endpoint). Not yet: LLM proxy and MCP gateway (own JSON errors; their upstream failures could get the header later). Tests: `proxy::status::tests` (15 failure texts, modes, chaining, quoting); `ci/e2e/proxy_policies.sh` checks 502 connection_refused + problem+json, 503 destination_unavailable, 200 received-status and no next-hop | done |
| 3 | `Cache-Status` in `CacheLayer` (memory, disk) and the mirror cache; upstream header chaining; `ci/e2e/mirror.sh` + cache unit tests assert each value | — |
| 4 | Docs (`proxy/`, `features/caching.md`, `reference/errors.md`, env vars), CHANGELOG, release 18.17.0 | — |
