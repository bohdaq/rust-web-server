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

## The web UI

Open `http://127.0.0.1:9901/admin/` (through the SSH tunnel on a remote host) and paste the token. It shows:

- **Overview**: uptime, request and error counters, a live requests-per-second chart, the listeners and the build.
- **Upstreams**: every backend's health and circuit breaker, with a switch to drain it.
- **Certificates**: what the server presents and when it expires, and ACME's renew button.
- **Controls**: maintenance mode, reload, the response caches (purge everything, a path or a prefix), the IP blocklist and feature flags.
- **Config**: every setting in effect, filterable, secrets hidden.
- **Logs**: the access log as it's written, with pause.

The token stays in the browser tab (`sessionStorage`), and the page talks only to the API below with it. The UI's own files (`/admin/`, `/admin/admin.js`, `/admin/admin.css`) are served without the token: they hold no data. They are built into the binary, so there is nothing to deploy, and they're served with a strict `Content-Security-Policy` (scripts and connections to the listener itself only, no framing).

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
| `GET /config` | Every `RWS_CONFIG_*` setting in effect; values of names with `SECRET`, `PASSWORD` or `TOKEN` read `(hidden)` |
| `POST /reload` | Reload the configuration and certificates, as `SIGHUP` does, and wait for it (up to 10 s): `{"reloaded":true,"applied":true,"reloads":3,"errors":[]}`. With errors (a config file that doesn't validate) `applied` is `false`, `errors` lists them and the running configuration stays. `503` while the server starts or stops, `504` when it didn't finish |
| `GET /metrics` | Request, error and connection counters, thread-pool queue, readiness (`/metrics` on the main port has the full Prometheus set) |
| `GET /build` | Compile-time features as `{"http3":true,…}` |
| `GET /rate-limit` | The global limit: `configured`, `max_requests`, `window_secs` |
| `GET /rate-limit/check?ip=…` | That address's remaining quota (counts as one request) |
| `GET /cors` | The CORS settings in effect |
| `GET /maintenance`, `PUT /maintenance` `{"active":true}` | Maintenance mode: `503` for everything but health checks |
| `GET /blocklist`, `POST /blocklist` `{"ip":"…"}`, `DELETE /blocklist/<ip>` | The runtime IP blocklist (`403`). Addresses are checked; the admin listener isn't behind the blocklist |
| `GET /flags`, `PUT /flags/<name>` `{"enabled":true}` | Runtime feature flags |
| `GET /cache`, `DELETE /cache` | Response-cache hits, misses and size; clear it. `404` when the server has no cache |
| `GET /caches` | Every response cache (18.11.0): `name` (`app` for the binary's own, `route:<name>` per config route, `route:#N` for unnamed ones), `kind` (`memory` or `disk`), `entries`, `bytes` (disk only, else `null`), `hits`, `misses` |
| `DELETE /caches/<name>?path=/p` or `?prefix=/p/` | Purge one target (every `Vary` variant) or a prefix; without either, everything. Answers `{"name":…,"purged":n}`; `404` for an unknown name |
| `GET /routes` | The application's routes (method and pattern) |
| `GET /requests?n=20`, `GET /errors?n=20` | The last requests, or the last 4xx/5xx (at most 100) |
| `GET /upstreams` | The config file's `[[upstream]]`s: each backend's `url`, `weight`, `healthy` (passing its health checks), `breaker` (`closed`, `open`, `half_open`, or `null` without a circuit breaker) and `drained` |
| `PUT /upstreams/<name>/drain` `{"backend":"http://10.0.0.5:8080","drained":true}` | Take a backend out of rotation, or put it back (`false`). New requests skip it, requests already on it finish, its health checks go on. With every backend drained the route answers `503`. Kept across reloads, not restarts. `404` for an unknown upstream or backend |
| `GET /certificates` | Every certificate served (`default` for the main one, virtual hosts and on-demand names by name): `not_after`, `expires`, `days_left`, `source`; and `acme`: whether its renewal loop runs and its last check (`renewed`, `current`, or the error) |
| `GET /logs?n=100` | The last access-log lines (up to 500 kept), oldest first, each as a string: in the `json` log format each is a JSON document |
| `GET /logs/stream` | New access-log lines as they're written, as server-sent events (`data: <line>`), batched every 100 ms. A client that falls behind misses lines rather than slowing the server; at most 8 follow at once (`503` past that) |
| `POST /certificates/renew` | Order a new ACME certificate now, through the renewal loop (`202`); it is hot-swapped when issued, as a scheduled renewal is. `404` without ACME |

Follow the log from a terminal:

```sh
curl -N -H "Authorization: Bearer $RWS_ADMIN_TOKEN" http://127.0.0.1:9901/admin/api/v1/logs/stream
```

Lines are kept only while the admin API runs, and come from the access log (`log_format = "off"` leaves them empty). Lines over 8 KiB are cut.

To reload certificates from files (after replacing them yourself), use `POST /reload`: it reloads them as `SIGHUP` does.

A typical backend rollout: drain it, wait for its in-flight requests (`rws_upstream_requests_total` stops moving), deploy, check `GET /upstreams` shows it `healthy`, undrain.

Maintenance mode applies in proxy mode too (the binary puts the maintenance check in front of config-file routes since 18.10.0; while it's off, proxy routes keep their non-blocking forwarder).

Changes made here are in memory: they last until the process restarts (a reload doesn't undo them). The binary's [MCP tools](/mcp/operate/) run the same code, so both answer alike.

Library users: the admin API reports the cache and routes registered with `admin::ops::register_cache` and `register_routes`.

Example `GET /status`:

```json
{"version":"18.10.0","pid":4242,"uptime_secs":3605,"ready":true,"features":["http2","http3","acme","auth","mimalloc"],"listeners":{"main":{"address":"0.0.0.0:443","tls":true},"redirect":"80","admin":"127.0.0.1:9901"}}
```

`POST /admin/config/reload` on the main listener (the older reload endpoint) is unchanged.
