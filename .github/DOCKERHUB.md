# rws — web server and reverse proxy

A single-binary web server and reverse proxy written in Rust: static files,
automatic HTTPS from Let's Encrypt, HTTP/1.1, HTTP/2 and HTTP/3, load
balancing with health checks, auth and rate limiting — configured in one TOML
file. **[Docs](https://rws8.tech) · [GitHub](https://github.com/bohdaq/rust-web-server)**

- **Small:** a static binary on [distroless](https://github.com/GoogleContainerTools/distroless), runs as a non-root user, about 12 MB.
- **Multi-arch:** `linux/amd64` and `linux/arm64` (Apple Silicon, Raspberry Pi, Graviton).
- **Tags:** `latest`, the major version (`17`), and every release (`17.113.0`). Pin a full version in production.

## Serve a folder

```bash
docker run -p 7878:7878 -v "$PWD:/srv:ro" bohdaq/rws
```

Open <http://localhost:7878>. The working directory inside the container is
`/srv`: mount your site there.

## Automatic HTTPS

Point your domain's DNS at the host, then put this `rws.config.toml` in the
folder you mount:

```toml
port               = 443
http_redirect_port = 80

[acme]
domains = ["example.com"]
email   = "you@example.com"
```

```bash
docker run -d --name rws -p 80:80 -p 443:443 -p 443:443/udp \
  -v "$PWD:/srv:ro" -v rws-data:/data bohdaq/rws
```

`rws` gets a certificate from Let's Encrypt on first start, serves HTTPS,
HTTP/2 and HTTP/3 (UDP 443), redirects HTTP to HTTPS, and renews in the
background. Certificates and the ACME account key are kept in the `/data`
volume, so restarts reuse them. Try the Let's Encrypt staging CA first with
`staging = true` under `[acme]`. → [Automatic HTTPS](https://rws8.tech/features/acme/)

## Reverse proxy

```toml
port = 7878

[[upstream]]
name     = "app"
backends = ["app1:8080", "app2:8080"]

  [upstream.health_check]
  path = "/healthz"

[[route]]
  [route.match]
  path = "/api/*"

  [route.action]
  type     = "proxy"
  upstream = "app"
```

Load balancing, health checks, bearer / JWT / basic auth, rate limits,
caching, rewrites, and TCP, UDP, WebSocket and gRPC proxying.
→ [Config-driven proxy](https://rws8.tech/proxy/config-driven/)

## Configuration

Every setting can go in `rws.config.toml`, on the command line, or in an
environment variable:

| `rws.config.toml` | Environment variable | Default |
|---|---|---|
| `port` | `RWS_CONFIG_PORT` | `7878` |
| `tls_cert_file`, `tls_key_file` | `RWS_CONFIG_TLS_CERT_FILE`, `RWS_CONFIG_TLS_KEY_FILE` | — |
| `log_format` | `RWS_CONFIG_LOG_FORMAT` | `json` (`combined`, `off`) |
| `[rate_limit] max_requests` | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | off |

```bash
docker run -p 8080:8080 -e RWS_CONFIG_PORT=8080 -v "$PWD:/srv:ro" bohdaq/rws
docker run --rm bohdaq/rws --help     # every option
```

Health and metrics: `GET /healthz`, `GET /readyz`, `GET /metrics` (Prometheus).
→ [All settings](https://rws8.tech/configuration/config-file/)

The image is also published as `ghcr.io/bohdaq/rws`. Report security issues
privately: [SECURITY.md](https://github.com/bohdaq/rust-web-server/blob/main/SECURITY.md).
