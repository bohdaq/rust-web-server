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
  `localhost`, `*.localhost`, `127.0.0.1`, `::1` and any configured
  names. With no certificate configured and the listener on loopback,
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
`rws --check` in CI (`ci/check_cookbook.sh` extracts the TOML blocks).

## #75 Admin users, roles, audit log, OIDC sign-in

- `[[admin.user]] name = "deploy-bot"  token_env = "RWS_ADMIN_DEPLOY"  role = "operator"`;
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
| 1 | #71 local HTTPS | — |
| 2 | #72 FastCGI | — |
| 3 | #73 OIDC + forward-auth in config | — |
| 4 | #74 cookbook → release 18.21.0 (with the user's go) | — |
| 5 | #75 admin users/roles/audit/OIDC | — |
| 6 | #76 cluster state via Redis | — |
| 7 | #77 WAF → release 18.22.0 | — |
| 8 | #81 LTS + security policy | — |
| 9 | #78 FIPS build | — |
| 10 | #79 PKCS#11 | — |
| 11 | #80 fleet config pull → release 18.23.0 | — |
