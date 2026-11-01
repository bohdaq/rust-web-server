---
title: "Cookbook: single-page app + API"
description: A React, Vue or Svelte build served with client-side routing, and an API proxied under /api on the same host.
---

The front end is a directory of built files; deep links like `/orders/42`
must get `index.html` (the client-side router takes over), while `/api/*`
goes to the backend:

```toml
# run rws in the build directory (dist/, build/)
port = 443
http_redirect_port = 80
spa_fallback = "index.html"                 # unknown paths get index.html…
spa_fallback_exclude_prefixes = ["/api"]    # …but /api keeps its real 404s

[acme]
domains = ["app.example.com"]
email   = "ops@example.com"

[[upstream]]
name = "api"
backends = ["127.0.0.1:8080"]
[upstream.health_check]
path = "/healthz"

[[route]]
name = "api"
[route.match]
path = "/api/*"
[route.action]
type = "proxy"
upstream = "api"
[route.middleware.rate_limit]
max_requests = 120
window_secs  = 60
```

Requests that match no route — the app's files and its client-side routes —
fall through to the static server. Same origin, so no CORS settings are
needed. When the API lives on another host (`api.example.com`), allow the
app's origin there instead:

```toml
port = 443

[cors]
allow_origins     = "https://app.example.com"
allow_methods     = "GET,POST,PUT,PATCH,DELETE,OPTIONS"
allow_headers     = "content-type,authorization"
allow_credentials = true

[[upstream]]
name = "api"
backends = ["127.0.0.1:8080"]

[[route]]
[route.match]
host = "api.example.com"
path = "/*"
[route.action]
type = "proxy"
upstream = "api"
```
