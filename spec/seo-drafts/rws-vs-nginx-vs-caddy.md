---
title: rws vs nginx vs Caddy
description: How rws compares with nginx and Caddy as a web server and reverse proxy — certificates, config, HTTP/3, load balancing, auth, observability, Kubernetes and speed — with links to measured benchmarks.
---

<!-- DRAFT for review (spec/SEO_GEO.md). Lines marked [verify] state facts
about nginx or Caddy: check each against their current docs before this
goes to docs/src/content/docs/reference/compare.md (+ sidebar entry). -->

**Short answer:** pick **rws** if you want one binary that serves files, gets
its own certificates, load balances and adds auth from one TOML file — and
the same engine as a Rust library. Pick **Caddy** for the same "HTTPS by
default" experience with a large Go plugin ecosystem. Pick **nginx** when you
need the most battle-tested server, the lowest memory use, or modules only
nginx has.

## At a glance

| | rws | Caddy | nginx |
|---|---|---|---|
| Automatic HTTPS (Let's Encrypt, renewal) | Built in: HTTP-01, TLS-ALPN-01, DNS-01 (RFC 2136, Cloudflare, Route 53, exec), on-demand TLS | Built in [verify] | Needs certbot, or the newer `ngx_http_acme` module [verify] |
| Config | One `rws.config.toml`; `rws --check`; hot reload on SIGHUP or file change | Caddyfile or JSON; admin API [verify] | `nginx.conf`; `nginx -t`; reload |
| HTTP/3 (QUIC) | On by default with TLS | Yes [verify] | Yes, since 1.25 [verify] |
| Load balancing | Round robin, least connections, IP hash, weights, sticky cookies, circuit breaker, retries, health checks | Yes [verify] | Yes; some features in NGINX Plus only [verify] |
| Auth | Bearer, JWT (HS256/RS256/ES256), basic, forward auth, OIDC, SAML — fail closed | Basic, forward auth; more via plugins [verify] | Basic, `auth_request`; JWT in NGINX Plus [verify] |
| Observability | Prometheus metrics with native histograms, OpenTelemetry traces and metrics, JSON access log, admin UI with traffic analytics (people, AI agents, bots) | Prometheus metrics, structured logs [verify] | Stub status; more in NGINX Plus [verify] |
| Kubernetes | Gateway API controller (all five Gateway profiles, v1.6.3 conformance) | Third-party ingress [verify] | NGINX Gateway Fabric / Ingress controller [verify] |
| Extending | A Rust crate: routes, middleware, MCP server, ORM; WASM plugins | Go plugins compiled in [verify] | C modules, njs, Lua (OpenResty) [verify] |
| License | MIT | Apache-2.0 [verify] | BSD-2-Clause [verify] |

## Speed

Measured on one 4-CPU GitHub Actions runner, 64 keep-alive connections, no
compression, access logs off — the method and every number are on the
[benchmarks page](/benchmarks/). In short: rws and nginx lead on requests per
second and CPU per request in most scenarios; nginx uses the least memory in
every scenario.

## When to choose which

- **rws**: a small team's whole front door in one file — static site,
  automatic certificates, a few proxied apps, auth and rate limits — or a Rust
  app that wants a production HTTP server inside it.
- **Caddy**: you want automatic HTTPS and already rely on a Caddy plugin.
- **nginx**: very high traffic on minimal memory, an existing nginx setup, or
  a module only nginx has.

## Moving from nginx or Caddy

- [Quick start](/docs/getting-started/quick-start/): static files and a proxy in five minutes
- [Automatic HTTPS](/docs/features/acme/)
- [Config-driven proxy](/docs/proxy/config-driven/)
