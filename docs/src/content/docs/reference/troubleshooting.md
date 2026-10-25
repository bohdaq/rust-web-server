---
title: Troubleshooting
description: Common errors when starting, reaching, building and deploying rws, and how to fix them.
---

## Starting the server

### `unable to set up TCP listener: Permission denied (os error 13)`

Ports below 1024 need privileges. Use a higher port (`--port=7878`), or run
with `sudo` / as Administrator. In a container, the release image runs as a
non-root user, so map a high container port to 80/443 instead.

### `unable to set up TCP listener: Address already in use`

Another process holds the port. Find it:

```bash
sudo lsof -i :7878     # macOS and Linux
sudo fuser 7878/tcp    # Linux
```

### `Cannot assign requested address (os error 99)`

The `--ip` you passed isn't assigned to any local interface. List them with
`ip addr` (Linux) or `ifconfig`, or bind to all of them with `--ip=0.0.0.0`.

### `sudo rws` says `command not found`

`~/.cargo/bin` (or `/usr/local/bin`) isn't on root's `PATH`. Use the full
path: `sudo ~/.cargo/bin/rws`.

## Reaching the server

### Other machines can't connect

- Bound to `127.0.0.1`? That only accepts local connections. The default is
  `0.0.0.0`; pass `--ip=0.0.0.0` if you overrode it.
- `Connection refused` from another host with the right IP: a firewall is
  blocking the port.

### Serving on IPv6

`rws --ip=::` listens on IPv6 (and IPv4-mapped addresses). Link-local
`fe80::…` addresses need a zone id that browsers rarely accept; connect with
the machine's IPv4 address or a global IPv6 address instead.

### HTTPS, HTTP/2 or HTTP/3 isn't working

No certificate is configured, so `rws` falls back to plain HTTP/1.1. Provide
one ([HTTPS / TLS](/features/https-tls/)) or let `rws` get one
([ACME](/features/acme/)):

```bash
rws --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```

### `TLS setup failed: failed to read cert file …`

The path is wrong, the process can't read it, or the file isn't PEM. Both the
certificate (chain) and the key must be PEM.

### The browser warns about the certificate

Self-signed certificates aren't trusted. For local development add yours to
the system trust store (or use `mkcert`); in production use a public CA,
for example through [ACME](/features/acme/).

### Malformed-request errors (`400`)

The server received something that isn't an HTTP request — a port scanner,
TLS sent to a plain-HTTP port, or garbage. The client gets a `400 Bad Request`;
it's safe to ignore.

### A directory shows a file listing

A directory without `index.html` is shown as a listing. Add an `index.html` to
serve that instead.

## Logs and metrics

### I want the Combined Log Format, not JSON

Set `RWS_CONFIG_LOG_FORMAT=combined` (or `log_format = "combined"` in
`rws.config.toml`); `off` disables the access log. JSON is the default. Lines
are buffered and written within 100 ms — see
[Observability](/deployment/observability/).

### Scraping metrics

`GET /metrics` serves Prometheus text format; see [Metrics](/features/metrics/)
for the metric names and a scrape config.

## Kubernetes

### Probes fail with `Connection refused`

The kubelet probes the pod IP, so `rws` must not be bound to `127.0.0.1`.
The default `0.0.0.0` works; check for an `ip = "127.0.0.1"` override.

### `/readyz` returns `503` right after start

Expected: it returns `503` until startup finishes, then `200`. If it stays at
`503`, startup failed — check the logs. Give slow starts an
`initialDelaySeconds`:

```yaml
readinessProbe:
  httpGet:
    path: /readyz
    port: 7878
  initialDelaySeconds: 5
  periodSeconds: 5
```

### Requests are dropped on `SIGTERM`

`rws` stops accepting on `SIGTERM` and finishes in-flight requests. Requests
can still arrive while the load balancer hasn't noticed yet; delay
termination with a `preStop` sleep (Kubernetes 1.30+ — the release image has
no shell for an `exec` hook):

```yaml
lifecycle:
  preStop:
    sleep:
      seconds: 5
```

More in [Kubernetes](/deployment/kubernetes/).

## Building from source

### `linker 'cc' not found`

Install a C toolchain (needed by `aws-lc-rs` and other native dependencies):

```bash
sudo apt-get install build-essential        # Debian/Ubuntu
sudo dnf group install "Development Tools"  # Fedora/RHEL
sudo pacman -S base-devel                   # Arch
```

### `found crate … compiled by an incompatible version of rustc`

Stale build artifacts from another toolchain: `cargo clean && cargo build`.

### `error: package … requires rustc 1.88`

The minimum supported Rust version is 1.88: `rustup update stable`.

### `failed open - '…/FETCH_HEAD' is locked: Permission denied`

The Cargo registry was written by `sudo cargo …` once. Fix the ownership
(`sudo chown -R $USER ~/.cargo`) rather than building as root.
