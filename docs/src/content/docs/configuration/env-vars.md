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
| `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` | `10000` | integer | Maximum number of bytes allocated per incoming request read buffer. Increase this if your clients send large request bodies or headers. |

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
| `RWS_CONFIG_ACME_CERT_PATH` | `` (empty) | Where to write the provisioned certificate chain (PEM). Defaults to `RWS_CONFIG_TLS_CERT_FILE`. |
| `RWS_CONFIG_ACME_KEY_PATH` | `` (empty) | Where to write the certificate's private key (PEM). Defaults to `RWS_CONFIG_TLS_KEY_FILE`. |
| `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port for the temporary HTTP-01 challenge server. Must be reachable from the internet on port 80. Not used with DNS-01. |
| `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `30` | Renew the certificate when fewer than this many days remain on it. |
| `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `acme_account.key` | Path to persist the ACME account key between restarts. |

## CORS

When `RWS_CONFIG_CORS_ALLOW_ALL` is `true` the server sets permissive CORS
headers on every response and the more specific `CORS_ALLOW_*` variables are
ignored.

| Variable | Default | Description |
|----------|---------|-------------|
| `RWS_CONFIG_CORS_ALLOW_ALL` | `true` | When `true`, allows all CORS requests from any origin. |
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
| `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `1000` | Maximum number of requests allowed per client IP within the window. Set to `0` to disable rate limiting. |
| `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `60` | Sliding window length in seconds. |

Rate-limit values are hot-reloadable. Existing per-IP counters are preserved when
limits change — only the allowed maximum and window size are updated.

## Logging

| Variable | Default | Options | Description |
|----------|---------|---------|-------------|
| `RWS_CONFIG_LOG_FORMAT` | `json` | `json`, `combined` | Log format for access logs. `json` writes structured JSON. `combined` writes Combined Log Format (Apache/nginx-compatible). |

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
