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
| `RWS_CONFIG_IP` | `ip` | `--ip` / `-i` | `127.0.0.1` | Bind IP address |
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

## Memory

`rws` allocates one read buffer per request (default 16 KB). Files larger than 8 MB are streamed with chunked transfer encoding and are not buffered into memory. For files below that threshold, use HTTP Range Requests on the client side to fetch them in parts, or increase `request_allocation_size` with caution.
