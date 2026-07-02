---
title: CLI Arguments
description: All command-line flags accepted by rust-web-server, with types, defaults, and examples.
---

Command-line arguments are the highest-priority configuration layer — they
override defaults, system environment variables, and `rws.config.toml`. Every
flag maps directly to a `RWS_CONFIG_*` environment variable; setting the flag is
exactly equivalent to setting that variable before the process starts.

Flags use the `--long-form=value` syntax. A single-character short form is also
supported with `-x=value`.

```bash
rws [--flag=value ...]
```

## Server

| Flag | Short | Type | Default | Maps to |
|------|-------|------|---------|---------|
| `--ip=<addr>` | `-i` | string | `0.0.0.0` | `RWS_CONFIG_IP` |
| `--port=<n>` | `-p` | integer | `7878` | `RWS_CONFIG_PORT` |
| `--thread-count=<n>` | `-t` | integer | `200` | `RWS_CONFIG_THREAD_COUNT` |
| `--request-allocation-size-in-bytes=<n>` | `-r` | integer | `10000` | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |

```bash
# Listen on all interfaces on port 443 with 500 worker threads
rws --ip=0.0.0.0 --port=443 --thread-count=500
```

## TLS

| Flag | Short | Type | Default | Maps to |
|------|-------|------|---------|---------|
| `--tls-cert-file=<path>` | `-s` | path | `` (empty) | `RWS_CONFIG_TLS_CERT_FILE` |
| `--tls-key-file=<path>` | `-k` | path | `` (empty) | `RWS_CONFIG_TLS_KEY_FILE` |

Setting both `--tls-cert-file` and `--tls-key-file` enables HTTPS, HTTP/2, and
HTTP/3. The server falls back to plain HTTP/1.1 when either flag is absent.

```bash
rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```

:::note[mTLS and HTTP redirect port]
The `--tls-client-ca-file` and `--http-redirect-port` settings do not have CLI
flags. Configure them via environment variables (`RWS_CONFIG_TLS_CLIENT_CA_FILE`,
`RWS_CONFIG_HTTP_REDIRECT_PORT`) or `rws.config.toml`.
:::

## CORS

| Flag | Short | Type | Default | Maps to |
|------|-------|------|---------|---------|
| `--cors-allow-all=<bool>` | `-a` | bool | `true` | `RWS_CONFIG_CORS_ALLOW_ALL` |
| `--cors-allow-origins=<list>` | `-o` | string | `` (empty) | `RWS_CONFIG_CORS_ALLOW_ORIGINS` |
| `--cors-allow-methods=<list>` | `-m` | string | `` (empty) | `RWS_CONFIG_CORS_ALLOW_METHODS` |
| `--cors-allow-headers=<list>` | `-h` | string | `` (empty) | `RWS_CONFIG_CORS_ALLOW_HEADERS` |
| `--cors-allow-credentials=<bool>` | `-c` | bool | `` (empty) | `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` |
| `--cors-expose-headers=<list>` | `-e` | string | `` (empty) | `RWS_CONFIG_CORS_EXPOSE_HEADERS` |
| `--cors-max-age=<seconds>` | `-g` | integer | `86400` | `RWS_CONFIG_CORS_MAX_AGE` |

`<list>` values are comma-separated strings. Header name lists should be
lowercase.

```bash
# Disable the allow-all default and restrict to specific origins
rws \
  --cors-allow-all=false \
  --cors-allow-origins=https://app.example.com,https://admin.example.com \
  --cors-allow-methods=GET,POST,PUT,DELETE \
  --cors-allow-headers=content-type,authorization \
  --cors-allow-credentials=true \
  --cors-expose-headers=x-request-id \
  --cors-max-age=3600
```

:::caution[allow-all takes precedence]
When `--cors-allow-all=true` (the default), the specific `--cors-allow-origins`,
`--cors-allow-methods`, and `--cors-allow-headers` flags are ignored. Set
`--cors-allow-all=false` to activate fine-grained CORS control.
:::

## Quick-start examples

### Development server (HTTP only)

```bash
cargo run -- --ip=127.0.0.1 --port=7878
```

### HTTPS with HTTP/2 and HTTP/3

```bash
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem
```

### Locked-down CORS for production

```bash
rws \
  --port=443 \
  --tls-cert-file=/etc/ssl/server.pem \
  --tls-key-file=/etc/ssl/server.key \
  --cors-allow-all=false \
  --cors-allow-origins=https://app.example.com \
  --cors-allow-methods=GET,POST \
  --cors-allow-headers=content-type,authorization \
  --request-allocation-size-in-bytes=65536
```

### Override a single value from the config file

CLI flags always win over `rws.config.toml`. Use this pattern to temporarily
change a setting without editing the file:

```bash
# rws.config.toml has port = 7878; override to 9090 for this run only
rws --port=9090
```

## Full flag reference

```
rws [OPTIONS]

OPTIONS:
  -i, --ip=<addr>
        IP address to bind (default: 0.0.0.0)

  -p, --port=<n>
        TCP port to listen on (default: 7878)

  -t, --thread-count=<n>
        Worker thread pool size (http1 build only; default: 200)

  -r, --request-allocation-size-in-bytes=<n>
        Read buffer size per request in bytes (default: 10000)

  -s, --tls-cert-file=<path>
        Path to PEM certificate chain; enables TLS when set

  -k, --tls-key-file=<path>
        Path to PEM private key; enables TLS when set

  -a, --cors-allow-all=<bool>
        Allow all CORS origins (default: true)

  -o, --cors-allow-origins=<list>
        Comma-separated allowed origins

  -m, --cors-allow-methods=<list>
        Comma-separated allowed HTTP methods

  -h, --cors-allow-headers=<list>
        Comma-separated allowed request headers (lowercase)

  -c, --cors-allow-credentials=<bool>
        Allow credentials in CORS requests

  -e, --cors-expose-headers=<list>
        Comma-separated response headers exposed to the browser (lowercase)

  -g, --cors-max-age=<seconds>
        Preflight cache duration in seconds (default: 86400)
```
