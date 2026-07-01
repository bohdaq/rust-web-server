[Read Me](README.md) > Configuration

# Configuration

`rws` can be started with no configuration and will bind to `127.0.0.1:7878` with 200 threads, CORS enabled for all origins.

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
| `RWS_CONFIG_HTTP_REDIRECT_PORT` | `http_redirect_port` | — | _(none)_ | Plain-HTTP port that redirects all requests to HTTPS (requires TLS) |
| `RWS_CONFIG_CORS_ALLOW_ALL` | `cors_allow_all` | `--cors-allow-all` / `-a` | `true` | Allow all CORS origins |
| `RWS_CONFIG_CORS_ALLOW_ORIGINS` | `cors_allow_origins` | `--cors-allow-origins` / `-o` | _(none)_ | Allowed origins (comma-separated) |
| `RWS_CONFIG_CORS_ALLOW_METHODS` | `cors_allow_methods` | `--cors-allow-methods` / `-m` | _(none)_ | Allowed methods |
| `RWS_CONFIG_CORS_ALLOW_HEADERS` | `cors_allow_headers` | `--cors-allow-headers` / `-h` | _(none)_ | Allowed headers |
| `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` | `cors_allow_credentials` | `--cors-allow-credentials` / `-c` | `false` | Allow credentials |
| `RWS_CONFIG_CORS_EXPOSE_HEADERS` | `cors_expose_headers` | `--cors-expose-headers` / `-e` | _(none)_ | Exposed headers |
| `RWS_CONFIG_CORS_MAX_AGE` | `cors_max_age` | `--cors-max-age` / `-g` | _(none)_ | Preflight cache duration (seconds) |
| `RWS_CONFIG_REQUEST_ALLOCATION_SIZE` | `request_allocation_size` | `--request-allocation-size-in-bytes` / `-r` | `16000` | Read buffer size per request |
| `RWS_CONFIG_CSP` | — | — | `default-src 'self'` | `Content-Security-Policy` header value; set to empty string to disable |
| `RWS_CONFIG_LOG_FORMAT` | `log_format` | — | `json` | Access log format: `json` (structured JSON for log aggregators) or `combined` (CLF) |

## Memory

`rws` allocates one read buffer per request (default 16 KB). Files larger than 8 MB are streamed with chunked transfer encoding and are not buffered into memory. For files below that threshold, use HTTP Range Requests on the client side to fetch them in parts, or increase `request_allocation_size` with caution.
