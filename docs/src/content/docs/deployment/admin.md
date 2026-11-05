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

## From the command line: `rws ctl`

`rws ctl` (since 18.20.0) drives the same API from a shell. The deb and rpm
packages also install it as `rwsctl`, and the binary run under that name acts
the same.

```text
rws ctl status                         # version, uptime, listeners, disks
rws ctl reload                         # waits; prints the errors and exits 1 on a bad config
rws ctl drain <upstream> <backend> [--undo]
rws ctl maintenance [on|off]
rws ctl block <ip> | unblock <ip> | blocklist
rws ctl purge <cache> [--path /p | --prefix /p/]
rws ctl caches | upstreams
rws ctl certs [renew]                  # expiry and ACME state; renew orders now
rws ctl logs [-n 100] [-f]             # recent access-log lines; -f follows
rws ctl traffic [--week | --month | --hours N] [--host example.com]
rws ctl whoami                         # the token's user and role
rws ctl audit [-n 50]                  # recent changes: who, what, when
rws ctl audit --verify [FILE]          # check the audit log's hash chain (18.33.0)
```

It finds the server the way the server finds its settings: `--admin
http://127.0.0.1:9901` if given, else `[admin] listen` in `RWS_CONFIG_FILE` or
`./rws.config.toml` (a `0.0.0.0` listener is reached on loopback), else
`127.0.0.1:9901`. The token comes from `--token-file <path>`, else the variable
`[admin] token_env` names, else `RWS_ADMIN_TOKEN`. There is no `--token`: an
argument shows up in `ps` and shell history.

```sh
# on the server, next to its config
sudo -u rws env RWS_CONFIG_FILE=/etc/rws/rws.config.toml rwsctl status
# from a laptop, through a tunnel
ssh -fNL 9901:127.0.0.1:9901 web1
rws ctl --admin 127.0.0.1:9901 --token-file ~/.config/rws/web1.token certs
```

Output is for people (aligned tables, one line per change); `--json` prints the
API's JSON instead. Exit codes: `0` done, `1` the server refused or the
operation failed (a reload with errors, an unknown backend or cache), `2` bad
arguments, the admin API unreachable, or the token refused — so a deploy script
can run `rwsctl reload || rollback`.

## The web UI

Open `http://127.0.0.1:9901/admin/` (through the SSH tunnel on a remote host) and paste the token. It shows:

- **Overview**: uptime, request and error counters, a live requests-per-second chart, the listeners and the build.
- **Traffic**: who visited, people, AI agents and bots, over a day, week or 30 days (18.15.0; see [Traffic insights](/deployment/traffic/)).
- **Upstreams**: every backend's health and circuit breaker, with a switch to drain it.
- **Certificates**: what the server presents and when it expires, and ACME's renew button.
- **Controls**: maintenance mode, reload, the response caches (purge everything, a path or a prefix), authorization tuples (add, remove, check), the IP blocklist and feature flags.
- **Config**: every setting in effect, filterable, secrets hidden.
- **Logs**: the access log as it's written, with pause.

The token stays in the browser tab (`sessionStorage`), and the page talks only to the API below with it. The UI's own files (`/admin/`, `/admin/admin.js`, `/admin/admin.css`) are served without the token: they hold no data. They are built into the binary, so there is nothing to deploy, and they're served with a strict `Content-Security-Policy` (scripts and connections to the listener itself only, no framing).

## Users, roles and the audit log

Since 18.22.0 a team can share the admin API without sharing one token.
The `token_env` token is user `admin` with role `admin`; `users` adds
named tokens, each in its own environment variable:

```toml
[admin]
listen    = "127.0.0.1:9901"
token_env = "RWS_ADMIN_TOKEN"                    # user "admin", role admin
users     = ["deploy-bot:RWS_ADMIN_DEPLOY:operator", "grafana:RWS_ADMIN_GRAFANA:viewer"]
audit_log = "/var/log/rws/audit.log"             # also keep the audit log in a file
```

| Role | May |
|---|---|
| `viewer` | read everything: status, config (secrets hidden), metrics, logs, traffic, upstreams, certificates, the audit log |
| `operator` | also reload, drain backends, switch maintenance mode, block IPs, flip feature flags, purge caches, renew certificates |
| `admin` | also add and remove authorization tuples |

A refused request is `403` saying which role it needs. Tokens must be at
least 16 characters; `rws --check` names an entry that isn't
`name:ENV_VAR:role` or whose variable is unset.

Every change — through the API, the UI, `rws ctl` or the built-in MCP tools
(user `mcp`) — and every refused attempt is recorded: time, user, role, how
they signed in, source address, method, path, the request body with
anything that looks like a secret redacted, and the result. The last 1000
are kept in memory: `GET /admin/api/v1/audit?n=100`, `rws ctl audit`, and the
**Audit** page. With `audit_log` they're also appended to that file as JSON
lines (one per change, for your log shipper). Reads aren't recorded.
`GET /admin/api/v1/whoami` (`rws ctl whoami`) says who a token is.

### A tamper-evident audit log

Since 18.33.0 every line rws appends to `audit_log` ends with two more
fields, which make the file a hash chain:

```json
{"at":1793793600,"time":"…","user":"deploy-bot",…,"node":"web-1","prev":"9f86d0…","hash":"3a7bd3…"}
```

`prev` is the SHA-256 (hex) of the whole line before it in the file (64
zeros for a new file's first line); `hash` is the SHA-256 of this line
without its `,"hash":"…"`. Editing an entry breaks its `hash`; deleting,
inserting or reordering entries breaks the next one's `prev`. Check a file:

```sh
rws ctl audit --verify                 # the server's audit_log, via GET /admin/api/v1/audit/verify
rws ctl audit --verify audit.log.3     # a copy or an archived file, here, without the API
# /var/log/rws/audit.log: intact — 1204 entries, 1204 chained
# /var/log/rws/audit.log: BROKEN at line 731: the entry doesn't match its hash: it was changed
```

`--verify` exits 1 when the chain is broken (`--json` prints the report).

- **Existing files** keep working: lines written before 18.33 have no hash
  and are accepted until the first chained line, whose `prev` covers the last
  of them. A line without a hash *after* the chain started is reported.
- **Restarts** continue the chain from the file's last line. rws keeps the
  hash of what it last wrote in memory, so an edit to the last line while
  it runs is caught by the next entry.
- **Rotation**: the first line of a new file has the last line of the old
  one as its `prev`; the report says `continues_from` (offline, check the
  old file too). Truncating the end of a file can't be detected from the
  file alone — ship it to a log store as it is written (it's JSON lines) and
  keep `rws ctl audit --verify` output alongside.
- One rws process per file: replicas writing one shared file would
  interleave two chains.

### Single sign-on for the admin UI

People can sign in with your identity provider instead of pasting a token,
their role coming from their groups:

```toml
[admin]
listen                 = "127.0.0.1:9901"
oidc_issuer            = "https://id.example.com/realms/main"  # or oidc_provider = "google" | "microsoft" | "github"
oidc_client_id         = "rws-admin"
oidc_client_secret_env = "RWS_ADMIN_OIDC_SECRET"
oidc_cookie_secret_env = "RWS_ADMIN_COOKIE_SECRET"   # ≥ 32 characters; sign-ins survive restarts
oidc_roles             = ["rws-admins=admin", "sre=operator", "*=viewer"]
# oidc_role_claim      = "groups"                    # the ID-token claim with the groups
```

Register `http://127.0.0.1:9901/admin/auth/callback` (or the address you
reach the UI at) as the redirect URI. The login page then shows **Sign in
with single sign-on**. `oidc_roles` maps a group, an email address or `*`
(anyone who can sign in) to a role; a user matching several gets the
highest, one matching none is refused. The session is a signed `HttpOnly`
cookie on the admin origin, valid 12 hours; changes made with it also need
the `X-Rws-Admin` header the UI sends (and pass the origin check), so
another site can't make them. Tokens keep working for scripts and
`rws ctl`. Needs the `sso` feature (in the release binaries).

## Security

- Every request needs `Authorization: Bearer <token>`, on loopback too. A missing or wrong token gets `401`, whatever the path, and the token is compared in constant time.
- A browser request whose `Origin` is neither the listener's own nor in `allowed_origins` gets `403`, so a page on another site can't use a token the browser holds.
- Admin requests are served on threads of their own (at most 32 connections at once), apart from the server's workers, so an overloaded server can still be inspected.
- Responses carry `Cache-Control: no-store`.

## Endpoints

All under `/admin/api/v1/`, JSON in and out.

| Method and path | What it does |
|---|---|
| `GET /status` | Version, process id, uptime, readiness, compile-time features, the main, redirect and admin listeners, and `disk`: each directory rws writes to with `free_bytes`, `total_bytes` and `low` (under `min_free_disk`, 18.15.0) |
| `GET /whoami` | The caller: `{"user":"deploy-bot","role":"operator","via":"token"}` (`via`: `token` or `oidc`; 18.22.0) |
| `GET /audit?n=100` | The newest changes and refused attempts, newest first (at most 1000): `at`, `time`, `user`, `role`, `via`, `address`, `method`, `path`, `detail` (body, secrets redacted), `status`, `node` |
| `GET /audit/verify` | Check the `audit_log` file's [hash chain](#a-tamper-evident-audit-log) (18.33.0): `{"file":…,"ok":true,"entries":12,"chained":10,"unchained":2,"continues_from":null,"broken":null}`; when broken, `"broken":{"line":7,"reason":"…"}`. `404` without an `audit_log` file |
| `GET /auth` | Without credentials: how to sign in, `{"token":true,"oidc":false}` (the UI's login page) |
| `GET /config` | Every `RWS_CONFIG_*` setting in effect; values of names with `SECRET`, `PASSWORD` or `TOKEN` read `(hidden)` |
| `POST /reload` | Reload the configuration and certificates, as `SIGHUP` does, and wait for it (up to 10 s): `{"reloaded":true,"applied":true,"reloads":3,"errors":[]}`. With errors (a config file that doesn't validate) `applied` is `false`, `errors` lists them and the running configuration stays. `503` while the server starts or stops, `504` when it didn't finish |
| `GET /metrics` | Request, error and connection counters, thread-pool queue, readiness (`/metrics` on the main port has the full Prometheus set) |
| `GET /build` | Compile-time features as `{"http3":true,…}` |
| `GET /rate-limit` | The global limit: `configured`, `max_requests`, `window_secs` |
| `GET /rate-limit/check?ip=…` | That address's remaining quota (counts as one request) |
| `GET /cors` | The CORS settings in effect |
| `GET /maintenance`, `PUT /maintenance` `{"active":true}` | Maintenance mode: `503` for everything but health checks |
| `GET /blocklist`, `POST /blocklist` `{"ip":"…"}`, `DELETE /blocklist/<ip>` | The runtime IP blocklist (`403`), for static files and config-file proxy routes alike (proxy routes since 18.20.1). Addresses are checked; the admin listener isn't behind the blocklist |
| `GET /flags`, `PUT /flags/<name>` `{"enabled":true}` | Runtime feature flags |
| `GET /cache`, `DELETE /cache` | Response-cache hits, misses and size; clear it. `404` when the server has no cache |
| `GET /caches` | Every response cache (18.11.0): `name` (`app` for the binary's own, `route:<name>` per config route, `route:#N` for unnamed ones), `kind` (`memory` or `disk`), `entries`, `bytes` (disk only, else `null`), `hits`, `misses` |
| `DELETE /caches/<name>?path=/p` or `?prefix=/p/` | Purge one target (every `Vary` variant) or a prefix; without either, everything. Answers `{"name":…,"purged":n}`; `404` for an unknown name |
| `GET /authz` | The authorization schema (18.14.0): `{"types":{"doc":["owner","viewer"],…}}`. `404` without authorization configured |
| `GET /authz/tuples?object=&relation=&subject=&limit=` | Relationship tuples; `object` is `doc:42` or just a type (`doc`); at most `limit` (100, up to 1000) |
| `POST /authz/tuples` `{"tuple":"doc:42#viewer@user:alice"}` | Add a tuple (checked against the schema): `{"tuple":…,"written":true}` (`false`: already there); `400` when the schema doesn't allow it |
| `DELETE /authz/tuples?tuple=…` (or the same JSON body) | Remove a tuple: `{"tuple":…,"deleted":true\|false}` |
| `POST /authz/check` `{"subject":"user:alice","relation":"viewer","object":"doc:42"}` | `{"allowed":true,"path":[…]}` — the path that allowed it |
| `GET /traffic?hours=24&host=` | Visitors, page views and requests for people, AI agents and bots over the last `hours` (1–2160), the period before, a series and the top lists (18.15.0, [Traffic insights](/deployment/traffic/)). `404` when counting is off |
| `GET /traffic/live?host=` | People on the site in the last 5 minutes, page views per minute for 30 minutes, current pages and countries (18.31.0). `404` when counting is off |
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
