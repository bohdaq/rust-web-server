---
title: "Cookbook: Node apps"
description: Next.js, Nuxt, Remix, SvelteKit or Express behind rws — HTTPS, several app processes, health checks, WebSockets and cached static assets.
---

Run the app on a local port (`next start -p 3000`, `node build` for
SvelteKit, `node .output/server/index.mjs` for Nuxt) and put rws in front:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["shop.example.com"]
email   = "ops@example.com"

[[upstream]]
name = "web"
backends = ["127.0.0.1:3000", "127.0.0.1:3001"]   # two processes; rws balances them
strategy = "least_connections"
[upstream.health_check]
path = "/api/health"
interval_secs = 5
[upstream.retry]
attempts = 2                                       # an idempotent request retries the other process

[[route]]
name = "next-static"
[route.match]
path = "/_next/static/*"                           # fingerprinted: cache them in rws
[route.action]
type = "proxy"
upstream = "web"
[route.middleware.cache]
ttl_secs = 86400

[[route]]
name = "web"
[route.match]
path = "/*"
[route.action]
type = "proxy"
upstream = "web"
preserve_host = true                               # the app sees shop.example.com, not 127.0.0.1
```

WebSockets (Socket.IO, GraphQL subscriptions, Next.js / Vite hot reload in
development) go through the same route: rws relays the upgrade to the
backend that accepted it. During a deploy, drain a process first so no new
requests reach it:

```sh
rws ctl drain web http://127.0.0.1:3000      # restart it, then:
rws ctl drain web http://127.0.0.1:3000 --undo
```
