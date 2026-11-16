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
rws ctl drain <upstream> <backend> --wait 2m       # returns when its last request ended (18.38.0)
rws ctl maintenance [on|off]
rws ctl maintenance on --for 1h --retry-after 5m   # off by itself after an hour (18.38.0)
rws ctl maintenance extend 1h          # keep a timed maintenance on longer (18.38.0)
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

- **Overview**: requests per second with a live chart, errors, open connections and the queue (the admin listener's own requests aren't counted; `[admin] count_own_requests = true` counts them), what needs attention (failing or drained backends, an open circuit breaker, certificates about to expire, a full disk), the response time (p50, p95 and p99 over the last five minutes the page has been open, since start before that), where requests go (the busiest hosts and each upstream, with its share, its errors and the time 95% of its requests stayed under), the newest requests, the listeners and the build.
- **Traffic**: who visited, people, AI agents and bots, over a day, week or 30 days (18.15.0; see [Traffic insights](/deployment/traffic/)).
- **Upstreams**: every backend's share of traffic, health and circuit breaker, the requests open on it, a button to drain it or put it back, and the recent drains. A drained backend gets a card on top: how many requests are still running on it, then "safe to deploy" when none is left (since 18.38.0).
- **Certificates**: what the server presents and when it expires, and ACME's renew button.
- **Controls**: maintenance mode, reload, the response caches (purge a path or a prefix, or everything), authorization tuples (add, remove, check), the IP blocklist and feature flags.
- **Config**: every setting in effect, filterable, secrets hidden. With `[admin] edit_config = true`, an admin also [edits the configuration file](#editing-the-configuration-file) there (18.38.0).
- **Logs**: the access log as it's written, as columns, with a filter, status chips, pause and download.
- **Audit**: every change made through the API, this UI or the MCP tools, and every refused attempt.

The header shows one state on every page: Ready, Not ready, Degraded (healthy backends out of all), Maintenance (with a banner and a Turn off button) or Disconnected (the admin API stopped answering; the time of the last answer, retried every 5 seconds). Changes that take a site down ask first: turning maintenance mode on (type `maintenance` to confirm), purging a whole cache, draining the last healthy backend of an upstream. Draining, blocking and unblocking can be undone from the message that confirms them.

Turning maintenance mode on also asks two things (18.38.0): what clients are told to wait (`Retry-After`: the configured value, 5 minutes, 15 minutes or 1 hour) and whether it turns off by itself (after 1 hour, 4 hours, or never). With a time set, the banner shows when it ends, with **Add 1 hour** and **Turn off now**; when the time runs out the audit log gets an entry by `timer`. `Retry-After` never points past the time maintenance ends. With `[cluster]`, the replica that was asked keeps the timer and tells the others when it runs out; if that replica restarts first, turn maintenance off by hand.

`Ctrl K` (`⌘ K` on a Mac) or `/` opens a command palette: go to a section, drain a backend, purge a cache, find a setting. `G` then a letter goes to a section; the letters are shown in the sidebar.

The token stays in the browser tab (`sessionStorage`), and the page talks only to the API below with it. The UI's own files (`/admin/`, `/admin/admin.js`, `/admin/admin.css`) are served without the token: they hold no data. They are built into the binary, so there is nothing to deploy, and they're served with a strict `Content-Security-Policy` (scripts and connections to the listener itself only, no framing).

### Editing the configuration file

Since 18.38.0 the **Config** page can change `rws.config.toml` itself. It is
off until you ask for it:

```toml
[admin]
edit_config = true
```

A reload turns it on. To turn it off, set it to `false` and reload (a line
that is removed keeps its value until a restart).

Then the page shows the file in an editor. As you type, the edits are
*staged*: nothing is written, the server keeps running the file on disk, and
the page shows

- **what will change**: the lines added and removed, against the file as it
  is on disk;
- **whether it is valid**: the same checks as `rws --check`, with the line and
  column of a mistake. A file that doesn't pass can't be applied;
- **what a reload won't apply**: settings read when the process starts
  (`port`, `ip`, `thread_count`, the certificate paths, the `[admin]`,
  `[fleet]` and `[cluster]` tables, TCP, UDP and WebSocket proxies) and
  settings taken out of the file are named, so you know a restart is still
  needed for them.

**Apply** asks once, then rws checks the text again, keeps the file as it was
in `rws.config.toml.bak` (same directory, same permissions), writes the new
one and reloads. Open connections are kept. If the reload refuses the new
file, the previous one is put back and reloaded, and the page says why.
**Discard** drops the edits (with Undo).

What to know before turning it on:

- **It is the admin role's**, reading included: the file can hold secrets, and
  whoever writes it can add a route that serves any directory the rws user
  reads. `viewer` and `operator` get `403`. Don't turn it on for an admin
  listener that people you wouldn't give a shell reach.
- **The edits live in the page**: not in the browser's storage, not on the
  server. Leaving the page with edits asks first; the sidebar marks Config
  while there are some.
- **Nobody's change is overwritten silently.** A write names the version it
  was based on. If the file changed on disk meanwhile (another admin, your
  deploy tool), the write is refused and the page offers to compare your
  edits with the new file.
- **The audit log has the change, not the text**: who, when, the SHA-256 of
  the file before and after, and its size.
- It isn't offered when rws runs without a configuration file, when
  [`[fleet]`](/deployment/fleet/) replaces the file from its source, when the
  rws user can't write the file, or for a file over 1 MB.
- rws must be able to create files next to the configuration file (the
  backup, and a temporary file that replaces the old one in one step). Where
  it can't, the file is rewritten in place.

With several replicas, each one edits its own file: use `[fleet]` to give
them one configuration.

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
| `admin` | also add and remove authorization tuples, and read and write the configuration file when [`edit_config`](#editing-the-configuration-file) is on |

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
- The API can't change the configuration file unless `[admin] edit_config = true`, and then only as role `admin` (18.38.0). Without it an admin token switches what the configuration already allows; with it, it changes the configuration.

## Endpoints

All under `/admin/api/v1/`, JSON in and out.

| Method and path | What it does |
|---|---|
| `GET /status` | Version, process id, uptime, readiness, `maintenance` and `maintenance_until` (when it turns off by itself, Unix seconds or `null`; 18.38.0), compile-time features, the main, redirect and admin listeners, and `disk`: each directory rws writes to with `free_bytes`, `total_bytes` and `low` (under `min_free_disk`, 18.15.0) |
| `GET /whoami` | The caller: `{"user":"deploy-bot","role":"operator","via":"token"}` (`via`: `token` or `oidc`; 18.22.0) |
| `GET /audit?n=100` | The newest changes and refused attempts, newest first (at most 1000): `at`, `time`, `user`, `role`, `via`, `address`, `method`, `path`, `detail` (body, secrets redacted), `status`, `node` |
| `GET /audit/verify` | Check the `audit_log` file's [hash chain](#a-tamper-evident-audit-log) (18.33.0): `{"file":…,"ok":true,"entries":12,"chained":10,"unchained":2,"continues_from":null,"broken":null}`; when broken, `"broken":{"line":7,"reason":"…"}`. `404` without an `audit_log` file |
| `GET /auth` | Without credentials: how to sign in, `{"token":true,"oidc":false}` (the UI's login page) |
| `GET /config` | Every `RWS_CONFIG_*` setting in effect; values of names with `SECRET`, `PASSWORD` or `TOKEN` read `(hidden)` |
| `GET /config/file` | The configuration file, for role `admin` (18.38.0): `{"enabled":true,"path":"/etc/rws/rws.config.toml","editable":true,"reason":null,"content":"…","sha256":"…","modified":1763290000}`. Without `[admin] edit_config = true`, or when the file can't be edited, `content` is `null` and `reason` says why |
| `POST /config/file/check` | `{"content":"…"}`: what `rws --check` says about that text, and what a reload wouldn't apply: `{"ok":true,"errors":[],"warnings":[],"restart":["port"]}`. Nothing is written, nothing is recorded |
| `PUT /config/file` | `{"content":"…","sha256":"<of the file it was based on>"}`: check, keep the previous file as `<file>.bak`, write, reload: `{"applied":true,"changed":true,"path":…,"sha256":…,"backup":…,"restart":[],"warnings":[]}`. `403` when editing is off, `409` when the file changed on disk since (or can't be edited), `413` over 1 MB, `422` with `errors` when it doesn't pass the check (nothing written), `500` when the write failed or the reload refused it (the previous file is back) |
| `POST /reload` | Reload the configuration and certificates, as `SIGHUP` does, and wait for it (up to 10 s): `{"reloaded":true,"applied":true,"reloads":3,"errors":[]}`. With errors (a config file that doesn't validate) `applied` is `false`, `errors` lists them and the running configuration stays. `503` while the server starts or stops, `504` when it didn't finish |
| `GET /metrics` | Request, error and connection counters, thread-pool queue, readiness (`/metrics` on the main port has the full Prometheus set) |
| `GET /breakdown` | Response times and where requests went, since start: `duration` (`count`, `sum` in seconds, `p50`, `p95`, `p99` in seconds or `null` before the first request, and the histogram itself: `buckets` from index `offset`, where bucket *i* holds requests that took up to 2^(*i*/8) seconds, plus `zero`), `hosts` (`host`, `requests`, `errors` — `5xx` answers — for up to 200 `Host` names, busiest first; the rest are added up in `hosts_other`; an empty name is requests without a `Host`) and `upstreams` (`name`, `requests`, `errors` — `5xx` or no answer —, and `duration.buckets`: how many took up to each of `upstream_bounds` seconds). An upstream counts every try, so a retried request is there twice. The admin listener's own requests aren't counted. Take two answers and subtract for a time window |
| `GET /build` | Compile-time features as `{"http3":true,…}` |
| `GET /rate-limit` | The global limit: `configured`, `max_requests`, `window_secs` |
| `GET /rate-limit/check?ip=…` | That address's remaining quota (counts as one request) |
| `GET /cors` | The CORS settings in effect |
| `GET /maintenance`, `PUT /maintenance` `{"active":true}` | Maintenance mode: `503` for everything but health checks. Answers `{"active":true,"since":1794822000,"until":null,"retry_after_secs":120}` (`since`, `until` and `retry_after_secs` since 18.38.0; times are Unix seconds). With `"for_secs":3600` it turns off by itself after that long (up to 30 days) and `until` says when; `"retry_after_secs":300` is the `Retry-After` to send meanwhile instead of `[maintenance] retry_after_secs` (up to a day, `0` for none) |
| `PUT /maintenance` `{"add_secs":3600}` | Keep a timed maintenance on that much longer (18.38.0). `409` when it is off or has no time set |
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
| `GET /traffic?hours=24&host=` (or `?from=YYYY-MM-DD&to=YYYY-MM-DD`) | Visitors, page views and requests for people, AI agents and bots over the last `hours` (1–2160), the period before, a series and the top lists (18.15.0, [Traffic insights](/deployment/traffic/)). `404` when counting is off |
| `GET /traffic/query?from=&to=&host=&path=&country=&channel=&utm_source=&utm_campaign=&browser=&os=&device=` | Page views from the traffic database (`[traffic] database_url_env`) for any days and filters (18.31.0, [Traffic insights](/deployment/traffic/#keeping-every-page-view-in-postgresql-optional)). `404` without a database, `400` for a bad date |
| `GET /traffic/owner` | The owner link's path (`/_rws/me?k=<key>`) and the configured sites to open it on; a browser that opens it counts as the owner's (18.35). `404` when counting is off |
| `GET /traffic/live?host=` | People on the site in the last 5 minutes, page views per minute for 30 minutes, current pages and countries (18.31.0). `404` when counting is off |
| `GET /routes` | The application's routes (method and pattern) |
| `GET /requests?n=20`, `GET /errors?n=20` | The last requests, or the last 4xx/5xx (at most 100) |
| `GET /upstreams` | The config file's `[[upstream]]`s: each backend's `url`, `weight`, `healthy` (passing its health checks), `breaker` (`closed`, `open`, `half_open`, or `null` without a circuit breaker), `drained`, `drained_since` (Unix seconds, `null` when not drained) and `open_requests` (requests this process has running on the backend right now, with streamed responses and WebSockets counted until they end; both since 18.38.0). Upstreams are listed by name |
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

A typical backend rollout: drain it, wait until its `open_requests` is `0`, deploy, check `GET /upstreams` shows it `healthy`, undrain. `rws ctl drain <upstream> <backend> --wait 2m` does the first two steps: it drains, then returns `0` when no request is left on the backend, or `1` when some still run after the time given (the backend stays drained).

```sh
rws ctl drain web http://10.0.0.5:8080 --wait 2m && deploy-backend 10.0.0.5
rws ctl drain web http://10.0.0.5:8080 --undo
```

`open_requests` counts what this rws process sent: with [several replicas](/deployment/cluster/) the drain is shared, the count isn't, so wait on each replica (or watch `sum(rws_upstream_backend_open_requests)` in Prometheus).

Maintenance mode applies in proxy mode too (the binary puts the maintenance check in front of config-file routes since 18.10.0; while it's off, proxy routes keep their non-blocking forwarder).

Changes made here are in memory: they last until the process restarts (a reload doesn't undo them). The binary's [MCP tools](/mcp/operate/) run the same code, so both answer alike.

Library users: the admin API reports the cache and routes registered with `admin::ops::register_cache` and `register_routes`.

Example `GET /status`:

```json
{"version":"18.10.0","pid":4242,"uptime_secs":3605,"ready":true,"maintenance":false,"features":["http2","http3","acme","auth","mimalloc"],"listeners":{"main":{"address":"0.0.0.0:443","tls":true},"redirect":"80","admin":"127.0.0.1:9901"}}
```

`POST /admin/config/reload` on the main listener (the older reload endpoint) is unchanged.
