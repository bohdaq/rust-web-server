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
| `RWS_CONFIG_RUNTIME` | `shared` | string | Async builds, Unix only. `shared`: one multi-threaded tokio runtime (`TOKIO_WORKER_THREADS` workers) takes every connection. `per-core`: one single-threaded runtime per worker, each accepting on its own `SO_REUSEPORT` socket on the same port, like nginx's worker processes — a connection stays on one thread, with no work stealing or cross-thread wake-ups. On fast CPUs that's more throughput (on the benchmark's fastest runners +12–16% requests per second, and 4% less CPU per proxied request); on slower ones it costs about 12% more CPU per small request for a few percent more throughput. HTTP/3, the redirect listener and ACME stay on the main runtime; SIGHUP and shutdown work as usual. Read at startup. |
| `RWS_CONFIG_STATIC_CACHE_CONTROL` | `no-cache` | string | `Cache-Control` sent with static files (since 18.0.0; other responses keep `no-store, …`). The default lets browsers keep a file and revalidate it with `If-None-Match` / `If-Modified-Since`, which costs a `304` instead of the whole file. Set e.g. `public, max-age=3600` for assets with versioned names; an empty value sends the `no-store` default for static files too. |
| `RWS_CONFIG_CLIENT_HINTS` | `false` | boolean | Send `Accept-CH`, `Critical-CH` and the client hints in `Vary` on every response (always on before 18.0.0). Off by default: they add ~700 bytes to every response, and `Critical-CH` makes Chromium retry a request that arrived without the hints. |
| `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` | `0` | integer | Maximum accepted request body size in bytes. `0` means unlimited. When set, a request whose `Content-Length` exceeds this is rejected with `413 Payload Too Large` before any body bytes are read, and the connection is closed. Applies to HTTP/1.1, HTTP/2, and HTTP/3. |
| `RWS_CONFIG_CSP` | unset | string | `Content-Security-Policy` header value, sent on every response. Unset: `default-src 'self'` with `RWS_CONFIG_SECURITY_HEADERS=true`, otherwise no header (18.0.0; was always `default-src 'self'`). An empty value suppresses the header. |
| `RWS_CONFIG_SECURITY_HEADERS` | `false` | boolean | Send `X-Frame-Options: SAMEORIGIN`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: geolocation=(), microphone=(), camera=()` and the default CSP on every response (always sent before 18.0.0). `X-Content-Type-Options: nosniff` is always sent. |
| `RWS_CONFIG_NANOS_HEADERS` | `false` | boolean | Send `Date-Unix-Epoch-Nanos` on every response and `Last-Modified-Unix-Epoch-Nanos` on static files (always sent before 18.0.0). |
| `RWS_CONFIG_SPA_FALLBACK` | `` (empty) | path | File (e.g. `index.html`) served for a `GET`/`HEAD` that matches no file or directory, instead of `404` — for client-side routers. Paths whose last segment contains a `.` still get `404`. Empty disables it. |
| `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES` | `` (empty) | string | Comma-separated path prefixes (e.g. `/api,/healthz`) that never get the SPA fallback. |
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
| `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `30` | Renew the certificate when fewer than this many days remain on it. |
| `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `.rws/acme/account.key` | Path to persist the ACME account key between restarts (written `0600`). |
| `RWS_CONFIG_ACME_CA_FILE` | `` (empty) | PEM file of extra root CAs to trust for the ACME directory, on top of the public roots — for private ACME CAs (step-ca, Pebble). |
| `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` | `43200` | Seconds between certificate-expiry checks. After a failed renewal the next attempt comes within an hour. |

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

## Logging

| Variable | Default | Options | Description |
|----------|---------|---------|-------------|
| `RWS_CONFIG_LOG_FORMAT` | `json` | `json`, `combined`, `off` | Log format for access logs. `json` writes structured JSON. `combined` writes Combined Log Format (Apache/nginx-compatible). `off` (or `none`) writes no per-request line — e.g. behind a proxy that already logs. Hot-reloadable. |

The log format is hot-reloadable.

## Templates

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_TEMPLATE_DIR` | `templates` | Directory containing Tera HTML templates. Only relevant when the `tera` Cargo feature is enabled. |

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
