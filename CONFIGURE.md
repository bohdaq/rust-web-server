[Read Me](README.md) > Configuration

# Configuration

`rws` can be started with no configuration and will bind to `0.0.0.0:7878` with 200 threads, CORS enabled for all origins.

Configuration is applied in order from lowest to highest priority:

1. Built-in defaults (`src/entry_point/mod.rs`)
2. System environment variables (`rws.variables`)
3. `rws.config.toml` in the working directory
4. Command-line arguments (`rws.command_line`)

## HTTPS, HTTP/2, and HTTP/3

TLS is built into the default `rws` binary. Providing a certificate and key enables HTTPS on the configured port. HTTP/2 is negotiated automatically via ALPN alongside HTTP/1.1 on the same TCP port. HTTP/3 listens on the same port number over UDP (QUIC) simultaneously.

To obtain a free certificate for a public domain use [Let's Encrypt](https://letsencrypt.org/).

For local development, generate a self-signed certificate:
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

### TLS configuration

| Environment variable | Config file key | Command-line arg | Description |
|---|---|---|---|
| `RWS_CONFIG_TLS_CERT_FILE` | `tls_cert_file` | `--tls-cert-file` / `-s` | Path to PEM certificate file |
| `RWS_CONFIG_TLS_KEY_FILE` | `tls_key_file` | `--tls-key-file` / `-k` | Path to PEM private key file |

Example — environment variables:
```bash
export RWS_CONFIG_TLS_CERT_FILE="/path/to/cert.pem"
export RWS_CONFIG_TLS_KEY_FILE="/path/to/key.pem"
```

Example — `rws.config.toml`:
```toml
tls_cert_file = '/path/to/cert.pem'
tls_key_file  = '/path/to/key.pem'
```

Example — command line:
```bash
rws --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```

## Virtual hosting / SNI routing

A single `rws` instance can serve multiple domains, each with its own TLS certificate. The server reads the SNI hostname from the TLS `ClientHello`, selects the matching certificate, and exposes the hostname as `ConnectionInfo::sni_hostname` so application code can route per-domain.

Add one `[[virtual_host]]` block per domain in `rws.config.toml`. The default cert/key (`tls_cert_file` / `tls_key_file`) is used when no SNI hostname matches or when the client sends no SNI.

```toml
# Default cert — used when no virtual host matches, or for plain HTTP/1.1
tls_cert_file = '/etc/ssl/default.pem'
tls_key_file  = '/etc/ssl/default.key'

[[virtual_host]]
domain   = 'example.com'
cert_file = '/etc/ssl/example.pem'
key_file  = '/etc/ssl/example.key'

[[virtual_host]]
domain   = 'other.com'
cert_file = '/etc/ssl/other.pem'
key_file  = '/etc/ssl/other.key'
```

Virtual hosts can also be configured via numbered environment variables:

```bash
RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN=example.com
RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE=/etc/ssl/example.pem
RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE=/etc/ssl/example.key

RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN=other.com
RWS_CONFIG_VIRTUAL_HOST_1_CERT_FILE=/etc/ssl/other.pem
RWS_CONFIG_VIRTUAL_HOST_1_KEY_FILE=/etc/ssl/other.key
```

Send `SIGHUP` (or `POST /admin/config/reload`) to hot-reload all virtual host certificates without restarting.

### App-level virtual-host routing

The `Router` exposes `.with_host(hostname)` to restrict a router's routes to a specific virtual host. For plain-HTTP connections the `Host` header is used as fallback when SNI is not available.

```rust
use rust_web_server::router::Router;

let example_router = Router::new()
    .with_host("example.com")
    .get("/", example_home)
    .get("/about", example_about);

let other_router = Router::new()
    .with_host("other.com")
    .get("/", other_home);

// In Application::execute, call both; first non-None result wins.
if let Some(resp) = example_router.handle(&request, &connection) { return Ok(resp); }
if let Some(resp) = other_router.handle(&request, &connection)  { return Ok(resp); }
```

## HTTP → HTTPS redirect

When TLS is configured, you can redirect all plain-HTTP traffic to HTTPS by setting `RWS_CONFIG_HTTP_REDIRECT_PORT`. The server binds an additional plain-HTTP listener on that port and returns `301 Moved Permanently` to the HTTPS URL for every request.

Example — redirect port 80 to HTTPS on port 443:
```bash
export RWS_CONFIG_HTTP_REDIRECT_PORT=80
rws --ip=0.0.0.0 --port=443 --tls-cert-file=cert.pem --tls-key-file=key.pem
```

Example — `rws.config.toml`:
```toml
tls_cert_file          = '/path/to/cert.pem'
tls_key_file           = '/path/to/key.pem'
http_redirect_port     = '80'
port                   = '443'
ip                     = '0.0.0.0'
```

> The redirect listener is a no-op when `RWS_CONFIG_TLS_CERT_FILE` is not set, so the option is safe to include in configuration files shared between HTTP-only and HTTPS deployments.

## All configuration options

| Environment variable | Config file key | Command-line arg | Default | Description |
|---|---|---|---|---|
| `RWS_CONFIG_IP` | `ip` | `--ip` / `-i` | `0.0.0.0` | Bind IP address |
| `RWS_CONFIG_PORT` | `port` | `--port` / `-p` | `7878` | Bind port |
| `RWS_CONFIG_THREAD_COUNT` | `thread_count` | `--thread-count` / `-t` | `200` | Thread pool size |
| `RWS_CONFIG_TLS_CERT_FILE` | `tls_cert_file` | `--tls-cert-file` / `-s` | _(none)_ | PEM certificate file |
| `RWS_CONFIG_TLS_KEY_FILE` | `tls_key_file` | `--tls-key-file` / `-k` | _(none)_ | PEM private key file |
| `RWS_CONFIG_HTTP_REDIRECT_PORT` | `http_redirect_port` | `--http-redirect-port` | _(none)_ | Plain-HTTP port that redirects all requests to HTTPS (requires TLS) |
| `RWS_CONFIG_CORS_ALLOW_ALL` | `cors_allow_all` | `--cors-allow-all` / `-a` | `true` | Allow all CORS origins |
| `RWS_CONFIG_CORS_ALLOW_ORIGINS` | `cors_allow_origins` | `--cors-allow-origins` / `-o` | _(none)_ | Allowed origins (comma-separated) |
| `RWS_CONFIG_CORS_ALLOW_METHODS` | `cors_allow_methods` | `--cors-allow-methods` / `-m` | _(none)_ | Allowed methods |
| `RWS_CONFIG_CORS_ALLOW_HEADERS` | `cors_allow_headers` | `--cors-allow-headers` / `-h` | _(none)_ | Allowed headers |
| `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` | `cors_allow_credentials` | `--cors-allow-credentials` / `-c` | `false` | Allow credentials |
| `RWS_CONFIG_CORS_EXPOSE_HEADERS` | `cors_expose_headers` | `--cors-expose-headers` / `-e` | _(none)_ | Exposed headers |
| `RWS_CONFIG_CORS_MAX_AGE` | `cors_max_age` | `--cors-max-age` / `-g` | `86400` | Preflight cache duration (seconds) |
| `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` | `request_allocation_size_in_bytes` | `--request-allocation-size-in-bytes` / `-r` | `10000` | Read chunk size per request (bytes); larger bodies span several reads |
| `RWS_CONFIG_CSP` | `csp` | — | `default-src 'self'` | `Content-Security-Policy` header value; set to empty string to disable |
| `RWS_CONFIG_STATIC_CACHE_CONTROL` | `static_cache_control` | — | `no-cache` | `Cache-Control` for static files (revalidated with ETag / Last-Modified); empty = the `no-store` default |
| `RWS_CONFIG_CLIENT_HINTS` | `client_hints` | — | `false` | Send `Accept-CH`, `Critical-CH` and the hints in `Vary` on every response |
| `RWS_CONFIG_LOG_FORMAT` | `log_format` | — | `json` | Access log format: `json` (structured JSON for log aggregators), `combined` (CLF), or `off` / `none` (no access log) |
| `RWS_CONFIG_SPA_FALLBACK` | `spa_fallback` | — | _(none, disabled)_ | Serve this file (e.g. `index.html`) for a `GET`/`HEAD` request that matches no real file/directory instead of `404` — the client-side-router ("SPA") fallback pattern |
| `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES` | `spa_fallback_exclude_prefixes` | — | _(none)_ | Comma-separated path prefixes (e.g. `/api`) that never receive the SPA fallback |
| `RWS_CONFIG_TLS_CLIENT_CA_FILE` | `tls_client_ca_file` | — | _(none)_ | PEM CA file; when set, clients must present a certificate signed by it (mTLS) |
| `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` | `max_body_size_in_bytes` | — | `0` (unlimited) | Larger request bodies get `413 Payload Too Large` |
| `RWS_CONFIG_OPEN_FILE_CACHE_SECS` | `open_file_cache_secs` | — | `1` | How long open static-file handles are reused before revalidation; `0` = off (Unix only) |
| `RWS_CONFIG_TEMPLATE_DIR` | `template_dir` | — | `templates` | Tera template directory (`tera` feature; read by `template::init_from_env()`) |
| `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `[rate_limit] max_requests` | — | _(none, no limit)_ | Global per-client-IP request limit per window (`429` when exceeded) |
| `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `[rate_limit] window_secs` | — | `60` | Rate-limit window in seconds |
| `RWS_CONFIG_ACME_DOMAINS` | `[acme] domains` | `--acme-domains` | _(none)_ | Comma-separated domains to get a Let's Encrypt certificate for; enables automatic HTTPS (`acme` feature) |
| `RWS_CONFIG_ACME_EMAIL` | `[acme] email` | `--acme-email` | _(none)_ | ACME account contact email |
| `RWS_CONFIG_ACME_STAGING` | `[acme] staging` | `--acme-staging` | `false` | Use the Let's Encrypt staging environment |
| `RWS_CONFIG_ACME_DIRECTORY` | `[acme] directory` | `--acme-directory` | Let's Encrypt | ACME directory URL of another CA |
| `RWS_CONFIG_ACME_CA_FILE` | `[acme] ca_file` | `--acme-ca-file` | _(none)_ | Extra root CAs (PEM) to trust for the ACME directory |
| `RWS_CONFIG_ACME_CHALLENGE_PORT` | `[acme] challenge_port` | `--acme-challenge-port` | `80` | Port the CA connects to for HTTP-01 challenges |
| `RWS_CONFIG_ACME_CERT_PATH` | `[acme] cert_path` | `--acme-cert-path` | `tls_cert_file`, else `cert.pem` | Where the issued certificate is stored |
| `RWS_CONFIG_ACME_KEY_PATH` | `[acme] key_path` | `--acme-key-path` | `tls_key_file`, else `key.pem` | Where the certificate's private key is stored |
| `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `[acme] account_key_path` | — | `acme_account.key` | ACME account key file |
| `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `[acme] renew_before_days` | — | `30` | Renew when the certificate expires within this many days |
| `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` | `[acme] renew_check_interval_secs` | — | `43200` | How often to check whether renewal is due |
| `RWS_CONFIG_FILE` | — | — | `rws.config.toml` | Path of the config file (settings and proxy routes) |

## Memory

`rws` reads each request in chunks of `request_allocation_size_in_bytes` (default 10000 bytes); larger bodies span several reads. Static files larger than 256 KiB are streamed from disk and not buffered into memory; proxied upstream bodies over 1 MB are streamed too. For smaller files, use HTTP Range Requests on the client side to fetch them in parts.
