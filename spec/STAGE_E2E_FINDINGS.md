[Read Me](../README.md) > [Spec](.) > Stage E2E findings

# Stage E2E findings

What the stage end-to-end suite (`ci/stage/`, design: the "rws on kube-dc
stage — E2E test design" doc) found, one row per finding, keyed by case id so
a rerun shows what is fixed. Kinds: **bug** (wrong behaviour), **gap**
(documented or expected but missing), **incomplete** (partly works),
**blocked** (a kube-dc fault, not rws).

| Case | Kind | Finding | Status |
|---|---|---|---|
| H0 chart config | gap | The Helm chart had no way to give rws a `rws.config.toml`: no ConfigMap mount, no extra volumes, so a chart user couldn't configure routes, upstreams or middleware at all (env vars only). | fixed: `configFile`, `configMountPath`, `extraVolumes`, `extraVolumeMounts` (18.27.0) |
| S3.ip client address | bug | Behind the kube-dc gateway every connection comes from its address (172.30.0.3), and rws keyed the rate limit, IP filter, blocklist, WAF IP rules, `ip_hash` and the logs on it: all clients shared one rate-limit bucket, `allow_ips` couldn't name a client. | fixed: the client from a trusted proxy's `X-Forwarded-For`, `client_ip = "peer"` to opt out (18.27.0) |
| S2.timeout | bug | A backend slower than `read_timeout_ms` got `502 Bad Gateway`; the design (and nginx) say `504`, and `Proxy-Status` already said `http_response_timeout`. | fixed (18.27.0) |
| S2.forwarded | incomplete | Behind a TLS-terminating gateway rws sends `X-Forwarded-Proto: https` (the gateway's, believed) but its own `Forwarded: for=<gateway>;proto=http`, so the two disagree. | fixed (18.27.0) |
| S3.3 basic challenge | bug | Basic auth answered wrong credentials with a bare `401`: no `WWW-Authenticate`, so a browser shows the error instead of asking again. | fixed (18.27.0) |
| S1.6 ETag per replica | incomplete | Static ETags come from the file's mtime, which differs per pod for the same ConfigMap (or image layer unpacked at different times): `If-None-Match` gets a `304` only from the pod that made the ETag, 1 in 3 behind the gateway. | fixed (18.27.0) |
| S1.18 CORS on routes | bug | `[cors]` ("applied to every response") is only applied to static files: `[[route]]` responses (respond, proxy) get no `Access-Control-*` headers and a preflight `OPTIONS` goes to the route. | fixed (18.27.0) |
| S8.4 public metrics | gap | `/metrics` is served on the main listener to anyone: through the public gateway it shows upstream names, backend addresses and traffic counts. There's no setting to keep it to scrapers. | fixed (18.27.0) |
| S8.9 drain | incomplete | One run of a rolling restart under traffic got one `503` from the gateway (699 × 200): on `SIGTERM` rws stops accepting at once, before the gateway has taken the pod's endpoint out. A second run was clean. | fixed (18.27.0) |
| S9.4 FastCGI script path | bug | rws sends php-fpm `SCRIPT_FILENAME` (and `DOCUMENT_ROOT`) with symlinks resolved (`canonicalize`). php-fpm in another container sees the same files under the same path but not the same symlink targets (a ConfigMap volume's `..2026_…` directory differs per pod), so PHP answers `File not found.` for every script. nginx sends `$document_root$fastcgi_script_name` (lexical) by default. | fixed on main: the configured path, symlinks left in |
| S5 DB TLS | gap | The model layer had no TLS settings: a managed PostgreSQL that requires TLS from its own CA could only be reached unverified (sqlx `prefer`). | fixed on main (428482f0): `RWS_DB_SSL_*`, `DbTls` |
| S5 DB TLS panic | bug | A PostgreSQL/MySQL pool panicked at its first TLS connection (`rustls`: no process crypto provider) in an app that serves plain HTTP behind a load balancer: only rws's TLS listener installed the provider. | fixed on main (428482f0) |
| S5 DB URL | bug | User and password went into a `postgres://user:pass@…` URL unescaped: `@`, `:`, `/`, `#` in a generated password broke the connection. | fixed on main (428482f0) |
| S5 S3 port | bug | S3 signed `Host` without a non-default port and sent it with one: an endpoint on `:9000` (MinIO) got `SignatureDoesNotMatch` for every request. Found by the test app build and by reading the code. | fixed on main (428482f0) |
| S5 S3 CA | gap | No way to trust a private CA for an `https://` S3 endpoint (the HTTP client knew only the public roots). | fixed on main (428482f0): `RWS_S3_CA_FILE`, `Client::ca_file` |
| S5 jobs/sessions SQL | bug | `jobs::PersistentJobQueue` and `session::DbSessionStore` write `?` placeholders, passed to PostgreSQL unchanged: `enqueue`, `tick` and `DbSessionStore::create` fail with a syntax error there. `recover_interrupted` resets every `running` job at startup, so a restarting replica re-runs jobs another one is still running. Reported by the test app build (against PostgreSQL 17). | open |
| S5 row decoding | bug | The PostgreSQL row decoder reads `INT4`, `NUMERIC` and `TIMESTAMPTZ` columns as NULL, so `INTEGER`/`SERIAL` columns can't be read. Reported by the test app build. | open |
| S5 migrate race | bug | `DbPool::migrate` takes no lock: replicas starting together fail with a duplicate-key error on `pg_type`. Files are split on `;`. Reported by the test app build. | open |
| S5 presign | gap | `S3Storage` can't make presigned URLs. | open |
| S3 query decoding | bug | `extract::Query` (url-search-params 12) decodes only a fixed list of upper-case escapes (`%3C`, `%C3%A9`, `+` stay), decodes `%25` first (`%2526` → `&`) and cuts a value at its second `=`. Reported by the test app build. | open |
| S4 IdP and RP | gap | rws as IdP (`AuthServer`) and rws as OIDC client (`OidcAuth`) don't work together: the server signs HS256 with an empty JWKS, puts no `nonce` in the token and has no userinfo endpoint, while the client verifies only RS256/ES256 from the JWKS and checks the nonce. Its session cookie name is a private constant a login page must copy; codes are in memory (one replica). | open |
| S3 CSRF scope | gap | `CsrfLayer` has no path scoping (on an app it refuses every unsafe request), and `CsrfToken::from_request` reads the first `X-Rws-Csrf-Token` header, which a client can send itself. | open |
| S5 async pool | bug | `AsyncAppWithState` runs each request on a throwaway tokio runtime (`block_on_isolated`), so a pooled sqlx connection made in one request belongs to a dead runtime in the next. Reported by the test app build. | open |
| S7.5 A2A card scheme | bug | Behind the TLS-terminating gateway the A2A agent card's interface URL was `http://…`, so clients called the agent over plain HTTP; OIDC's default redirect URI and cookie `Secure` flag had the same cause. | fixed on main (0ef3c4bb): `ip_filter::request_scheme` |
| S7 MCP sessions per replica | gap | `McpServer` keeps sessions in memory per replica: behind a Service without affinity (3 app replicas), rws's MCP gateway gets `session not found` on most calls and lists no tools. The same for rws's own MCP gateway route on 3 replicas behind the kube-dc gateway: a session made on one pod is `404 Session not found` on the next connection (S7.0). MCP 2026-07-28 (stateless) works on every replica; session-based clients need sticky routing, or sessions in [cluster] Redis. The suite reaches the app through a `sessionAffinity: ClientIP` Service and uses stateless MCP for S7.1-4. | open |
| S6.5 gateway 15 s | blocked | The kube-dc gateway ends every response after 15 s, even an SSE stream sending an event every 5 s (Envoy's default route timeout); WebSockets aren't affected. In the cluster the same stream lasts. | kube-dc |
| S5 bucket endpoint | blocked | A kube-dc bucket's ConfigMap names `rook-ceph-rgw-my-store.rook-ceph.svc` as `BUCKET_HOST`, which a project's pods can't resolve or reach; `https://s3.stage.kube-dc.com` works. | kube-dc |

Rerun on 18.27.0 (2026-11-03): 50 ok, nothing known or failing.
