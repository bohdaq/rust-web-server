---
title: rws vs nginx vs Caddy
description: "rws compared with nginx, Caddy, Traefik, axum and actix-web: certificates, config, HTTP/3, load balancing, auth, Kubernetes and speed, with sources."
---

**Short answer:** pick **rws** if you want one binary that serves files, gets
its own certificates, load balances and adds auth from one TOML file, with
the same engine available as a Rust library. Pick **Caddy** for the same
"HTTPS by default" experience with a large Go plugin ecosystem. Pick
**nginx** when you need the most battle-tested server, the lowest memory use,
or modules only nginx has.

Facts about nginx and Caddy were checked against their documentation on
2026-11-06, Traefik, axum and actix-web on 2026-11-15 ([sources](#sources)). Tell us on
[GitHub](https://github.com/bohdaq/rust-web-server/issues) if something
has changed.

## At a glance

| | rws | Caddy | nginx |
|---|---|---|---|
| Automatic HTTPS | Built in: HTTP-01, TLS-ALPN-01, DNS-01 (RFC 2136, Cloudflare, Route 53, exec), wildcards, on-demand TLS | Built in and on by default: HTTP-01, TLS-ALPN-01, DNS-01 via plugins, on-demand TLS, a local CA for localhost | Official `nginx-acme` dynamic module (HTTP-01, TLS-ALPN-01; no DNS-01), or certbot |
| Config | One `rws.config.toml`; `rws --check`; hot reload on SIGHUP or file change; admin API | Caddyfile or JSON; `caddy validate`; admin API | `nginx.conf`; `nginx -t`; reload; Control API since 1.31.5 (off by default) |
| HTTP/3 (QUIC) | On by default with TLS | On by default | Yes, since 1.25.0 (marked experimental) |
| Load balancing | Round robin, random, least connections, IP hash, weights, sticky cookies, circuit breaker, retries, active health checks | Random, round robin, weighted, least connections, IP/URI/header/cookie hash, active and passive health checks, retries | Round robin, least connections, least time, hash, random, sticky (1.29.6+); active health checks in NGINX Plus |
| Auth | Config: bearer, JWT, basic, failing closed when misconfigured; OIDC sign-in for the admin UI. Library: forward auth, OIDC, SAML | Basic, forward auth; more via plugins | Basic, `auth_request`; JWT and OIDC in NGINX Plus |
| Observability | Prometheus metrics with native histograms, OpenTelemetry traces, metrics and logs, JSON access log, admin UI with traffic analytics (people, AI agents, bots) | Prometheus metrics (HTTP metrics opt-in), OTLP metrics, OpenTelemetry tracing, structured JSON logs | `stub_status`, OpenTelemetry tracing module; full metrics API in NGINX Plus |
| Kubernetes | Gateway API controller (passes the v1.7.0 conformance suite in CI, listed upstream at v1.6.3; all five Gateway profiles), Helm chart | Ingress controller (caddyserver/ingress, work in progress); no Gateway API implementation | NGINX Gateway Fabric (Gateway API), NGINX Ingress Controller |
| Extending | A Rust crate: routes, middleware, MCP server, ORM; WASM plugins | Go plugins compiled in (xcaddy) | C modules, njs, Lua via OpenResty |
| License | MIT | Apache-2.0 | BSD-2-Clause |

## Speed

Measured on one 4-CPU GitHub Actions runner, 64 keep-alive connections, no
compression, access logs off. The method and every number are on the
[benchmarks page](/reference/benchmarks/). In short: rws and nginx lead on
requests per second and CPU per request in most scenarios; nginx uses the
least memory in every scenario.

## When to choose which

- **rws**: a small team's whole front door in one file (static site,
  automatic certificates, a few proxied apps, auth and rate limits), or a
  Rust app that wants a production HTTP server inside it.
- **Caddy**: you want automatic HTTPS and already rely on a Caddy plugin.
- **nginx**: very high traffic on minimal memory, an existing nginx setup, or
  a module only nginx has.

## rws vs Traefik

Traefik (Go, MIT) also gets certificates itself (HTTP-01, TLS-ALPN-01,
DNS-01), serves HTTP/3 and implements the Gateway API. Its strength is
discovery: containers announce their own routes through Docker labels or
Kubernetes objects, and middleware plugins load from a catalogue. rws reads
routes from its file or the Gateway API, has no Docker-label provider, and
also serves static files and runs as a Rust library. Pick Traefik for a
fleet of containers that come and go; rws for a fixed set of sites and apps
in one file.

## rws vs axum or actix-web

axum and actix-web are Rust web frameworks: you write the app, and put
something like rws, nginx or Caddy in front for certificates and proxying.
axum builds on hyper and Tower; it has no TLS, ACME or reverse proxy of its
own. actix-web has its own HTTP stack and serves TLS through its `rustls`
or `openssl` features, with certificates you bring. The `rust-web-server`
crate is the framework and the front door in one: routes and middleware in
Rust, plus automatic certificates, HTTP/3 and the proxy, so a Rust service
can ship as one binary. Pick axum for the Tower ecosystem, actix-web for
its maturity and community; rws when you want one less process.

## Moving from nginx or Caddy

- [Quick start](/getting-started/quick-start/): static files and a proxy in five minutes
- [Automatic HTTPS](/features/acme/)
- [Config-driven proxy](/proxy/config-driven/)

## Sources

- Caddy: [automatic HTTPS](https://caddyserver.com/docs/automatic-https),
  [global options](https://caddyserver.com/docs/caddyfile/options),
  [reverse_proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy),
  [directives](https://caddyserver.com/docs/caddyfile/directives),
  [metrics](https://caddyserver.com/docs/metrics),
  [ingress](https://github.com/caddyserver/ingress),
  [license](https://github.com/caddyserver/caddy/blob/master/LICENSE)
- nginx: [ACME module](https://nginx.org/en/docs/http/ngx_http_acme_module.html),
  [HTTP/3](https://nginx.org/en/docs/http/ngx_http_v3_module.html),
  [upstream](https://nginx.org/en/docs/http/ngx_http_upstream_module.html),
  [changes](https://nginx.org/en/CHANGES),
  [JWT (commercial)](https://nginx.org/en/docs/http/ngx_http_auth_jwt_module.html),
  [OpenTelemetry](https://nginx.org/en/docs/ngx_otel_module.html),
  [Control API (1.31.5)](https://blog.nginx.org/blog/nginx-1-31-5-control-api-predicate-locations-early-body-inspection-and-more),
  [license](https://nginx.org/LICENSE)
- Traefik: [ACME](https://doc.traefik.io/traefik/reference/install-configuration/tls/certificate-resolvers/acme/),
  [entry points (HTTP/3)](https://doc.traefik.io/traefik/reference/install-configuration/entrypoints/),
  [Docker provider](https://doc.traefik.io/traefik/providers/docker/),
  [plugins](https://plugins.traefik.io/),
  [license](https://github.com/traefik/traefik/blob/master/LICENSE.md)
- axum: [docs.rs](https://docs.rs/axum/latest/axum/); actix-web: [HTTP/2 and TLS](https://actix.rs/docs/http2/)
- Kubernetes: [Gateway API implementations](https://gateway-api.sigs.k8s.io/implementations/)
