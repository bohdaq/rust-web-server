---
title: "Cookbook: local development"
description: Trusted https://localhost with rws's own certificate authority, one origin for a dev front end and its API, and PHP without Docker.
---

`https://localhost` that browsers trust — needed for secure cookies,
service workers, WebAuthn and OAuth redirects — with a Vite / Next dev
server and an API on one origin:

```toml
port = 8443
tls = "internal"                         # certificates from rws's own CA (run `rws trust` once)
tls_internal_names = "app.localhost"

[[upstream]]
name = "vite"
backends = ["127.0.0.1:5173"]

[[upstream]]
name = "api"
backends = ["127.0.0.1:8080"]

[[route]]
name = "api"
[route.match]
path = "/api/*"
[route.action]
type = "proxy"
upstream = "api"

[[route]]
name = "frontend"
[route.match]
path = "/*"
[route.action]
type = "proxy"
upstream = "vite"                        # hot reload's WebSocket goes through too
```

```sh
rws trust          # once per machine: adds rws's CA to the system and browser stores
rws                # https://localhost:8443 and https://app.localhost:8443
```

A PHP project without Docker or Valet — php-fpm from your package manager:

```toml
port = 8443
tls = "internal"

[[route]]
[route.match]
path = "/*"
[route.action]
type    = "fastcgi"
address = "127.0.0.1:9000"
root    = "public"
```

More in [Frontend dev proxy](/getting-started/frontend-dev-proxy/) and
[HTTPS / TLS](/features/https-tls/#local-https-tls--internal).
