---
title: Environment Variables
description: Complete reference for every RWS_CONFIG_* environment variable, with defaults and descriptions.
---

All configuration in `rust-web-server` is ultimately stored as process
environment variables. The variables below can be set before the process starts
(e.g. in a shell, a `docker run -e` flag, a Kubernetes `env:` block, or a
`systemd` unit file). Settings in `rws.config.toml` and CLI flags map to the
same variables and will override anything set in the environment.

See the [Configuration Overview](/configuration/overview) for the full priority
order.

## Server

| Variable | Default | Type | Description |
|----------|---------|------|-------------|
| `RWS_CONFIG_IP` | `0.0.0.0` | string | IP address the server binds to. `0.0.0.0` makes the server reachable from any interface, including inside containers and Kubernetes pods. Override to `127.0.0.1` for local-only development. |
| `RWS_CONFIG_PORT` | `7878` | integer | TCP port the server listens on. |
| `RWS_CONFIG_THREAD_COUNT` | `200` | integer | Size of the worker thread pool (synchronous HTTP/1.1 build only). |
| `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` | `10000` | integer | Size in bytes of each individual socket read. Bodies larger than this are accumulated across multiple reads automatically (as long as `Content-Length` is present) — this tunes read-syscall chunk size, not the maximum accepted body size. |
| `RWS_CONFIG_SERVE_DOTFILES` | `false` | boolean | Serve files and directories whose name starts with `.` (`.git/`, `.env`). Off by default since 17.113.1; `.well-known/` is always served. Read at startup. |
| `RWS_CONFIG_OPEN_FILE_CACHE_SECS` | `1` | integer | How long an open static file is reused before its path is checked again (like nginx's `open_file_cache`). `0` turns the cache off. Edits in place are always served correctly; a replaced or deleted file is noticed within this many seconds. At most 128 files are held open. Read at startup; Unix only. |
| `RWS_CONFIG_OPEN_FILE_CACHE_STRICT` | `true` | boolean | `true`: every static request checks the cached file's size and modification time (one `fstat`), so an edit in place is served at once. `false`: that check is reused for `RWS_CONFIG_OPEN_FILE_CACHE_SECS`, like nginx's `open_file_cache_valid` — about 0.7 µs less per request, but a file edited in place can be served stale (old size or bytes) for up to that long. Read at startup. |
| `RWS_CONFIG_RUNTIME` | `per-core` (`rws` binary; `shared` for library apps) | string | Async builds, Unix only. The `rws` binary runs `per-core` since 18.0.0 (`shared` before); an application built on the library stays `shared` unless it sets this. `shared`: one multi-threaded tokio runtime (`TOKIO_WORKER_THREADS` workers) takes every connection. `per-core`: one single-threaded runtime per worker, each accepting on its own `SO_REUSEPORT` socket on the same port, like nginx's worker processes — a connection stays on one thread, with no work stealing or cross-thread wake-ups. On fast CPUs that's more throughput (on the benchmark's fastest runners +12–16% requests per second, and 4% less CPU per proxied request); on slower ones it costs about 12% more CPU per small request for a few percent more throughput. HTTP/3, the redirect listener and ACME stay on the main runtime; SIGHUP and shutdown work as usual. Unset with a `pkcs11:` TLS key, the binary uses `shared` (since 18.25.0; the token's wait can be moved off a shared worker, not a per-core one). Read at startup. |
| `RWS_CONFIG_STATIC_CACHE_CONTROL` | `no-cache` | string | `Cache-Control` sent with static files (since 18.0.0; other responses keep `no-store, …`). The default lets browsers keep a file and revalidate it with `If-None-Match` / `If-Modified-Since`, which costs a `304` instead of the whole file. Set e.g. `public, max-age=3600` for assets with versioned names; an empty value sends the `no-store` default for static files too. |
| `RWS_CONFIG_ERROR_FORMAT` | `problem` | string | Body of the errors rws produces itself (since 18.17.0): RFC 9457 `application/problem+json`, or `text` for the plain-text bodies of earlier releases. See [Error responses](/reference/errors/). |
| `RWS_CONFIG_PROXY_STATUS` | `always` | string | `Proxy-Status` (RFC 9209) on proxied responses (since 18.17.0): `always`, `errors` (failures only) or `off`. See [Proxy-Status](/proxy/reverse-proxy/#when-a-backend-fails-proxy-status). |
| `RWS_CONFIG_PROXY_NAME` | `rws` | string | This hop's name in `Proxy-Status` and `Cache-Status`. |
| `RWS_CONFIG_PROXY_STATUS_NEXT_HOP` | `false` | boolean | Add the backend address (`next-hop`) and the failure's text (`details`) to `Proxy-Status`; they reveal internal addresses. |
| `RWS_CONFIG_CACHE_STATUS` | `true` | boolean | `Cache-Status` (RFC 9211) on responses that pass through a response cache: `hit; ttl=…`, `fwd=uri-miss; stored`, … (since 18.17.0). See [Caching](/features/caching/#cache-status-header). |
| `RWS_CONFIG_TRUSTED_PROXIES` | loopback + private ranges | list of addresses / CIDR | Peers whose `X-Forwarded-*` and `Forwarded` headers are passed on; from others they're replaced (since 18.24.0). `*` trusts everyone, `none` no one. See [Reverse proxy](/proxy/reverse-proxy/). |
| `RWS_CONFIG_CLIENT_IP` | `forwarded` | `forwarded` / `peer` | Which address is the client: behind a `trusted_proxies` peer, the first untrusted address from the right of `X-Forwarded-For` (or `Forwarded: for=`); `peer` keeps the TCP peer. Rate limits, IP filters, the blocklist, WAF, `ip_hash` and logs use it (since 18.27.0). See [The client address](/proxy/reverse-proxy/#the-client-address). |
| `RWS_CONFIG_METRICS_ALLOW` | loopback + private ranges | list of addresses / CIDR | Clients `GET /metrics` answers (the client as [`client_ip`](/proxy/reverse-proxy/#the-client-address) sees it); others get `404` (since 18.27.0). `*` everyone, `none` no one. |
| `RWS_CONFIG_SHUTDOWN_DELAY_SECS` | `5` | seconds | On SIGTERM, `/readyz` answers `503` at once but rws keeps accepting this long before it drains, so a load balancer takes it out of rotation first; `0` drains at once (since 18.27.0). See [Graceful shutdown](/deployment/kubernetes/#graceful-shutdown). |
| `RWS_CONFIG_FORWARDED_HEADER` | `add` | string | Proxied requests carry RFC 7239 `Forwarded` beside `X-Forwarded-*` (since 18.20.0); `off` leaves it out and passes the client's through. |
| `RWS_CONFIG_TLS_CLIENT_CRL_FILE` | (empty) | paths | Certificate revocation lists for client certificates (mTLS), PEM or DER, comma-separated; revoked certificates fail the handshake (since 18.20.0). See [mTLS](/features/mtls/#revoked-certificates-crls). |
| `RWS_CONFIG_TLS_CLIENT_CRL_STRICT` | `false` | boolean | Refuse a CRL past its `nextUpdate` and certificates no CRL covers. |
| `RWS_CONFIG_COMPRESSION_DICTIONARIES` | (empty) | string | Comma-separated URL patterns (`/js/app.*.js`) of static files offered as compression dictionaries; later versions go out as `dcz` / `dcb` deltas (since 18.12.0). See [Dictionary compression](/features/compression/#dictionary-compression). |
| `RWS_CONFIG_EARLY_HINTS_HTTP1` | `false` | boolean | Also send `103 Early Hints` to HTTP/1.1 clients (since 18.12.0). HTTP/2 and HTTP/3 always get them; see [Early Hints](/features/early-hints/). |
| `RWS_CONFIG_DIRECTORY_LISTING` | `true` | boolean | List a directory that has no `index.html` (since 18.8.4). `false` answers `404 Not Found` instead; directories with an `index.html` and files are unaffected. |
| `RWS_CONFIG_CLIENT_HINTS` | `false` | boolean | Send `Accept-CH`, `Critical-CH` and the client hints in `Vary` on every response (always on before 18.0.0). Off by default: they add ~700 bytes to every response, and `Critical-CH` makes Chromium retry a request that arrived without the hints. |
| `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` | `0` | integer | Maximum accepted request body size in bytes. `0` means unlimited. When set, a request whose `Content-Length` exceeds this is rejected with `413 Payload Too Large` before any body bytes are read, and the connection is closed. Applies to HTTP/1.1, HTTP/2, and HTTP/3. |
| `RWS_CONFIG_CSP` | unset | string | `Content-Security-Policy` header value, sent on every response. Unset: `default-src 'self'` with `RWS_CONFIG_SECURITY_HEADERS=true`, otherwise no header (18.0.0; was always `default-src 'self'`). An empty value suppresses the header. |
| `RWS_CONFIG_SECURITY_HEADERS` | `false` | boolean | Send `X-Frame-Options: SAMEORIGIN`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: geolocation=(), microphone=(), camera=()` and the default CSP on every response (always sent before 18.0.0). `X-Content-Type-Options: nosniff` is always sent. |
| `RWS_CONFIG_NANOS_HEADERS` | `false` | boolean | Send `Date-Unix-Epoch-Nanos` on every response and `Last-Modified-Unix-Epoch-Nanos` on static files (always sent before 18.0.0). |
| `RWS_CONFIG_SPA_FALLBACK` | `` (empty) | path | File (e.g. `index.html`) served for a `GET`/`HEAD` that matches no file or directory, instead of `404` — for client-side routers. Paths whose last segment contains a `.` still get `404`. Empty disables it. |
| `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES` | `` (empty) | string | Comma-separated path prefixes (e.g. `/api,/healthz`) that never get the SPA fallback. |
| `RWS_CONFIG_SPA_FALLBACK_ROUTES` | `` (empty) | string | SPA fallbacks per path prefix, comma-separated `prefix=file` pairs (e.g. `/cloud=cloud/index.html,/admin=admin/index.html`). The longest matching prefix wins; other paths use `RWS_CONFIG_SPA_FALLBACK`. See [Static files](/features/static-files/#several-apps-on-one-server). |
| `RWS_CONFIG_FILE` | `rws.config.toml` | path | Config file to load (settings and proxy routes) instead of `./rws.config.toml`. |

:::note[Thread count and the async build]
`RWS_CONFIG_THREAD_COUNT` only applies to the `http1`-only build, which uses a
hand-rolled `ThreadPool`. The `http2` / `http3` builds run on tokio and manage
concurrency differently — this variable is read but ignored at runtime.
:::

## TLS

| Variable | Default | Type | Description |
|----------|---------|------|-------------|
| `RWS_CONFIG_TLS_CERT_FILE` | `` (empty) | path | Path to the PEM-encoded certificate chain file. When non-empty, enables HTTPS, HTTP/2, and HTTP/3. |
| `RWS_CONFIG_TLS` | (empty) | string | `internal`: serve HTTPS with a certificate from rws's own CA (since 18.21.0; local development — see [HTTPS / TLS](/features/https-tls/#local-https-tls--internal)). Ignored when certificate files or ACME domains are set. |
| `RWS_CONFIG_TLS_INTERNAL_NAMES` | (empty) | list | More names (comma-separated) for the `tls = "internal"` certificate, beside `localhost`, `127.0.0.1`, `::1`, the listen address and the virtual hosts. |
| `RWS_CONFIG_TLS_INTERNAL_DIR` | per-user data dir | path | Where rws keeps its local CA and the certificate it issues (`~/.local/share/rws/pki` on Linux). |
| `RWS_CONFIG_TLS_KEY_FILE` | `` (empty) | path | Path to the PEM-encoded private key file. Must match `RWS_CONFIG_TLS_CERT_FILE`. |
| `RWS_CONFIG_TLS_CLIENT_CA_FILE` | `` (empty) | path | Path to a PEM-encoded CA certificate used to verify client certificates (mTLS). When set, the TLS handshake requires a valid client certificate signed by this CA. Connections without a valid cert are rejected before any HTTP processing. |
| `RWS_CONFIG_HTTP_REDIRECT_PORT` | `` (empty) | integer | When non-empty, a plain-HTTP listener is started on this port that issues `301 Moved Permanently` redirects to HTTPS. Set to `80` when running on standard ports. Requires TLS to be configured. |

### Virtual hosts

Per-domain TLS is configured through indexed variables. The server reads entries
sequentially, stopping at the first missing index.

| Variable | Description |
|----------|-------------|
| `RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN` | Domain name for virtual host 0 (e.g. `example.com`) |
| `RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE` | Certificate file for virtual host 0 |
| `RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE` | Private key file for virtual host 0 |
| `RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN` | Domain name for virtual host 1 |
| `RWS_CONFIG_VIRTUAL_HOST_1_CERT_FILE` | Certificate file for virtual host 1 |
| `RWS_CONFIG_VIRTUAL_HOST_1_KEY_FILE` | Private key file for virtual host 1 |

Continue the pattern (`_2_`, `_3_`, …) for as many domains as needed. The
`SniCertResolver` selects the correct certificate at TLS handshake time.

### ACME (Automatic Certificate Management)

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_ACME_DOMAINS` | `` (empty) | Comma-separated list of domain names. Setting this activates ACME at startup. Example: `example.com,www.example.com`. |
| `RWS_CONFIG_ACME_EMAIL` | `` (empty) | Contact email sent to the CA. Recommended but not required by all CAs. |
| `RWS_CONFIG_ACME_STAGING` | `false` | Set to `true` to use the Let's Encrypt staging environment for testing. |
| `RWS_CONFIG_ACME_DIRECTORY` | `` (empty) | Custom ACME directory URL. Defaults to Let's Encrypt production when empty. |
| `RWS_CONFIG_ACME_CERT_PATH` | `` (empty) | Where to write the provisioned certificate chain (PEM). Defaults to `RWS_CONFIG_TLS_CERT_FILE`, else `.rws/acme/cert.pem` (since 17.113.1; was `cert.pem` in the web root). When TLS paths are unset, ACME also points the TLS listener at this file. |
| `RWS_CONFIG_ACME_KEY_PATH` | `` (empty) | Where to write the certificate's private key (PEM). Defaults to `RWS_CONFIG_TLS_KEY_FILE`, else `.rws/acme/key.pem`; written `0600`. |
| `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port the CA connects to for HTTP-01. A temporary listener answers at startup; once the redirect listener (`RWS_CONFIG_HTTP_REDIRECT_PORT`) runs on this port it answers renewal challenges. Must be reachable from the internet on port 80. |
| `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `` (empty) | Renew this many days before expiry. Empty (the default since 18.0.0; it was `30`): at two-thirds of the certificate's lifetime, or earlier when the CA's renewal information (RFC 9773) says so. |
| `RWS_CONFIG_ACME_PROFILE` | `` (empty) | Certificate profile to order, e.g. `shortlived` (6-day certificates) or `tlsserver` at Let's Encrypt. Empty: the CA's default. A profile the CA doesn't offer stops provisioning with the list it offers. |
| `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `.rws/acme/account.key` | Path to persist the ACME account key between restarts (written `0600`). |
| `RWS_CONFIG_ACME_CA_FILE` | `` (empty) | PEM file of extra root CAs to trust for the ACME directory, on top of the public roots — for private ACME CAs (step-ca, Pebble). |
| `RWS_CONFIG_ACME_CHALLENGE` | `http-01` | How the CA checks you control the domains: `http-01` (port 80), `tls-alpn-01` (on the HTTPS port, RFC 8737), `dns-01` (a TXT record; needed for wildcards) or `dns-persist-01` (a TXT record you publish once). Since 18.4.0. See [Challenge types](/features/acme/#challenge-types). |
| `RWS_CONFIG_ACME_DNS_PROVIDER` | `` (empty) | DNS-01: `rfc2136` (a DNS UPDATE), `cloudflare`, `route53` (credentials from `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`) or `exec` (a program). |
| `RWS_CONFIG_ACME_DNS_API_TOKEN` | `` (empty) | DNS-01 `cloudflare`: the API token (DNS edit). No command-line flag; never printed. |
| `RWS_CONFIG_ACME_DNS_ZONE_ID` | `` (empty) | DNS-01 `cloudflare` / `route53`: the zone's id instead of looking it up. |
| `RWS_CONFIG_ACME_DNS_API_URL` | `` (empty) | DNS-01 `cloudflare` / `route53`: another API base URL (a proxy, a test double). |
| `RWS_CONFIG_ACME_DNS_SERVER` | `` (empty) | DNS-01: `rfc2136`'s primary server, `host[:port]` (port 53); with `exec`, optional: where to check that the record is visible (else the system resolver). |
| `RWS_CONFIG_ACME_DNS_ZONE` | `` (empty) | DNS-01: the zone holding `_acme-challenge` records; found by an SOA lookup when empty. |
| `RWS_CONFIG_ACME_DNS_TSIG_KEY` | `` (empty) | DNS-01 `rfc2136`: the TSIG key name. |
| `RWS_CONFIG_ACME_DNS_TSIG_SECRET` | `` (empty) | DNS-01 `rfc2136`: the TSIG secret, base64. Set it here or in `rws.config.toml` — there is no command-line flag — and it is never printed. |
| `RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM` | `hmac-sha256` | DNS-01 `rfc2136`: `hmac-sha256` or `hmac-sha512`. |
| `RWS_CONFIG_ACME_DNS_EXEC` | `` (empty) | DNS-01 `exec`: run as `<program> present <fqdn> <value>` and `<program> cleanup <fqdn> <value>`. |
| `RWS_CONFIG_ACME_DNS_PROPAGATION_SECS` | `120` | DNS-01: how long to wait for the record to be visible before the CA checks it. |
| `RWS_CONFIG_ACME_ON_DEMAND` | `false` | `true`: order a certificate at the first TLS handshake for a name `RWS_CONFIG_ACME_ON_DEMAND_ASK` allows. Since 18.4.0. See [On-demand TLS](/features/acme/#on-demand-tls). |
| `RWS_CONFIG_ACME_ON_DEMAND_ASK` | `` (empty) | URL asked `?domain=<name>` before each on-demand order; a 2xx answer allows it. Required with `RWS_CONFIG_ACME_ON_DEMAND`. |
| `RWS_CONFIG_ACME_REDIS` | `` (empty) | Redis `host:port` shared by replicas: one orders each certificate, all install it, and any answers the CA's challenges. Since 18.4.0. See [Several replicas](/features/acme/#several-replicas). |
| `RWS_CONFIG_ACME_REDIS_PASSWORD` | `` (empty) | Password for `RWS_CONFIG_ACME_REDIS`. No command-line flag; never printed. |
| `RWS_CONFIG_ACME_REDIS_PREFIX` | `rws:acme` | Key prefix in that Redis, for several sites in one. |
| `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` | `43200` | Longest time between renewal checks (sooner when renewal is due earlier or the CA's renewal information asks). After a failed renewal the next attempt comes within an hour. |

## CORS

Off by default (since 18.0.0). When `RWS_CONFIG_CORS_ALLOW_ALL` is `true` the server sets permissive CORS
headers on every response — any origin, with credentials — and the more specific `CORS_ALLOW_*` variables are
ignored.

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_CORS_ALLOW_ALL` | `false` | When `true`, allows all CORS requests from any origin, with credentials. Was `true` before 18.0.0. |
| `RWS_CONFIG_CORS_ALLOW_ORIGINS` | `` (empty) | Comma-separated list of allowed origins. Example: `https://app.example.com,https://admin.example.com`. Ignored when `CORS_ALLOW_ALL` is `true`. |
| `RWS_CONFIG_CORS_ALLOW_METHODS` | `` (empty) | Comma-separated list of allowed HTTP methods. Example: `GET,POST,PUT`. |
| `RWS_CONFIG_CORS_ALLOW_HEADERS` | `` (empty) | Comma-separated list of allowed request headers (lowercase). Example: `content-type,x-api-key`. |
| `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` | `` (empty) | Set to `true` to allow cookies and credentials to be sent with cross-origin requests. |
| `RWS_CONFIG_CORS_EXPOSE_HEADERS` | `` (empty) | Comma-separated list of response headers the browser may expose to JavaScript (lowercase). |
| `RWS_CONFIG_CORS_MAX_AGE` | `86400` | How long (in seconds) the browser may cache a preflight response. Default is 24 hours. |

All CORS variables are hot-reloadable. Send `SIGHUP` or `POST /admin/config/reload`
to apply changes without restarting.

## Rate limiting

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `` (unset) | Maximum number of requests allowed per client IP within the window. The `rws` binary enforces the global limit only when this is set above `0` (decided at startup). `rate_limit::global()` in library code defaults to `1000`. |
| `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `60` | Sliding window length in seconds. |

Rate-limit values are hot-reloadable. Existing per-IP counters are preserved when
limits change — only the allowed maximum and window size are updated.

## Circuit breaker

Read by `RedisCircuitBreaker::from_env()` (with `RWS_REDIS_HOST`/`_PORT`/`_PASSWORD`;
see [Circuit breaker](/proxy/circuit-breaker/)). Config-driven proxy routes
don't use them.

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD` | `5` | Consecutive failures before the circuit opens. |
| `RWS_CONFIG_CIRCUIT_BREAKER_RECOVERY_SECS` | `30` | Seconds the circuit stays open before a half-open probe. |
| `RWS_CONFIG_CIRCUIT_BREAKER_MAX_HALF_OPEN_PROBES` | `1` | Probe requests allowed while half-open. |

## Admin API

| Variable | Default | Options | Description |
|----------|---------|---------|-------------|
| `RWS_CONFIG_ADMIN_LISTEN` | (off) | `host:port` | Listen address of the [admin API](/deployment/admin/) (since 18.10.0). Read at startup. |
| `RWS_CONFIG_ADMIN_TOKEN_ENV` | `RWS_ADMIN_TOKEN` | variable name | The environment variable holding the admin bearer token. Unset or empty: the admin listener doesn't start. |
| `RWS_CONFIG_ADMIN_ALLOWED_ORIGINS` | (none) | comma list of origins | Browser origins allowed to call the admin API besides its own. |
| `RWS_CONFIG_ADMIN_USERS` | (none) | comma list | More admin users, `name:ENV_VAR:role` each (role `viewer`, `operator` or `admin`; since 18.22.0). See [users and roles](/deployment/admin/#users-roles-and-the-audit-log). |
| `RWS_CONFIG_FLEET_SOURCE` | (off) | URL | Where to pull the config from: `https://…`, `s3://bucket/key`, `git+<url>#<branch>:<path>`; `{node}` is the node name (since 18.23.0). See [Fleet config](/deployment/fleet/). |
| `RWS_CONFIG_FLEET_PUBLIC_KEY` | (none) | base64 or path | The Ed25519 public key configs are signed with (`rws fleet keygen`). |
| `RWS_CONFIG_FLEET_INTERVAL_SECS` | `30` | seconds | How often to check the source (at least 5). |
| `RWS_CONFIG_FLEET_REPORT_URL` | (none) | URL | Each outcome is POSTed here as JSON. |
| `RWS_CONFIG_FLEET_INSECURE` | `false` | bool | Accept unsigned configs (testing only). |
| `RWS_CONFIG_FLEET_S3_ENDPOINT` | AWS | URL | S3-compatible storage for `s3://` sources. |
| `RWS_CONFIG_CLUSTER_REDIS` | `RWS_CONFIG_ACME_REDIS` | `host:port` | Redis shared by replicas for runtime state: blocklist, maintenance, drains, flags, purges (since 18.22.0). See [Several replicas](/deployment/cluster/). |
| `RWS_CONFIG_CLUSTER_REDIS_PASSWORD` | `RWS_CONFIG_ACME_REDIS_PASSWORD` | string | Its password (never printed). |
| `RWS_CONFIG_CLUSTER_PREFIX` | `rws:cluster:` | string | Key prefix in that Redis. |
| `RWS_CONFIG_CLUSTER_NODE` | host name | string | This replica's name in `status`, heartbeats and audit entries (`[cluster] node`; since 18.22.0). |
| `RWS_CONFIG_ADMIN_AUDIT_LOG` | (memory only) | path | Append every admin change to this file as JSON lines. |
| `RWS_CONFIG_ADMIN_OIDC_ISSUER` | (off) | URL | OIDC provider for signing in to the admin UI (discovery at `<issuer>/.well-known/openid-configuration`). |
| `RWS_CONFIG_ADMIN_OIDC_PROVIDER` | (off) | `google`, `microsoft`, `github` | A provider preset instead of an issuer. |
| `RWS_CONFIG_ADMIN_OIDC_TENANT` | `common` | string | Azure AD tenant for `microsoft`. |
| `RWS_CONFIG_ADMIN_OIDC_CLIENT_ID` | (none) | string | The OIDC client id. |
| `RWS_CONFIG_ADMIN_OIDC_CLIENT_SECRET_ENV` | (none) | variable name | The variable holding the client secret. |
| `RWS_CONFIG_ADMIN_OIDC_COOKIE_SECRET_ENV` | (random per start) | variable name | The variable holding the session-cookie key (≥ 32 characters). |
| `RWS_CONFIG_ADMIN_OIDC_ROLES` | (none) | comma list | `group=role` (a group, an email or `*`) for signed-in users. |
| `RWS_CONFIG_ADMIN_OIDC_ROLE_CLAIM` | `groups` | claim name | The ID-token claim holding the user's groups. |

## Logging

| Variable | Default | Options | Description |
|----------|---------|---------|-------------|
| `RWS_CONFIG_PROXY_PROTOCOL` | `false` | boolean | Every connection to the main listener starts with a PROXY protocol v1 or v2 header (HAProxy `send-proxy`, AWS NLB, GCP); its source address becomes the client address. Connections without one are closed. Since 18.3.0; read at startup. See [Behind a load balancer](/proxy/overview/#behind-an-l4-load-balancer-proxy-protocol). |
| `RWS_CONFIG_WATCH` | `false` | boolean | Reload when `rws.config.toml` (or `RWS_CONFIG_FILE`) or a configured TLS certificate or key changes — the same validated reload as `SIGHUP`, checked once a second (since 18.1.0). Read at startup. See [Hot reload](/features/hot-reload/#reloading-when-the-file-changes). |
| `RWS_CONFIG_ACCESS_LOG` | (stdout) | path | Append access-log lines to this file instead of stdout (since 18.1.0). Opened at the first line; `SIGUSR1` reopens it (for logrotate). Read at startup. See [Logs](/deployment/observability/#writing-to-a-file). |
| `RWS_CONFIG_ACCESS_LOG_MAX_SIZE` | `100M` | bytes (`K`/`M`/`G` suffixes) | Rotate the access-log file when the next lines would take it past this size: `access.log` → `access.log.1` …. `0` never rotates (leave it to logrotate and `SIGUSR1`). The default was `0` before 18.15.0. |
| `RWS_CONFIG_ACCESS_LOG_KEEP` | `5` | integer | Rotated access-log files to keep; `0` empties the file instead of keeping any. |
| `RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP` | `off` | `off`, `truncate`, `hash` | Client addresses in access-log lines, OTLP log records and `recent_requests`: `truncate` keeps an IPv4 /24 or IPv6 /48, `hash` writes a keyed pseudonym (since 18.33.0). An unknown value truncates. Hot-reloadable. See [Anonymised addresses](/deployment/observability/#anonymised-addresses-and-redaction). |
| `RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET` | (none) | string | The HMAC-SHA256 key for `access_log_anonymize_ip = "hash"`; without it `hash` truncates. Hidden in startup output and `/admin/api/v1/config`. |
| `RWS_CONFIG_LOG_REDACT_QUERY` | (none) | comma list | Query parameter names whose values are logged as `REDACTED` in paths and `Referer`s, e.g. `token,api_key` (since 18.33.0). Hot-reloadable. |
| `RWS_CONFIG_LOG_REDACT_HEADERS` | (none) | comma list | Header names whose values are logged as `REDACTED`, besides `Authorization`, `Proxy-Authorization`, `Cookie` and `Set-Cookie`, which never are logged (since 18.33.0). Hot-reloadable. |
| `RWS_CONFIG_ACCESS_LOG_SYSLOG` | (none) | `udp://host:port`, `tcp://host:port`, `unix:///path` | Also send access-log lines to this syslog server as RFC 5424 messages (TCP: octet counting), from a background thread (since 18.33.0). Port 514 when left out. Read at startup. See [Syslog](/deployment/observability/#syslog). |
| `RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY` | `local0` | facility name or 0–23 | The syslog facility of those messages. |
| `RWS_CONFIG_MIN_FREE_DISK` | larger of 1% and 1G | size or `N%` | Below this much free space on its filesystem rws stops writing the access-log file and disk-cache entries (dropped and counted) until space returns; `0` turns it off (since 18.15.0). See [Disk safety](/deployment/observability/#disk-safety). |
| `RWS_CONFIG_TRAFFIC_ENABLED` | on with the admin API | boolean | Count visitors (people, AI, bots) for the admin UI's Traffic page; `false` turns it off (since 18.15.0). |
| `RWS_CONFIG_TRAFFIC_RETENTION_DAYS` | `30` | 1–90 | Days of traffic counts kept. |
| `RWS_CONFIG_TRAFFIC_PERSIST` | `traffic.json` beside the certificate | path or `off` | Where the counts are saved (every 5 minutes and at shutdown) so restarts keep them; unset with no certificate configured: not saved. |
| `RWS_CONFIG_TRAFFIC_EXCLUDE` | `/healthz,/readyz,/metrics` | paths | Exact paths not counted. |
| `RWS_CONFIG_TRAFFIC_SCRIPT` | `false` | boolean | Serve the first-party traffic script at `/_rws/t.js` and take its reports at `/_rws/t` (since 18.31.0). See [The first-party script](/deployment/traffic/#the-first-party-script-optional). |
| `RWS_CONFIG_TRAFFIC_GOALS` | (none) | comma list | Conversions to count: page paths (`/thanks/`, prefix `/docs/*`) or script events (`event:signup`); at most 32 (since 18.31.0). |
| `RWS_CONFIG_TRAFFIC_DATABASE_URL_ENV` | (none) | variable name | The environment variable holding a `postgres://` URL to keep every page view in (since 18.31.0; needs the `model-postgres` feature, in the release binaries but the FIPS one). See [Keeping every page view](/deployment/traffic/#keeping-every-page-view-in-postgresql-optional). |
| `RWS_CONFIG_TRAFFIC_GEO_DB` | the built-in table | path | A country table in `ci/gen_geo.py`'s format to use instead of the built-in one (since 18.31.0). Unreadable: the built-in one, with a warning. |
| `RWS_CONFIG_LOG_FORMAT` | `json` | `json`, `combined`, `off` | Log format for access logs. `json` writes structured JSON. `combined` writes Combined Log Format (Apache/nginx-compatible). `off` (or `none`) writes no per-request line — e.g. behind a proxy that already logs. Hot-reloadable. |

The log format is hot-reloadable.

## Kubernetes Gateway API

The `gateway-api` build feature's controller (in progress, 18.8.0). Read at startup. See [Kubernetes Gateway API](/deployment/kubernetes-gateway-api/).

| Variable | Default | Type | Description |
|----------|---------|------|-------------|
| `RWS_CONFIG_GATEWAY_API_ENABLED` | `false` | boolean | Watch the cluster's Gateway API objects, serve the Gateways of this controller's GatewayClasses and report their status |
| `RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME` | `rws8.tech/gateway-controller` | string | The `spec.controllerName` of the GatewayClasses this instance handles |
| `RWS_CONFIG_GATEWAY_API_POD_SELECTOR` | `app.kubernetes.io/name=rws` | `key=value,…` | Labels of the rws pods (in the controller's namespace) that serve Gateways in the shared mode; rws lists them in every Gateway Service's EndpointSlice |
| `RWS_CONFIG_GATEWAY_API_PORT_RANGE` | `10000-10999` | `first-last` | Container ports Gateway listeners are served on (each Service maps a listener port to one) |
| `RWS_CONFIG_GATEWAY_API_SERVICE_TYPE` | `LoadBalancer` | string | The type of the Service each Gateway gets |
| `RWS_CONFIG_GATEWAY_API_LEASE_NAMESPACE` | the pod's namespace | string | Where the `rws-gateway-controller` leader-election Lease lives |
| `RWS_CONFIG_GATEWAY_API_MODE` | `shared` | `shared` or `provisioned` | `shared`: the controller's pods serve every Gateway. `provisioned`: each Gateway gets its own Deployment of rws pods; the controller's pods serve none |
| `RWS_CONFIG_GATEWAY_API_IMAGE` | `ghcr.io/bohdaq/rws:<this version>` | string | The image provisioned Deployments run |
| `RWS_CONFIG_GATEWAY_API_SERVICE_ACCOUNT` | `POD_SERVICE_ACCOUNT`, else `default` | string | The ServiceAccount provisioned pods run as |
| `RWS_CONFIG_GATEWAY_API_GATEWAY` | — | `namespace/name` | Set by the controller on provisioned pods: serve only this Gateway, and never write status or objects |

## Templates

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_TEMPLATE_DIR` | `templates` | Directory containing Tera HTML templates. Only relevant when the `tera` Cargo feature is enabled. |

## Environment kind

`RWS_ENV` says what kind of environment the process runs in. Like the
`RWS_DB_*` variables it is outside the `RWS_CONFIG_*` namespace, and it has no
config-file key or flag: set it where the process starts.

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_ENV` | `prod` | `dev` (or `development`), `test`, or `prod` (or `production`); anything else counts as `prod`. With `dev`, [`app.dev_mcp()`](/mcp/dev-tools/) mounts its MCP endpoint without a token; otherwise only when `MCP_TOKEN` is set (since 18.28.0). Read in code with `entry_point::RwsEnv::current()`. |

## Database

Database variables are only used when the `model-sqlite`, `model-postgres`, or
`model-mysql` Cargo feature is enabled. They are not part of the `RWS_CONFIG_*`
namespace — they use a shorter `RWS_DB_*` prefix.

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_DB_BACKEND` | inferred | `sqlite`, `postgres`, or `mysql`. Inferred when exactly one `model-*` feature is compiled in; required otherwise. |
| `RWS_DB_HOST` | `localhost` | Database server hostname or IP address. |
| `RWS_DB_PORT` | `5432` | Database server port. The default matches PostgreSQL; MySQL uses `3306`. |
| `RWS_DB_USER` | `` (empty) | Database user name. |
| `RWS_DB_PASSWORD` | `` (empty) | Database password. |
| `RWS_DB_NAME` | `` (required) | Database name. No default — the server will fail to start if this is unset when the model feature is active. |
| `RWS_DB_POOL_SIZE` | `10` | Number of connections to pre-create in the connection pool at startup. |
| `RWS_DB_REPLICA_URLS` | — | Read replicas for `DbPool::from_env`: comma-separated database URLs (`postgres://…`, `mysql://…`, `sqlite:…`) of the primary's backend. Reads go to them in turn, writes and transactions to the primary. See [Read replicas](/database/overview/#read-replicas). |
| `RWS_DB_SSL_MODE` | `prefer` | TLS to PostgreSQL / MySQL: `disable`, `prefer`, `require`, `verify-ca`, `verify-full`. See [Database TLS](/database/overview/#tls). |
| `RWS_DB_SSL_ROOT_CERT` | — | PEM CA certificates the server's certificate is checked against (`verify-ca`, `verify-full`). |
| `RWS_DB_SSL_CLIENT_CERT` | — | PEM client certificate; with `RWS_DB_SSL_CLIENT_KEY`. |
| `RWS_DB_SSL_CLIENT_KEY` | — | Private key of `RWS_DB_SSL_CLIENT_CERT`. |

:::caution[SQLite path]
For `model-sqlite`, `RWS_DB_NAME` is the file path of the SQLite database, for
example `RWS_DB_NAME=./data/app.db`. Use `:memory:` for an in-memory database
(useful in tests).
:::

## Example: production environment

```bash
# Server
export RWS_CONFIG_IP=0.0.0.0
export RWS_CONFIG_PORT=443
export RWS_CONFIG_THREAD_COUNT=500

# TLS
export RWS_CONFIG_TLS_CERT_FILE=/etc/ssl/certs/server.pem
export RWS_CONFIG_TLS_KEY_FILE=/etc/ssl/private/server.key
export RWS_CONFIG_HTTP_REDIRECT_PORT=80

# CORS — restrict to known origins
export RWS_CONFIG_CORS_ALLOW_ALL=false
export RWS_CONFIG_CORS_ALLOW_ORIGINS=https://app.example.com
export RWS_CONFIG_CORS_ALLOW_METHODS=GET,POST,PUT,DELETE
export RWS_CONFIG_CORS_ALLOW_HEADERS=content-type,authorization

# Rate limiting
export RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS=500
export RWS_CONFIG_RATE_LIMIT_WINDOW_SECS=60

# Logging
export RWS_CONFIG_LOG_FORMAT=json
```
