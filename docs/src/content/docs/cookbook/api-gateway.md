---
title: "Cookbook: API gateway"
description: Several services behind one API host — path routing, JWT auth, per-route rate limits, response caching, CORS, retries and circuit breaking.
---

One public host, a service per path prefix, JWT on everything but the
public catalogue, stricter limits on writes, and the catalogue cached:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["api.example.com"]
email   = "ops@example.com"

[cors]
allow_origins = "https://app.example.com"
allow_methods = "GET,POST,PUT,PATCH,DELETE,OPTIONS"
allow_headers = "content-type,authorization"

[rate_limit]                        # every client, every route
max_requests = 600
window_secs  = 60

[[upstream]]
name = "catalog"
backends = ["10.0.1.10:8080", "10.0.1.11:8080"]
[upstream.health_check]
path = "/healthz"
[upstream.circuit_breaker]
failure_threshold = 5
recovery_secs     = 30

[[upstream]]
name = "orders"
backends = ["10.0.2.10:8080", "10.0.2.11:8080"]
[upstream.retry]
attempts = 2

[[route]]
name = "catalog"
[route.match]
path   = "/v1/products/*"
method = "GET"
[route.action]
type = "proxy"
upstream = "catalog"
strip_path_prefix = "/v1"
[route.middleware.cache]
ttl_secs = 30
vary_by  = ["Accept-Language"]

[[route]]
name = "orders"
[route.match]
path = "/v1/orders/*"
[route.action]
type = "proxy"
upstream = "orders"
strip_path_prefix = "/v1"
[route.middleware.auth]
type       = "jwt"
secret_env = "JWT_SECRET"
[route.middleware.rate_limit]
max_requests = 60
window_secs  = 60

[[route]]
name = "health"
[route.match]
path = "/healthz"
[route.action]
type   = "respond"
status = 200
body   = "ok"
```

Requests get `Proxy-Status` and errors come back as RFC 9457
`application/problem+json`; `rws_upstream_*` metrics on `/metrics` show
each backend's latency, retries and breaker state. Rotate `JWT_SECRET`
without logging anyone out with `previous_secret_env`
([key rotation](/features/auth/#rotating-the-secret)).
