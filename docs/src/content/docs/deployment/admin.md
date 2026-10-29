---
title: Admin API
description: Inspect and change a running rws on a listener of its own, behind a bearer token, never on the public routes.
---

The admin API (since 18.10.0) lets you inspect a running server without editing files or restarting it. It runs on a listener of its own, which is off by default and never mixed into the public routes, and every request needs a bearer token.

## Turning it on

```toml
[admin]
listen          = "127.0.0.1:9901"           # off when unset
token_env       = "RWS_ADMIN_TOKEN"          # env var holding the token (this is the default)
allowed_origins = ["https://ops.example.com"] # browser origins besides the listener's own
```

```sh
export RWS_ADMIN_TOKEN="$(openssl rand -hex 32)"
rws
curl -H "Authorization: Bearer $RWS_ADMIN_TOKEN" http://127.0.0.1:9901/admin/api/v1/status
```

The token lives in the environment, never in the file. When the variable is unset or empty the admin listener doesn't start (the server still does, and logs why), and `rws --check` reports it as an error. `rws --check` also warns when `listen` is reachable from other hosts.

Keep it on loopback and reach it through an SSH tunnel (`ssh -L 9901:127.0.0.1:9901 host`). If you must listen on another address, firewall the port: the token is the only protection.

## Security

- Every request needs `Authorization: Bearer <token>`, on loopback too. A missing or wrong token gets `401`, whatever the path, and the token is compared in constant time.
- A browser request whose `Origin` is neither the listener's own nor in `allowed_origins` gets `403`, so a page on another site can't use a token the browser holds.
- Admin requests are served on threads of their own (at most 32 connections at once), apart from the server's workers, so an overloaded server can still be inspected.
- Responses carry `Cache-Control: no-store`.

## Endpoints

All under `/admin/api/v1/`, JSON in and out.

| Method and path | What it does |
|---|---|
| `GET /status` | Version, process id, uptime, readiness, compile-time features, and the main, redirect and admin listeners |

Example `GET /status`:

```json
{"version":"18.10.0","pid":4242,"uptime_secs":3605,"ready":true,"features":["http2","http3","acme","auth","mimalloc"],"listeners":{"main":{"address":"0.0.0.0:443","tls":true},"redirect":"80","admin":"127.0.0.1:9901"}}
```

`POST /admin/config/reload` on the main listener (the older reload endpoint) is unchanged.
