---
title: "Cookbook: Python and Ruby apps"
description: Django, FastAPI, Flask or Rails behind rws — the app server on a port, static and media files served by rws, WebSockets, upload limits and timeouts.
---

The app server (Gunicorn, Uvicorn, Puma) listens on a local port; rws
terminates HTTPS, serves the collected static files itself and proxies the
rest:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["app.example.com"]
email   = "ops@example.com"

[[upstream]]
name = "app"
backends = ["127.0.0.1:8000"]
[upstream.health_check]
path = "/healthz"

# Django: STATIC_ROOT = "/srv/app/public/static" (manage.py collectstatic);
# Rails: public/assets. The request path is looked up under `root`, so
# /static/css/site.css is /srv/app/public/static/css/site.css.
[[route]]
name = "static"
[route.match]
path = "/static/*"
[route.action]
type = "static"
root = "/srv/app/public"

[[route]]
name = "media"
[route.match]
path = "/media/*"
[route.action]
type = "static"
root = "/srv/app/public"

[[route]]
name = "app"
[route.match]
path = "/*"
[route.action]
type = "proxy"
upstream = "app"
read_timeout_ms = 60000          # a slow report may take a minute
[route.middleware]
max_body_size = 52428800         # 50 MB uploads on this route
```

Tell the framework it is behind a proxy so it trusts `X-Forwarded-Proto`
(rws also sends RFC 7239 `Forwarded`): Django `SECURE_PROXY_SSL_HEADER =
("HTTP_X_FORWARDED_PROTO", "https")` and `USE_X_FORWARDED_HOST = True`;
Rails `config.assume_ssl = true`; Uvicorn `--proxy-headers`
`--forwarded-allow-ips=127.0.0.1`. Django Channels and Rails Action Cable
WebSockets work through the `app` route.
