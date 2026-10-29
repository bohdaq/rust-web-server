---
title: Config File (rws.config.toml)
description: Complete annotated reference for every section and key in rws.config.toml.
---

`rws.config.toml` is an optional TOML file placed in the server's working
directory. It provides a convenient alternative to environment variables and CLI
flags. Any key set here overrides the built-in default and any system environment
variable, but can be overridden by a CLI flag.

The file is auto-detected at startup. Override the path with the
`RWS_CONFIG_FILE` environment variable (used for both server settings and
proxy routes):

```bash
RWS_CONFIG_FILE=/etc/rws/prod.toml rws
```

### Check the file before you deploy

`rws --check` validates the configuration without starting the server and
exits `0` if it has no errors, `1` otherwise — like `nginx -t`:

```bash
rws --check                       # the file rws would load
rws --check=/etc/rws/prod.toml    # a specific file
rws --check --format=json         # machine-readable, for CI and editors
```

With `--format=json` the report is one JSON object —
`{"path":…,"ok":…,"errors":[…],"warnings":[…]}` — with the same exit codes.

```text
error: [route.action] upsteam: unknown key — ignored
error: route 'api': upstream 'missing' is not a declared [[upstream]]
warning: [acme] without http_redirect_port: nothing answers the HTTP-01 challenge on port 80; set http_redirect_port = 80
rws.config.toml: 2 errors, 1 warning
```

It reports unknown keys anywhere in the file, values of the wrong type,
certificate and key files that are missing or don't load, ACME mistakes,
proxy routes that point at undeclared upstreams, unknown action types, IP
filter entries that aren't addresses, and auth secrets that aren't set.
Secrets and files are checked in the current environment, so run it where the
server runs. Proxy problems are also printed when the server starts.

The same checks are a library call, e.g. in a test that keeps a deployment's
config honest:

```rust
use rust_web_server::config_check;

#[test]
fn production_config_is_valid() {
    let report = config_check::check_file("deploy/rws.config.toml");
    assert!(report.is_ok(), "{}", report.render("deploy/rws.config.toml"));
}
```

`report.render_json(path)` gives the `--format=json` output.

### How keys map to settings

Every setting has an `RWS_CONFIG_*` environment variable. In the file, a key's
variable is its table name plus the key, upper-cased:

| In `rws.config.toml` | Setting |
|---|---|
| `port = 443` (top level) | `RWS_CONFIG_PORT` |
| `[cors]` `allow_all = false` | `RWS_CONFIG_CORS_ALLOW_ALL` |
| `[rate_limit]` `max_requests = 500` | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` |
| 2nd `[[virtual_host]]` block, `domain = ...` | `RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN` |

Strings keep their exact contents (`csp = "default-src 'self'; img-src *"`
works); arrays become comma-separated lists. A key that doesn't correspond to
any setting is reported at startup — `Warning: unknown setting '[server] port'
in rws.config.toml — ignored` — rather than silently dropped. `[[route]]`,
`[[upstream]]`, `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]`, and
`[middleware]` tables configure the proxy (below) and aren't settings.

An inline table is the same as a sub-table, here and in the proxy sections:

```toml
rate_limit = { max_requests = 1000, window_secs = 60 }
# same as
[rate_limit]
max_requests = 1000
window_secs  = 60
```

:::note[Proxy mode]
When the file contains at least one `[[route]]` or `[[upstream]]` section, the
server starts in **proxy mode** (`ConfigDrivenApp`). The built-in static-file and
controller stack is used only as a fallback for unmatched requests.
:::

## Server settings (top level)

Basic server settings go at the top of the file, before any `[section]`
header. Each key maps to the `RWS_CONFIG_*` environment variable and the
`--flag` of the same name (`rws --help` lists them all).

```toml
ip                 = "0.0.0.0"   # RWS_CONFIG_IP (default: 0.0.0.0)
port               = 443         # RWS_CONFIG_PORT (default: 7878)
thread_count       = 200         # RWS_CONFIG_THREAD_COUNT (default: 200, http1 build only)
tls_cert_file      = "cert.pem"  # RWS_CONFIG_TLS_CERT_FILE — enables HTTPS, HTTP/2, HTTP/3
tls_key_file       = "key.pem"   # RWS_CONFIG_TLS_KEY_FILE
http_redirect_port = 80          # RWS_CONFIG_HTTP_REDIRECT_PORT — HTTP → HTTPS redirect listener
request_allocation_size_in_bytes = 10000  # RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES — per-read chunk size
```

:::caution
Don't wrap these in a `[server]` table: keys are prefixed with their table's
name, so `[server] port = 443` becomes `--server-port`, which doesn't exist
and is ignored.
:::

## `[acme]`

Automatic HTTPS from Let's Encrypt or another ACME CA (`acme` feature,
included in the prebuilt binaries). Setting `domains` enables it. See
[Automatic HTTPS](/features/acme/) for how it works.

```toml
[acme]
domains        = ["example.com", "www.example.com"]  # RWS_CONFIG_ACME_DOMAINS
email          = "ops@example.com"                   # RWS_CONFIG_ACME_EMAIL
staging        = false             # Let's Encrypt staging CA (testing)
directory      = ""                # ACME directory URL of another CA
ca_file        = ""                # extra root CA PEM for a private ACME CA
challenge      = "http-01"         # or "tls-alpn-01" (port 443 only), "dns-01" (wildcards),
                                   # "dns-persist-01" (a TXT record you publish once)
challenge_port = 80                # port the CA connects to for HTTP-01
# dns_provider = "rfc2136"         # dns-01: rfc2136, cloudflare, route53 or exec; dns_server, dns_zone,
#                                  # dns_tsig_key, dns_tsig_secret, dns_tsig_algorithm,
#                                  # dns_exec, dns_api_token, dns_zone_id, dns_api_url,
#                                  # dns_propagation_secs — see Automatic HTTPS
# redis        = "10.0.0.20:6379"  # replicas share certificates and challenges
#                                  # (redis_password, redis_prefix)
# on_demand     = true             # certificates for names on_demand_ask allows,
# on_demand_ask = "http://127.0.0.1:9000/allowed"   # at their first handshake
cert_path      = ""                # default: tls_cert_file, else cert.pem
key_path       = ""                # default: tls_key_file, else key.pem
profile        = ""                # certificate profile, e.g. "shortlived"
# renew_before_days       = 30                  # default: at 2/3 of the lifetime
renew_check_interval_secs = 43200               # longest time between checks
account_key_path          = "acme_account.key"  # ACME account key, kept between restarts
```

## `[cors]`

CORS policy applied to every response. All fields are hot-reloadable.

```toml
[cors]
# Allow every origin, with credentials (default false since 18.0.0). When
# true, the specific allow_* fields below are ignored.
allow_all         = false

# Restrict to specific origins (comma-separated). Used when allow_all = false.
allow_origins     = "https://app.example.com,https://admin.example.com"

# Allowed HTTP methods (comma-separated).
allow_methods     = "GET,POST,PUT,DELETE,OPTIONS"

# Allowed request headers, in lowercase (comma-separated).
allow_headers     = "content-type,authorization,x-api-key"

# Whether to allow cookies and credentials with cross-origin requests.
allow_credentials = "true"

# Response headers the browser may expose to JavaScript (comma-separated, lowercase).
expose_headers    = "x-request-id,x-rate-limit-remaining"

# Preflight cache duration in seconds (default: 86400 = 24 hours).
max_age           = 86400
```

## `[rate_limit]`

Global rate limit per client IP: requests over the budget get
`429 Too Many Requests`. Off unless `max_requests` is set — enabling it needs a
restart; changing the numbers afterwards is hot-reloadable. For per-route
limits in proxy mode use `[route.middleware] rate_limit` instead.

```toml
[rate_limit]
max_requests = 1000   # Requests allowed per window; 0 or unset = no global limit
window_secs  = 60     # Sliding window length in seconds (default: 60)
```

## Other settings

```toml
log_format              = "json"     # RWS_CONFIG_LOG_FORMAT: "json" (default) | "combined" | "off"
watch                   = true       # RWS_CONFIG_WATCH: reload when this file or a certificate changes (default false)
proxy_protocol          = true       # RWS_CONFIG_PROXY_PROTOCOL: connections start with a PROXY protocol header (default false)
access_log              = "/var/log/rws/access.log"  # RWS_CONFIG_ACCESS_LOG: write the access log here instead of stdout
access_log_max_size     = "100M"     # RWS_CONFIG_ACCESS_LOG_MAX_SIZE: rotate past this size; 0 (default) = never (use logrotate + SIGUSR1)
access_log_keep         = 5          # RWS_CONFIG_ACCESS_LOG_KEEP: rotated files kept (access.log.1 … .5)
max_body_size_in_bytes  = 10485760   # RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES: 413 above this; 0 = unlimited
open_file_cache_secs    = 1          # RWS_CONFIG_OPEN_FILE_CACHE_SECS: reuse open static files this long; 0 = off
open_file_cache_strict  = true       # RWS_CONFIG_OPEN_FILE_CACHE_STRICT: false = trust size/mtime for that long (no fstat per request)
runtime                 = "per-core" # RWS_CONFIG_RUNTIME: one single-threaded runtime per core (SO_REUSEPORT; the binary's default since 18.0.0); "shared" = one multi-threaded runtime
csp                     = "default-src 'self'"   # RWS_CONFIG_CSP: Content-Security-Policy header
security_headers        = true       # RWS_CONFIG_SECURITY_HEADERS: X-Frame-Options, Referrer-Policy, Permissions-Policy, default CSP
nanos_headers           = false      # RWS_CONFIG_NANOS_HEADERS: Date-/Last-Modified-Unix-Epoch-Nanos
static_cache_control    = "no-cache" # RWS_CONFIG_STATIC_CACHE_CONTROL: Cache-Control for static files
client_hints            = false      # RWS_CONFIG_CLIENT_HINTS: send Accept-CH / Critical-CH
tls_client_ca_file      = "/etc/rws/clients-ca.pem"  # RWS_CONFIG_TLS_CLIENT_CA_FILE: require client certs (mTLS)
template_dir            = "templates"            # RWS_CONFIG_TEMPLATE_DIR
serve_dotfiles          = false        # RWS_CONFIG_SERVE_DOTFILES: serve .git/, .env, … (.well-known/ always)
spa_fallback            = "index.html"   # RWS_CONFIG_SPA_FALLBACK: file served for client-side routes
spa_fallback_exclude_prefixes = ["/api"]   # RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES

[gateway_api]                        # RWS_CONFIG_GATEWAY_API_*: the Kubernetes Gateway API controller (gateway-api feature)
enabled         = true
controller_name = "rws8.tech/gateway-controller"
pod_selector    = "app.kubernetes.io/name=rws"
port_range      = "10000-10999"
service_type    = "LoadBalancer"
mode            = "shared"           # or "provisioned": a Deployment per Gateway
image           = "ghcr.io/bohdaq/rws:18.8.0"   # provisioned pods' image

[circuit_breaker]                    # RWS_CONFIG_CIRCUIT_BREAKER_*: read by RedisCircuitBreaker::from_env()
failure_threshold      = 5
recovery_secs          = 30
max_half_open_probes   = 1
```

See [Environment Variables](/configuration/env-vars/) for what each one does.

## Request limits

Fixed limits that apply whatever the configuration:

| Limit | Value | Over the limit |
|---|---|---|
| Request head (request line + headers), HTTP/1.1 | 64 KiB | `431 Request Header Fields Too Large` |
| Header list, HTTP/2 and HTTP/3 | 64 KiB | stream reset |
| Concurrent streams per HTTP/2 connection | 250 | stream refused |
| Idle time between reads | 30 s | connection closed |
| HTTP/2 connection with nothing running and nothing received | 30 s | `GOAWAY(NO_ERROR)`, connection closed |
| HTTP/2 stream resets, the client's or provoked by it (rapid reset, MadeYouReset) | 1000 per 10 s | `GOAWAY(ENHANCE_YOUR_CALM)` |
| Whole request head, from its first byte (HTTP/1.1) | 30 s | `408 Request Timeout` |
| TLS handshake | 10 s | connection closed |

The request body limit is `max_body_size_in_bytes` above. On HTTP/1.1, bodies
may be sent with `Content-Length` or `Transfer-Encoding: chunked`; a request
that carries both, or two different `Content-Length` values, is rejected with
`400` so `rws` and a proxy in front of it can never disagree about where a
request ends.

For the same reason an HTTP/1.1 request head is read strictly (17.113.2) and
answered `400` when it has no `Host` or more than one, a method not in upper
case (`get`), a version other than `HTTP/1.0`/`HTTP/1.1`, a folded header
line (starting with a space or tab), whitespace before a header's colon, or a
header name with characters a token can't have. `Connection` is read token by
token: `Connection: close, TE` closes the connection after the response.

## `[[virtual_host]]`

Repeat this section for each domain that needs its own TLS certificate. The SNI
resolver selects the right certificate automatically at handshake time.

```toml
[[virtual_host]]
domain    = "example.com"
cert_file = "/etc/ssl/example.com.pem"
key_file  = "/etc/ssl/example.com.key"

[[virtual_host]]
domain    = "api.example.com"
cert_file = "/etc/ssl/api.example.com.pem"
key_file  = "/etc/ssl/api.example.com.key"
```

## `[[upstream]]`

Defines a named backend pool. Upstreams are referenced by name from routes.

```toml
[[upstream]]
name      = "api"
backends  = ["localhost:3000", "localhost:3001"]
strategy  = "round_robin"   # "round_robin" | "random" | "ip_hash" | "least_connections" (default: round_robin)
# tls_ca_file = "/etc/rws/internal-ca.pem"  # TLS backends: trust this CA instead of the public roots
# tls_client_cert_file = "/etc/rws/client.pem"  # TLS backends that require a client certificate (mutual TLS),
# tls_client_key_file  = "/etc/rws/client.key"  #   with its private key — since 18.3.0

[upstream.health_check]
path                = "/health"  # HTTP path polled on each backend (default: /health)
interval_secs       = 30         # Poll interval (default: 30)
timeout_ms          = 5000       # Connect + read timeout per poll (default: 5000)
healthy_threshold   = 2          # Consecutive passes before marking healthy (default: 2)
unhealthy_threshold = 3          # Consecutive failures before marking unhealthy (default: 3)
```

When the health checker removes all backends, the route returns `502 Bad Gateway`
until at least one backend recovers.

## `[[route]]`

Each `[[route]]` entry maps a set of match criteria to an action and an optional
per-route middleware stack. Routes are evaluated in declaration order; the first
match wins.

### `[route.match]`

All criteria are optional. A request must satisfy every criterion that is set.

```toml
[[route]]
name = "api-proxy"

[route.match]
host         = "api.example.com"  # SNI hostname or Host header (exact match)
path         = "/api/*"           # Prefix match when ending with *; exact match otherwise
method       = "POST"             # HTTP method (case-insensitive; matches any if omitted)
content_type = "application/json" # Content-Type prefix match
```

### `[route.action]` — proxy

Forward requests to a named upstream pool.

```toml
[route.action]
type = "proxy"

[route.action.proxy]
upstream            = "api"          # Must match a [[upstream]] name
connect_timeout_ms  = 5000           # (default: 5000)
read_timeout_ms     = 30000          # (default: 30000)
strip_path_prefix   = "/api"         # Strip this prefix before forwarding (optional)
add_path_prefix     = "/v2"          # Add this prefix after stripping (optional)
```

### `[route.action]` — grpc

Forward gRPC traffic over HTTP/2 to a named upstream.

```toml
[route.action]
type = "grpc"

[route.action.grpc]
upstream           = "grpc-backend"
connect_timeout_ms = 5000
read_timeout_ms    = 30000
```

### `[route.action]` — static

Serve static files from a directory on disk.

```toml
[route.action]
type = "static"

[route.action.static]
root  = "./public"
index = ["index.html", "index.htm"]
```

### `[route.action]` — redirect

Issue an HTTP redirect.

```toml
[route.action]
type = "redirect"

[route.action.redirect]
location = "https://new.example.com$path"  # $path is replaced with the request URI
status   = 301                             # (default: 301)
```

### `[route.action]` — respond

Return a fixed response body without proxying.

```toml
[route.action]
type = "respond"

[route.action.respond]
status       = 200
body         = "{\"ok\":true}"
content_type = "application/json"   # (default: text/plain)
```

### `[route.action]` — mcp

Accepted by the parser, but not served from the config file: `rws --check`
warns, and matching requests fall through to the static files. Mount MCP in
code instead (see [MCP](/mcp/overview/)).

```toml
[route.action]
type = "mcp"
```

### `[route.middleware]`

Per-route middleware applied before the action. All sub-sections are optional.

```toml
[route.middleware]
timeout_ms    = 5000      # 504 Gateway Timeout past this (0/unset = none)
max_body_size = 1048576   # 413 above this many bytes (0/unset = only the global limit)

[route.middleware.rate_limit]
max_requests = 100
window_secs  = 60

[route.middleware.cache]
ttl_secs = 300
vary_by  = ["Accept-Encoding", "Authorization"]

[route.middleware.auth]
type      = "bearer"        # "bearer" | "jwt" | "basic"
token_env = "API_TOKEN"     # env var name holding the expected token (bearer)
# secret_env = "JWT_SECRET" # env var name holding the JWT secret (jwt)
# htpasswd_file = "users.htpasswd" # path to htpasswd file (basic)

[route.middleware.ip_filter]
allow = ["10.0.0.0/8", "192.168.1.0/24"]
deny  = ["10.0.0.5"]
```

A top-level `[middleware]` table (same keys and sub-tables as
`[route.middleware]`) wraps every route and the static fallback.

#### Request rewrite rules

```toml
[[route.middleware.rewrite.request]]
type  = "header_set"
name  = "X-Forwarded-Proto"
value = "https"

[[route.middleware.rewrite.request]]
type   = "uri_strip_prefix"
prefix = "/api"

[[route.middleware.rewrite.request]]
type   = "uri_add_prefix"
prefix = "/v2"

[[route.middleware.rewrite.request]]
type  = "header_remove"
name  = "X-Internal-Secret"

[[route.middleware.rewrite.request]]
type  = "uri_set"
value = "/health"
```

#### Response rewrite rules

```toml
[[route.middleware.rewrite.response]]
type  = "header_set"
name  = "X-Powered-By"
value = "rws"

[[route.middleware.rewrite.response]]
type  = "header_remove"
name  = "Server"

[[route.middleware.rewrite.response]]
type   = "status"
code   = 200
reason = "OK"

[[route.middleware.rewrite.response]]
type = "body_replace"
from = "old-domain.com"
to   = "new-domain.com"
```

## `[[tcp_proxy]]`

Standalone L4 TCP proxy. Binds its own port and relays bytes bidirectionally
between the client and a backend. Runs in a background thread and is independent
of the HTTP server.

```toml
[[tcp_proxy]]
name               = "db-proxy"
listen             = "0.0.0.0:5433"               # Address to bind
backends           = ["db-primary:5432", "db-replica:5432"]
connect_timeout_ms = 5000                          # (default: 5000)
```

## `[[udp_proxy]]`

Standalone UDP datagram proxy. Each datagram is forwarded to a backend and the
reply is relayed back to the originating client.

```toml
[[udp_proxy]]
name              = "dns-proxy"
listen            = "0.0.0.0:5353"
backends          = ["8.8.8.8:53", "8.8.4.4:53"]
reply_timeout_ms  = 5000    # How long to wait for a reply (default: 5000)
buffer_size       = 65536   # Datagram buffer in bytes (default: 65536)
```

## `[[ws_proxy]]`

Standalone WebSocket proxy. Handles the HTTP upgrade handshake then relays raw
bytes bidirectionally between client and backend.

```toml
[[ws_proxy]]
name               = "ws-backend"
listen             = "0.0.0.0:8080"
backends           = ["localhost:8081"]
connect_timeout_ms = 5000    # (default: 5000)
read_timeout_ms    = 30000   # (default: 30000)

[ws_proxy.health_check]       # optional, same keys as [upstream.health_check]
path = "/health"
```

## Complete example

```toml
ip                 = "0.0.0.0"
port               = 443
http_redirect_port = 80

[acme]
domains = ["example.com", "api.example.com"]
email   = "ops@example.com"

[cors]
allow_all = false
allow_origins = "https://app.example.com"
allow_methods = "GET,POST,PUT,DELETE"
allow_headers = "content-type,authorization"
max_age       = 86400

[rate_limit]
max_requests = 500
window_secs  = 60

[[virtual_host]]
domain    = "example.com"
cert_file = "/etc/ssl/certs/example.pem"
key_file  = "/etc/ssl/private/example.key"

[[upstream]]
name      = "api"
backends  = ["localhost:3000", "localhost:3001"]
strategy  = "round_robin"

[upstream.health_check]
path                = "/health"
interval_secs       = 15
healthy_threshold   = 2
unhealthy_threshold = 3

[[route]]
name = "api"

[route.match]
path = "/api/*"

[route.action]
type = "proxy"

[route.action.proxy]
upstream          = "api"
strip_path_prefix = "/api"

[route.middleware.rate_limit]
max_requests = 100
window_secs  = 60

[[route]]
name = "health"

[route.match]
path = "/ping"

[route.action]
type = "respond"

[route.action.respond]
status = 200
body   = "pong"
```
