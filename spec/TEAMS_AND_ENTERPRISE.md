[Read Me](../README.md) > [Spec](.) > Teams and enterprise

# Teams and enterprise (18.21.0–18.23.0)

Planned 2026-11-01 from a review of what keeps rws from being the only web
server a solo developer, a startup, a small company and a big organization
need. Each release serves one of those audiences.

| # | What | Audience | Release |
|---|---|---|---|
| #71 | Local HTTPS: `tls internal` and `rws trust` | solo | 18.21.0 |
| #72 | FastCGI (php-fpm) | solo | 18.21.0 |
| #73 | OIDC login and forward-auth in the config file | startup | 18.21.0 |
| #74 | Cookbook: configs for common stacks | startup | 18.21.0 |
| #82 | WebSocket upgrades through proxy routes (found while writing #74) | startup | 18.21.0 |
| #75 | Admin users, roles, audit log, OIDC sign-in | small company | 18.22.0 |
| #76 | Runtime state shared across replicas (Redis) | small company | 18.22.0 |
| #77 | Web application firewall (native rules) | small company | 18.22.0 |
| #78 | FIPS build | big org | 18.23.0 |
| #79 | TLS keys in an HSM (PKCS#11) | big org | 18.23.0 |
| #80 | Fleet config pull | big org | 18.23.0 |
| #81 | LTS and security policy | big org | 18.23.0 |

Not code, so not here: an external security audit, paid support and a track
record at scale (the user's).

## Decisions (2026-11-01, the user's)

1. WAF: rws's own rule engine (built-in detections + TOML rules), not a
   SecLang/OWASP CRS parser and not Coraza.
2. Admin: named tokens with roles and an audit log, **plus** OIDC sign-in to
   the admin UI (roles from a groups claim).
3. Big org: FIPS build, PKCS#11 keys, fleet config pull, LTS + security
   policy docs — all four.
4. Replicas share runtime changes through Redis (the setting ACME and rate
   limits use), not peer-to-peer.

## #71 Local HTTPS

`https://localhost` that browsers trust, without mkcert.

- `tls = "internal"` (`RWS_CONFIG_TLS`; also `[[virtual_host]] tls = "internal"`):
  rws keeps its own CA (EC P-256, 10 years) in the state directory
  (`RWS_CONFIG_STATE_DIR`, default the ACME state dir) and issues leaf
  certificates (7 days, renewed at half-life, hot-swapped) for
  `localhost`, `127.0.0.1`, `::1` and any configured names (no
  `*.localhost`: browsers and webpki refuse wildcards under a single label). With no certificate configured and the listener on loopback,
  `rws --tls-internal` is the one-flag local HTTPS.
- `rws trust` / `rws untrust`: install the CA into the system trust store
  (Debian/Ubuntu `update-ca-certificates`, Fedora/RHEL `update-ca-trust`,
  macOS `security add-trusted-cert`, Windows `certutil -addstore Root`),
  and print the Firefox/NSS note (`certutil -d sql:$HOME/.pki/nssdb`) when
  `certutil` exists. `rws trust --print` writes the CA PEM to stdout.
- Never for public names: `--check` warns when `tls = "internal"` serves a
  name that resolves publicly; ACME wins when both are set.
- Tests: CA + leaf generation, SAN set, renewal; `rustls` client trusting
  the CA → handshake; trust-store commands built but run only in a CI job
  (`ci/e2e/tls_internal.sh`: `rws trust` on the runner, then
  `curl https://localhost` without `-k`).

## #72 FastCGI

PHP (and any FastCGI app) without FrankenPHP.

- Action `type = "fastcgi"` with `address = "unix:/run/php/php8.3-fpm.sock"`
  or `"127.0.0.1:9000"`, `root = "/var/www/app/public"`, `index =
  "index.php"`, `split_path = ".php"`, `try_files = ["{path}", "{path}/index.php", "/index.php?{query}"]`
  (the front-controller pattern of Laravel/Symfony/WordPress), `params =
  { APP_ENV = "prod" }`.
- Protocol: FCGI_BEGIN_REQUEST (responder, keep-conn), PARAMS (CGI/1.1
  variables per RFC 3875: `SCRIPT_FILENAME`, `SCRIPT_NAME`, `PATH_INFO`,
  `DOCUMENT_ROOT`, `REQUEST_URI`, `QUERY_STRING`, `REMOTE_ADDR`, `HTTPS`,
  `HTTP_*`), STDIN streamed, STDOUT parsed as a CGI response (`Status:`,
  `Location:` → 302), STDERR logged. A pool of kept-alive connections per
  address (`max_conns`, default 16). Static files under `root` that aren't
  scripts are served directly.
- Safety: the script must exist under `root` (no `cgi.fix_pathinfo`
  exploit: `/upload/x.jpg/a.php` doesn't run `x.jpg`); dot-files refused.
- Tests: frame codec (fuzz target `fastcgi_record`), a Rust FastCGI test
  responder; `ci/e2e/fastcgi.sh` against real php-fpm (apt) with a
  front-controller app, uploads, redirects, `PATH_INFO`.

## #73 OIDC and forward-auth in the config file

- `[route.middleware.auth] type = "oidc"`: `issuer` (discovery) or
  `provider = "google" | "microsoft" | "okta" | "auth0" | "keycloak" | "github"`,
  `client_id`, `client_secret_env`, `redirect_uri` (default
  `https://<host>/auth/callback`), `scopes`, `allowed_emails`,
  `allowed_domains` (e.g. `example.com`), `allowed_groups` (claim name
  `groups_claim`), `cookie_secret_env` (session cookie signing; sessions
  survive a restart), `exclude = ["/healthz"]`. Wraps `sso::OidcAuth`;
  upstream gets `X-Auth-Request-Email`, `X-Auth-Request-User`,
  `X-Auth-Request-Groups` (oauth2-proxy names, so apps written for it work).
- `type = "forward"`: `url`, `copy_headers = ["X-User-Id"]`, `timeout_ms`
  → `ForwardAuthLayer` (Authelia, Authentik, oauth2-proxy compatible).
- `--check` validates both (issuer reachable unless offline, secrets set).
- Tests: OIDC flow against an in-process mock IdP (discovery, JWKS, token
  endpoint); email/domain/group allow-lists; forward-auth allow/deny/
  header copy; `ci/e2e/oidc_keycloak.sh` with Keycloak in a container on
  the runner (login by form post through curl).

## #74 Cookbook

`docs/.../cookbook/`: one page per stack with a complete, tested config —
static site + SPA fallback, Next.js / Nuxt (node upstream + static
assets), Rails/Django/FastAPI (upstream + static + websockets), Laravel /
WordPress (#72), Grafana/Gitea behind OIDC (#73), API gateway (JWT + rate
limits + CORS), Docker Compose, Kubernetes. Each config is checked by
`rws --check` in CI (a Rust test extracts the TOML blocks and checks each).

## #82 WebSocket upgrades through proxy routes

Found while writing the cookbook: `type = "proxy"` routes dropped `Upgrade`
as a hop-by-hop header, so Rails Action Cable, Phoenix LiveView, Socket.IO
and dev-server hot reload needed a separate `[[ws_proxy]]` listener. The
Gateway API controller already relayed upgrades (`gateway_api::upgrade`);
that moves to `proxy::upgrade` and `DynamicProxy::execute` uses it for
`Connection: upgrade` requests (the async forwarder defers them to the
blocking path, which hands the connection over). Plain-HTTP backends only.

## #75 Admin users, roles, audit log, OIDC sign-in

- `[admin] users = ["deploy-bot:RWS_ADMIN_DEPLOY:operator"]` (a list: the
  settings layer maps tables to env vars, and arrays of tables don't fit);
  the old `token_env` stays as user `admin` with role `admin`.
- Roles: `viewer` (every GET), `operator` (+ reload, drain, maintenance,
  blocklist, flags, cache purge, certificate renewal), `admin` (+ authz
  tuples, config). Refused → `403` with the role it needs.
- Audit log: every non-GET admin request (and denied ones) as a JSON line —
  time, user, role, source IP, method, path, body summary (secrets
  redacted), result — to `[admin] audit_log = "/var/log/rws/audit.log"`
  (rotated like the access log), kept in memory (last 1000), `GET
  /admin/api/v1/audit`, `rws ctl audit`, an Audit page in the admin UI,
  and the MCP tools' changes too.
- OIDC sign-in: `[admin.oidc]` issuer/provider, client id/secret env,
  `role_claim = "groups"`, `roles = { "rws-admins" = "admin", "sre" = "operator", "*" = "viewer" }`.
  The UI gets a "Sign in with …" button; the session is an HttpOnly,
  SameSite=Strict cookie on the admin origin; API calls carry it instead of
  the token (CSRF: the existing origin check + a double-submit header).
- Tests: role matrix over every endpoint, audit lines, redaction,
  OIDC sign-in against the mock IdP, the UI's sign-in in `admin_ui.sh`.

## #76 Runtime state across replicas

- `[cluster] redis = "redis://…"`, `prefix = "rws:"` (defaults to `[acme]
  redis` when set). Blocklist, maintenance mode, drained backends and
  feature flags are stored in Redis hashes and published on a channel;
  every replica subscribes (`SUBSCRIBE` on a dedicated `RespConn`,
  reconnect with backoff) and applies changes within a second, and
  reloads the full state on (re)connect. Local changes write Redis first,
  then apply. Redis down → local changes still apply, logged, synced on
  reconnect (last writer wins, per key with a version counter).
- Cache purges are broadcast (each replica purges its own caches).
- The audit log names the replica (`hostname`/`[cluster] node`).
- `rws ctl status` lists the replicas seen in the last minute (heartbeat
  key with a TTL).
- Tests: two in-process replicas against a real Redis in CI
  (`ci/e2e/cluster.sh`: block on one, refused on the other; drain;
  maintenance; Redis restart).

## #77 WAF (native rules)

- `[waf] mode = "block" | "detect" | "off"` (global) and
  `[route.middleware.waf]` (per route, overrides), `paranoia = 1..3`.
- Built-in detections, each with an id and a score: SQL injection (a
  tokenizer in the spirit of libinjection — fingerprints of keyword/
  operator/string/comment sequences, not regex lists), XSS (tag/attribute/
  `javascript:` / event-handler tokens after HTML-decoding), path traversal
  and local file inclusion, remote file inclusion, command injection
  (shell metacharacters + common binaries), scanners (User-Agent and
  path signatures), protocol anomalies (missing Host, conflicting
  lengths — most already refused by the parser), request limits (args,
  arg length, upload types). Inputs: path, query args, form and JSON body
  values (bounded: first `inspect_body_bytes`, default 64 KiB), cookies,
  selected headers; decoded (percent, `+`, HTML entities, `\u`, null bytes)
  before matching, bounded loops (fuzzed).
- Custom rules in TOML: `[[waf.rule]] id = 1001  match = { path_prefix = "/admin", ip_not_in = ["10.0.0.0/8"] }  action = "deny"`;
  `contains`, `regex` (the `regex` crate, linear time), `field =
  "arg:q" | "header:User-Agent" | "body"`; `action = "deny" | "log" |
  "allow"` (allow = exclusion for false positives), plus
  `[[waf.exclude]] rule = 942100  path_prefix = "/api/search"`.
- Anomaly scoring: block when the request's score ≥ `threshold` (5);
  `403` problem+json with the matched rule ids and a request id, logged as
  a structured `waf` access-log field and `rws_waf_*` metrics; detect mode
  only logs.
- Performance: a request with no hits costs < 2 µs (measured in a bench);
  the WAF is not passthrough, so routes with it use the blocking path —
  documented.
- Tests: a corpus of attack payloads (OWASP CRS regression-test style
  YAML, our own) must be caught at paranoia 1, a corpus of benign traffic
  (real API/form bodies) must pass; fuzz target `waf_inspect`;
  `ci/e2e/waf.sh` with curl.

## #78 FIPS build

- Cargo feature `fips`: rustls with aws-lc-rs's `fips` feature
  (aws-lc-fips-sys, FIPS 140-3 validated module; needs CMake + Go at
  build time), `rustls::crypto::default_fips_provider()`, and
  `ServerConfig::fips()` asserted at startup (refuses to start if not).
  Every in-crate hash/HMAC that isn't aws-lc (`sha2`, `hmac`, own
  `core::sha256`) is routed through aws-lc when `fips` is on, or the
  feature refuses the code paths that can't (documented list).
- Release: a separate `rws-<ver>-x86_64-unknown-linux-gnu-fips.tar.gz`
  (glibc; aws-lc-fips doesn't build for musl) and a `-fips` container
  tag. `rws --version` prints `FIPS`. `/admin/api/v1/status` reports it.
- Tests: CI job builds with `--features fips`, runs the TLS tests, checks
  that a TLS 1.2 non-approved cipher (ChaCha20) is refused.

## #79 TLS keys in an HSM (PKCS#11)

- `tls_key_file = "pkcs11:token=rws;object=web-key?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-source=env:HSM_PIN"`
  (RFC 7512 URI) — also per virtual host. A `rustls::sign::SigningKey`
  backed by `cryptoki` (PKCS#11): ECDSA P-256/P-384 and RSA-PSS/PKCS#1;
  the private key never leaves the token. Sessions pooled per thread;
  login once; reconnect on `CKR_SESSION_HANDLE_INVALID`.
- Feature `pkcs11` (on in release builds; loads the module at runtime).
  Cloud KMS through their PKCS#11 libraries (AWS CloudHSM, Google Cloud
  KMS's `libkmsp11`, Azure Managed HSM) — documented.
- Tests: SoftHSM2 in CI (`ci/e2e/pkcs11.sh`): import a key, serve TLS,
  curl verifies; unit tests with SoftHSM when present.

## #80 Fleet config pull

- `[fleet] source = "https://config.example.com/rws/{node}.toml" | "s3://bucket/rws.toml" | "git+https://…#main:rws.toml"`,
  `interval_secs = 30`, `public_key = "/etc/rws/fleet.pub"` (Ed25519;
  `<config>.sig` next to it, required unless `insecure = true`),
  `node = "web-1"` (default hostname), `labels`.
- A fetched config is verified, written to the config path atomically,
  checked like `rws --check` and applied with the reload path; a refused
  config keeps the old one and is reported back (`POST <source>/status`
  when `report_url` is set: node, version, config hash, applied/errors).
  ETag/If-None-Match so idle polls cost nothing.
- `rws fleet sign <file> --key <private>` / `rws fleet keygen`.
- Runtime state (#76) is the live layer; fleet config is the declared one.
- Tests: an in-process HTTP source, signature good/bad/missing, a bad
  config refused and reported, ETag; S3 via the existing storage-s3
  client against MinIO in CI; git via `git` CLI.

## #81 LTS and security policy

- SECURITY.md: supported versions table (latest minor of 18.x; the LTS
  line), response targets (acknowledge in 3 working days, fix critical
  issues in 14 days), disclosure (coordinated, advisory + CVE via GitHub),
  how fixes are backported, signed releases (Sigstore/cosign attestation
  already published — how to verify).
- LTS: every 6 months a minor is designated LTS and gets security and
  critical fixes for 12 months on a `<minor>.x` branch (18.20 is the
  first, to 2027-11). RELEASE.md describes cutting patch releases from
  an LTS branch. Docs page `reference/support-policy.md`.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | **done 2026-11-01**: `tls::internal` — `LocalCa` (rcgen; CA P-256 10 y in the per-user data dir or `tls_internal_dir`, `ca.key` 0600, issuer rebuilt from `ca.name` since rcgen can't parse without x509-parser), leaf 7 d for loopback + listen address + vhosts + `tls_internal_names` (names validated; no single-label wildcards), written key-first, `start_from_env` before ACME fills empty TLS paths, renewal thread at half-life → `request_reload` (unless `watch`). `rws trust [--print]`/`untrust` (update-ca-certificates / update-ca-trust / trust, sudo when not root; macOS System keychain; Windows user Root; NSS db optional). `--tls` flag, `--check` validates. rcgen moved into `http2`. Tests: 7 unit (CA reuse, 0600, handshake per name via rustls, foreign CA refused, bad names, names(), start_from_env); `ci/e2e/tls_internal.sh` (CI `tls-internal`, runs `rws trust` only on CI). Per-vhost `tls = "internal"` dropped: vhosts without files fall back to this certificate | done |
| 2 | **done 2026-11-01**: `src/fastcgi` — records/params codec, CGI header parsing (Status, Location → 302), `Address` (unix:/path, host:port), pooled keep-conn connections (retry once on a stale pooled one), `FastCgi` app: resolve order file → dir index (redirect without slash) → `/x.php/info` → front controller; scripts must be regular files under the canonical root ending in `split_path`, no dot segments; CGI/1.1 vars incl. HTTPS, `HTTP_*` minus `Proxy` (httpoxy), `REDIRECT_STATUS`; stderr to the log; 502 on failure. Config `type = "fastcgi"` (short or nested; `[route.action.params]` any keys), static files via `StaticAdapter`, `--check` validates address/root. Bodies and responses buffered (no `try_files` list: the fixed order above covers the frameworks). Tests: 7 unit (in-process responder: vars, keep-alive reuse, status/location, 502, static, redirect), 2 config; fuzz `fastcgi_record`; `ci/e2e/fastcgi.sh` against real php-fpm (CI apt php-fpm; locally `docker` php:8.3-fpm): front controller, PATH_INFO, dir index, form + 300 KB upload, cookies, redirects, 404, static not sent to fpm, upload-as-PHP refused, fpm down 502, Unix socket | done |
| 3 | **done 2026-11-01**: `OidcAuth` — `signed_cookies` (HMAC-SHA256 `_rws_oidc`, pre-auth `_rws_oidc_pre` 10 min), `allow_emails/domains/groups` (verified email only; groups from the verified ID token's claim), `forward_headers` (X-Auth-Request-*), `session_ttl`, host-derived redirect_uri, client identity headers stripped everywhere, local-only return_to. Config `type = "oidc"` via `proxy_config::oidc_route::LazyOidc` (discovery on first request, 503 + retry 10 s), `type = "forward"` (ForwardAuthLayer: copied headers only from the service, X-Forwarded-* set by rws), unknown auth type → 503 + check error. Release + CI binaries add `sso`. Found + fixed: http_client `Host` lacked non-default ports. Tests: 6 OidcAuth (signed flow across instances, tamper/other key, allow lists, unverified email, store mode groups, redirect_uri/return_to, spoofed headers), 2 forward, 3 config, Host port; `ci/e2e/oidc.sh` (CI `oidc`): Keycloak 26 in docker, curl through the login form, groups allow, 403 for others, restart keeps session, logout, forward-auth with a Python service. Not done: provider presets okta/auth0/keycloak (use `issuer`) | done |
| 4a | **done 2026-11-01** #82: relay moved to `proxy::upgrade` (gateway re-exports it), `WebSocket::upgrade_raw` ungated, `ConfigDrivenApp::execute_async` returns `None` for upgrades, `DynamicProxy::forward_upgrade` (prepare → backend, host override, read timeout; TLS backends 502). Tests: 2 unit (101 + relay with path rewrite and X-Forwarded-For, dead backend 502); `ci/e2e/ws_upgrade.sh` (CI `ws-upgrade`): Python RFC 6455 echo backend + client through rws, plain and TLS (`tls = "internal"`), 8 concurrent sockets, ordinary requests on the same route, dead backend 502 | done |
| 4 | **done 2026-11-01** #74: `docs/…/cookbook/` — overview + 8 recipes (static site, SPA + API, Node, Python/Ruby, PHP, internal tools with OIDC + forward-auth, API gateway, local dev), sidebar group; `config_check::tests::cookbook_configs_pass_check` runs every TOML block through `check_str_offline` (needs acme + auth + sso: CI's feature-set run). Docker Compose / Kubernetes recipes not added (deployment/docker.md and kubernetes*.md cover them). Release 18.21.0 waits for the user | done |
| 5 | **done 2026-11-01**: `admin::access` (Role, TokenUser/`parse_users`, `parse_role_map`/`role_for`, `required_role`, global audit ring 1000 + JSON-lines file, `redact`, `McpAuditLayer` for the binary's changing MCP tools); AdminApi: users, `authenticate` (tokens compared in constant time against all; OIDC via `LazyOidc` with `path_prefix=/admin` + new `OidcAuth::signed_in`), `/admin/auth/*`, public `GET /auth`, `GET /whoami`, `GET /audit`, CSRF `X-Rws-Admin` for cookie writes, role check, audit of changes/refusals; settings `[admin] users, audit_log, oidc_*` + `--check`. UI: SSO button, user · role in the header, X-Rws-Admin + same-origin credentials, Audit page (`admin-ui/build.sh`). `rws ctl whoami`/`audit`. Tests: 10 admin (users, role map, role matrix, audit+redaction, file, auth methods, check, MCP audit), ctl whoami/audit; `oidc.sh` admin sign-in via Keycloak (ops → operator, CSRF, tuples refused, unmapped refused, audited); `admin_ui.sh` header + Audit page | done |
| 6 | #76 cluster state via Redis | — |
| 7 | #77 WAF → release 18.22.0 | — |
| 8 | #81 LTS + security policy | — |
| 9 | #78 FIPS build | — |
| 10 | #79 PKCS#11 | — |
| 11 | #80 fleet config pull → release 18.23.0 | — |
