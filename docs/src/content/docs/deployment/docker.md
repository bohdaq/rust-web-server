---
title: Docker
description: Build and run rust-web-server in a minimal Docker container using multi-stage builds.
---

## Multi-stage Dockerfile

The recommended Dockerfile uses a two-stage build: a `rust:1.75` builder compiles the binary, then a slim Debian image carries only the final executable.

```dockerfile
# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1.75 AS builder
WORKDIR /app

# Cache dependencies before copying source.
COPY Cargo.toml Cargo.lock ./
# Build a dummy main to prime the dependency cache.
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm src/main.rs

# Now copy real source and build.
COPY . .
RUN cargo build --release

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# Runtime dependencies for TLS (ca-certificates) and diagnostics (curl).
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rws /usr/local/bin/rws

# TCP port (HTTP/1.1, HTTP/2)
EXPOSE 7878
# UDP port (HTTP/3 / QUIC) — only relevant with the http3 feature
EXPOSE 7878/udp

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s \
  CMD curl -f http://localhost:7878/healthz || exit 1

CMD ["rws"]
```

:::note[Dependency cache layer]
The dummy `src/main.rs` trick builds and caches all Cargo dependencies before the real source is copied. Subsequent builds that only change application code skip the full `cargo build` of your dependencies.
:::

## Image sizes by feature flag

The default `cargo build --release` enables the `http3` feature, which includes QUIC, HTTP/2, TLS (`rustls`), and the `quinn`/`h3` stacks. Lighter builds are available:

| Feature flag | TLS | HTTP/2 | HTTP/3 | Approx. final image |
|---|---|---|---|---|
| `http1` (no-default) | No | No | No | ~3 MB |
| `http2` (no-default) | Yes (rustls) | Yes | No | ~7 MB |
| `http3` (default) | Yes (rustls) | Yes | Yes | ~12 MB |

To build with the `http1`-only feature (smallest image):

```dockerfile
RUN cargo build --release --no-default-features --features http1
```

## Exposed ports

| Port | Protocol | Purpose |
|---|---|---|
| `7878` | TCP | HTTP/1.1 and HTTP/2 (TLS upgrade via ALPN) |
| `7878/udp` | UDP | HTTP/3 / QUIC (only active when a TLS cert is configured) |

Change the port at runtime with `RWS_CONFIG_PORT`.

## Environment variable injection

All `RWS_CONFIG_*` variables can be passed via `docker run -e` or a `.env` file:

```bash
docker run \
  -e RWS_CONFIG_IP=0.0.0.0 \
  -e RWS_CONFIG_PORT=8080 \
  -e RWS_CONFIG_THREAD_COUNT=8 \
  -e RWS_CONFIG_LOG_FORMAT=json \
  -p 8080:8080 \
  my-rws-image
```

## docker-compose with TLS

Mount your certificate files as a read-only volume and pass the paths via environment variables:

```yaml
# docker-compose.yml
version: "3.9"

services:
  rws:
    build: .
    ports:
      - "443:7878"
      - "443:7878/udp"   # QUIC / HTTP/3
      - "80:7879"         # HTTP → HTTPS redirect port
    environment:
      RWS_CONFIG_IP: "0.0.0.0"
      RWS_CONFIG_PORT: "7878"
      RWS_CONFIG_TLS_CERT_FILE: "/run/secrets/cert.pem"
      RWS_CONFIG_TLS_KEY_FILE: "/run/secrets/key.pem"
      RWS_CONFIG_HTTP_REDIRECT_PORT: "7879"
      RWS_CONFIG_LOG_FORMAT: "json"
    volumes:
      - ./certs/cert.pem:/run/secrets/cert.pem:ro
      - ./certs/key.pem:/run/secrets/key.pem:ro
    healthcheck:
      test: ["CMD", "curl", "-fk", "https://localhost:7878/healthz"]
      interval: 15s
      timeout: 3s
      retries: 3
      start_period: 10s
    restart: unless-stopped
```

:::note[SIGHUP hot-reload inside Docker]
Send `docker kill --signal=SIGHUP <container>` to reload TLS certificates, CORS rules, rate limits, and log format without restarting the container. The server's `/admin/config/reload` endpoint does the same over HTTP.
:::
