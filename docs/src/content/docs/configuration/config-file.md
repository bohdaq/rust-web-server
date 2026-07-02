---
title: Config File (rws.config.toml)
description: Complete annotated reference for every section and key in rws.config.toml.
---

`rws.config.toml` is an optional TOML file placed in the server's working
directory. It provides a convenient alternative to environment variables and CLI
flags. Any key set here overrides the built-in default and any system environment
variable, but can be overridden by a CLI flag.

The file is auto-detected at startup. Override the path with the
`RWS_CONFIG_FILE` environment variable:

```bash
RWS_CONFIG_FILE=/etc/rws/prod.toml rws
```

:::note[Proxy mode]
When the file contains at least one `[[route]]` or `[[upstream]]` section, the
server starts in **proxy mode** (`ConfigDrivenApp`). The built-in static-file and
controller stack is used only as a fallback for unmatched requests.
:::

## `[server]`

Basic server settings. All keys map to the corresponding `RWS_CONFIG_*`
environment variable.

```toml
[server]
ip            = "0.0.0.0"   # RWS_CONFIG_IP (default: 0.0.0.0)
port          = 7878         # RWS_CONFIG_PORT (default: 7878)
thread_count  = 200          # RWS_CONFIG_THREAD_COUNT (default: 200)
```

## `[cors]`

CORS policy applied to every response. All fields are hot-reloadable.

```toml
[cors]
# Allow every origin. When true, the specific allow_* fields below are ignored.
allow_all         = true

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

Global rate-limit policy applied per client IP. Hot-reloadable.

```toml
[rate_limit]
max_requests = 1000   # Requests allowed per window (default: 1000; 0 disables)
window_secs  = 60     # Sliding window length in seconds (default: 60)
```

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
strategy  = "round_robin"   # "round_robin" | "random" | "ip_hash" (default: round_robin)

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

Mount the MCP Streamable HTTP server on this route.

```toml
[route.action]
type = "mcp"
```

### `[route.middleware]`

Per-route middleware applied before the action. All sub-sections are optional.

```toml
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
# users_file = "users.htpasswd" # path to htpasswd file (basic)

[route.middleware.ip_filter]
allow = ["10.0.0.0/8", "192.168.1.0/24"]
deny  = ["10.0.0.5"]
```

#### Request rewrite rules

```toml
[[route.middleware.rewrite.request]]
type  = "set_header"
name  = "X-Forwarded-Proto"
value = "https"

[[route.middleware.rewrite.request]]
type   = "strip_path_prefix"
prefix = "/api"

[[route.middleware.rewrite.request]]
type   = "add_path_prefix"
prefix = "/v2"

[[route.middleware.rewrite.request]]
type  = "remove_header"
name  = "X-Internal-Secret"

[[route.middleware.rewrite.request]]
type  = "set_uri"
value = "/health"
```

#### Response rewrite rules

```toml
[[route.middleware.rewrite.response]]
type  = "set_header"
name  = "X-Powered-By"
value = "rws"

[[route.middleware.rewrite.response]]
type  = "remove_header"
name  = "Server"

[[route.middleware.rewrite.response]]
type   = "set_status"
code   = 200
reason = "OK"

[[route.middleware.rewrite.response]]
type = "replace_body"
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
```

## Complete example

```toml
[server]
ip           = "0.0.0.0"
port         = 7878
thread_count = 200

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
