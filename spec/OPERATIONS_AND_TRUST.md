[Read Me](../README.md) > [Spec](.) > Operations and trust

# Operations and trust (18.20.0–18.21.0)

Planned 2026-11-01. Four gaps operators run into once rws is the front door:

| # | What | Release |
|---|---|---|
| #67 | systemd `Type=notify` and socket activation | 18.20.0 |
| #68 | `Forwarded` (RFC 7239) and CRLs for inbound mTLS | 18.20.0 |
| #69 | Key rotation for HS256 JWTs and Basic auth | 18.21.0 |
| #70 | `rwsctl`: the admin API from the command line | 18.21.0 |

## #67 systemd notify and socket activation

Today the unit is `Type=simple`: systemd calls rws started the moment the
process exists, before certificates load or listeners bind, so `systemctl
restart` reports success for a server that then fails, and a restart drops
connections while the new process binds.

- **`Type=notify`**: when `NOTIFY_SOCKET` is set, rws sends `READY=1` once
  every listener is bound and the first certificate is loaded (ACME
  included), `RELOADING=1` + `MONOTONIC_USEC` / `READY=1` around a SIGHUP
  reload, `STOPPING=1` on shutdown, and `STATUS=` lines ("serving 3
  listeners", "renewing certificate…"). With `WatchdogSec=` it pings
  `WATCHDOG=1` at half the interval from the main runtime, so a wedged
  process is restarted. A datagram to a Unix socket path or abstract name:
  no libsystemd, ~60 lines.
- **Socket activation**: when `LISTEN_FDS`/`LISTEN_PID` name this process,
  rws takes its TCP and UDP listeners from fds 3… (matched by
  `LISTEN_FDNAMES`, `http`/`https`/`quic`/`redirect`/`admin`, else by local
  address) instead of binding. systemd holds the sockets across restarts:
  nothing is refused while the new process starts, and rws needs no
  `CAP_NET_BIND_SERVICE`. Per-core runtimes (`SO_REUSEPORT` per worker)
  get each worker a `try_clone` of the inherited socket.
- **Packaging**: `packaging/linux/rws.service` → `Type=notify`,
  `NotifyAccess=main`, `WatchdogSec=30`; an optional `rws.socket`
  (`ListenStream=80`, `443`, `ListenDatagram=443`); docs
  `deployment/systemd.md`. Existing units keep working (`Type=simple`
  ignores the notifications).
- **Tests**: a Unix datagram socket standing in for systemd (the messages
  and their order); `ci/e2e/systemd.sh` runs rws under
  `systemd-run --user` (`Type=notify`, then a `.socket` unit) on the CI
  runner and checks `systemctl --user show -p ActiveState`, a request
  answered during a restart, and the watchdog.

## #68 Forwarded and CRLs

- **`Forwarded`**: proxied requests may carry RFC 7239's
  `Forwarded: for=192.0.2.60;proto=https;host=example.com;by=…` next to
  the `X-Forwarded-*` headers rws already sends (IPv6 quoted and
  bracketed, an incoming `Forwarded` extended like `X-Forwarded-For`).
  On by default (decided 2026-11-01, standards first as with problem+json):
  `forwarded_header = "off"` (`RWS_CONFIG_FORWARDED_HEADER`) leaves it out;
  CHANGELOG "Changed" says so, since backends that trust `Forwarded`
  (Spring, Rails) start reading it — with the values `X-Forwarded-*`
  already carry.
- **CRLs for inbound mTLS**: `tls_client_crl_file` (`RWS_CONFIG_TLS_CLIENT_CRL_FILE`,
  PEM or DER, several files comma-separated) → `WebPkiClientVerifier::with_crls`
  (rustls checks them; revoked client certificates fail the handshake),
  re-read on SIGHUP / `watch = true` like certificates, `rws --check`
  validates them (parse, signed by a configured CA, `nextUpdate` not past —
  a stale CRL warns; `tls_client_crl_strict = true` refuses to start/reload
  with one). Metric `rws_tls_client_crl_next_update_seconds`.
- **Tests**: XFF/Forwarded composition (unit, every forwarder);
  rcgen-made CA, client certs and a CRL revoking one → handshake accepted /
  refused; `ci/e2e/mtls_crl.sh` with curl.

## #69 Key rotation for HS256 and Basic

JWKS rotates RS256/ES256 keys already; HS256 secrets and htpasswd files are
single and read once.

- **HS256**: `JwtLayer::new(secret)` keeps working; `JwtLayer::secrets([…])`
  verifies against each (first signs, if rws issues tokens), and
  `[route.middleware.auth] type = "jwt"` takes `secret_env` *and*
  `previous_secret_env` (or `secrets_env = ["A", "B"]`): rotate by adding
  the new secret, moving the old one to `previous`, removing it after the
  longest token lifetime. Tokens can name their key with `kid` →
  exact match first, then each.
- **Basic**: the htpasswd file is re-read when it changes (size+mtime
  checked at most once a second, like config `watch`), so adding or
  removing a user needs no restart; a file that fails to parse keeps the old
  list and logs. Several `htpasswd_file`s may be listed (one per team).
- **Tests**: tokens under old/new/unknown secrets and `kid`s; htpasswd
  edited under a running layer (add, remove, broken file kept).

## #70 rwsctl

The admin API (`/admin/api/v1`, bearer token, its own listener) covers
status, reload, maintenance, blocklist, flags, caches, upstream drain,
certificates, logs and traffic; the web UI and MCP use it. A CLI does too:

```text
rwsctl status                       # version, uptime, listeners, disks
rwsctl reload                       # waits; prints errors and exits 1 on a bad config
rwsctl drain web http://10.0.0.5:8080 [--undo]
rwsctl maintenance on|off
rwsctl block 203.0.113.9 | unblock …
rwsctl purge <cache> [--path /p | --prefix /p/]
rwsctl certs                        # expiry, ACME state; `renew` orders now
rwsctl logs [-f]                    # tail, or follow the SSE stream
rwsctl traffic [--week|--month] [--host h]
```

- **Shape**: `rws ctl …` inside the existing binary (no second artifact
  to package; `rwsctl` is a symlink the packages install, and `rws`
  run as `rwsctl` acts as `rws ctl`).
- **Connection**: `--admin http://127.0.0.1:9901` (default from
  `[admin] listen` in `RWS_CONFIG_FILE`/`rws.config.toml`), token from
  `RWS_ADMIN_TOKEN` or the env var `[admin] token_env` names, or
  `--token-file`; never on the command line (it would show in `ps`).
- **Output**: human tables by default, `--json` for scripts; exit codes
  0 ok, 1 refused/failed, 2 unreachable/auth.
- **Tests**: each command against an in-process admin API; `ci/e2e/admin.sh`
  drives `rws ctl` against the binary.

## Decisions (2026-11-01, the user's)

1. `Forwarded` is sent by default, alongside `X-Forwarded-*`; `forwarded_header = "off"` to leave it out.
2. `rwsctl` is `rws ctl` in the main binary; packages install an `rwsctl` symlink.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | **done 2026-11-01**: `src/systemd` (unix, server builds): `notify` (path or `@abstract`), `ready`/`reloading`/`reloaded`/`stopping`/`status`, watchdog from the tokio runtime (thread in http1) when `WATCHDOG_USEC` (and `WATCHDOG_PID`) say; `LISTEN_FDS`/`LISTEN_PID`/`LISTEN_FDNAMES` read once (vars removed, fds CLOEXEC, kind via `SO_TYPE`), `tcp(addr, name)`/`udp(addr, name)` return a dup matched by name, exact address, then port. Wired: `Server::setup` (name http/https), per-core `bind` (each runtime a dup — a new SO_REUSEPORT socket can't share systemd's port), redirect (`redirect`), QUIC via `Endpoint::new` (`quic`), admin (`admin`); READY next to `SERVER_READY=true` in every main, RELOADING/READY around `config_reload::reload`, STOPPING in `sigterm()` and the http1 shutdown. Packaging: unit `Type=notify` + `NotifyAccess=main` + `WatchdogSec=30` (docs block kept identical), `rws.socket` installed, preremove disables it. Tests: notifications (path + abstract), inherited matching/copies; `ci/e2e/systemd.sh` (CI `systemd`): Python NOTIFY_SOCKET (READY after serving, WATCHDOG, RELOADING/READY on SIGHUP, STOPPING on SIGTERM) + `systemd-socket-activate` | done |
| 2 | **done 2026-11-01** (release waits for the user): `proxy::forwarded_value` (incoming chain + `for` (IPv4 bare, IPv6 quoted/bracketed, mapped → v4, else `unknown`), `proto`, `host` (token or quoted; unsafe hosts skipped)), sent by `build_request` (sync + async), HTTP/2 and HTTP/3 upstreams and Gateway API upgrades; with it on the client's own `Forwarded` isn't passed separately; `forwarded_header` default on (`off`). CRLs: `tls::load_crls` (PEM blocks or one DER, comma-separated), `WebPkiClientVerifier::with_crls` + `allow_unknown_revocation_status` (default) or `enforce_revocation_expiration` (`tls_client_crl_strict`), files watched by `config_watch`, parsed by `--check`. Tests: Forwarded value/chain/off/one line; rcgen CA + CRL → revoked refused, DER + multiple files, junk/missing files, strict refuses a stale CRL. Not done: a curl-based mTLS CRL e2e (the verifier tests use the real rustls verifier) | done |
| 3 | **done 2026-11-01**: `JwtVerifier::Hs256` holds `(kid, secret)`s; `JwtLayer::secrets`, `secret_with_kid`; `verify_with_any` (header `kid` → that secret only, else each); config `previous_secret_env` (+ key schema). `BasicAuthLayer::from_htpasswd_file` takes comma-separated files and re-reads on size/mtime change (≤ 1/s, unreadable keeps the old users). Tests: rotation (new/old/other, kid routing, unknown kid, empty list), htpasswd add/remove/delete with two files, config previous secret end to end | done |
| 4 | #70 `rws ctl` + docs + e2e; release 18.21.0 | — |
