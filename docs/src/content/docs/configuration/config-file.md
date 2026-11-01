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

For configuration text from someone else, `config_check::check_str_offline(text)`
runs the same checks without looking at this machine: no file is opened or
tested (certificates, htpasswd files, static roots) and no environment
variable is read (auth secrets, DNS credentials). The docs MCP server's
`check_config` tool uses it.

### How keys map to settings

Every setting has an `RWS_CONFIG_*` environment variable. In the file, a key's
variable is its table name plus the key, upper-cased:

| In `rws.config.toml` | Setting |
|---|---|
| `port = 443` (top level) | `RWS_CONFIG_PORT` |
| `[cors]` `allow_all = false` | `RWS_CONFIG_CORS_ALLOW_ALL` |
| `[rate_limit]` `max_requests = 500` | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` |
| 2nd `[[virtual_host]]` block, `domain = ...` | `RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN` |

Strings are TOML strings, here and in the proxy sections: in a basic string
(`"…"`) the escapes `\n \r \t \b \f \e \" \\ \uXXXX \UXXXXXXXX` stand for
the characters they name (`body = "line 1\r\nline 2"` is two lines); a
literal string (`'…'`) is taken exactly as written (`'C:\new'`). A backslash
before anything else in a basic string is kept as written, so `"^/v\d+/"`
stays as it is. Quotes of the other kind need no escape
(`csp = "default-src 'self'; img-src *"` works); arrays become
comma-separated lists. A key that doesn't correspond to
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
# tls              = "internal"  # RWS_CONFIG_TLS: HTTPS from rws's own CA instead (local development, rws trust)
# tls_internal_names = "app.localhost"  # RWS_CONFIG_TLS_INTERNAL_NAMES: more names for that certificate
# tls_internal_dir = "/path/pki" # RWS_CONFIG_TLS_INTERNAL_DIR: where the CA lives (default: per-user data dir)
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

## `[admin]`

The [admin API](/deployment/admin/) on a listener of its own (since 18.10.0). Off unless `listen` is set; the bearer token comes from the environment variable `token_env` names, and without it the admin listener doesn't start. Read at startup.

```toml
[admin]
listen          = "127.0.0.1:9901"            # host:port (an IP address)
token_env       = "RWS_ADMIN_TOKEN"           # default
allowed_origins = ["https://ops.example.com"] # browser origins besides the listener's own
# users         = ["deploy-bot:RWS_ADMIN_DEPLOY:operator"]  # more tokens: name:ENV_VAR:role (viewer, operator, admin)
# audit_log     = "/var/log/rws/audit.log"     # append every change here too (JSON lines)
# oidc_issuer   = "https://id.example.com/realms/main"     # single sign-on for the admin UI (or oidc_provider)
# oidc_client_id = "rws-admin"
# oidc_client_secret_env = "RWS_ADMIN_OIDC_SECRET"
# oidc_cookie_secret_env = "RWS_ADMIN_COOKIE_SECRET"
# oidc_roles    = ["rws-admins=admin", "sre=operator", "*=viewer"]  # group (or email, or *) = role
# oidc_role_claim = "groups"
```


## `[traffic]`

Traffic insights for the admin UI (since 18.15.0): on whenever the admin API
is, counting people, AI agents and bots per hour and day. No IP addresses or
User-Agents are kept.

```toml
[traffic]
# enabled        = false                     # on with [admin] listen; false turns it off
retention_days   = 30                        # 1–90
# persist        = "/var/lib/rws/traffic.json"  # default: traffic.json beside the certificate; "off"
# exclude        = ["/healthz", "/readyz", "/metrics"]
```
## Other settings

```toml
log_format              = "json"     # RWS_CONFIG_LOG_FORMAT: "json" (default) | "combined" | "off"
watch                   = true       # RWS_CONFIG_WATCH: reload when this file or a certificate changes (default false)
proxy_protocol          = true       # RWS_CONFIG_PROXY_PROTOCOL: connections start with a PROXY protocol header (default false)
access_log              = "/var/log/rws/access.log"  # RWS_CONFIG_ACCESS_LOG: write the access log here instead of stdout
access_log_max_size     = "100M"     # RWS_CONFIG_ACCESS_LOG_MAX_SIZE: rotate past this size (default 100M since 18.15.0); 0 = never (use logrotate + SIGUSR1)
access_log_keep         = 5          # RWS_CONFIG_ACCESS_LOG_KEEP: rotated files kept (access.log.1 … .5)
min_free_disk           = "2G"       # RWS_CONFIG_MIN_FREE_DISK: stop writing the access log / disk cache below this free (or "5%"); default max(1%, 1G); "0" = off
max_body_size_in_bytes  = 10485760   # RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES: 413 above this; 0 = unlimited
open_file_cache_secs    = 1          # RWS_CONFIG_OPEN_FILE_CACHE_SECS: reuse open static files this long; 0 = off
open_file_cache_strict  = true       # RWS_CONFIG_OPEN_FILE_CACHE_STRICT: false = trust size/mtime for that long (no fstat per request)
runtime                 = "per-core" # RWS_CONFIG_RUNTIME: one single-threaded runtime per core (SO_REUSEPORT; the binary's default since 18.0.0); "shared" = one multi-threaded runtime
csp                     = "default-src 'self'"   # RWS_CONFIG_CSP: Content-Security-Policy header
security_headers        = true       # RWS_CONFIG_SECURITY_HEADERS: X-Frame-Options, Referrer-Policy, Permissions-Policy, default CSP
nanos_headers           = false      # RWS_CONFIG_NANOS_HEADERS: Date-/Last-Modified-Unix-Epoch-Nanos
static_cache_control    = "no-cache" # RWS_CONFIG_STATIC_CACHE_CONTROL: Cache-Control for static files
error_format            = "problem"  # RWS_CONFIG_ERROR_FORMAT: rws-made errors as application/problem+json; "text" = plain text as before 18.17.0
proxy_status            = "always"   # RWS_CONFIG_PROXY_STATUS: Proxy-Status on proxied responses; "errors" or "off"
proxy_name              = "rws"      # RWS_CONFIG_PROXY_NAME: this hop's name in Proxy-Status / Cache-Status
proxy_status_next_hop   = false      # RWS_CONFIG_PROXY_STATUS_NEXT_HOP: add the backend address and error text
cache_status            = true       # RWS_CONFIG_CACHE_STATUS: Cache-Status on responses through a response cache
forwarded_header        = "add"      # RWS_CONFIG_FORWARDED_HEADER: RFC 7239 Forwarded on proxied requests; "off" leaves it out
tls_client_crl_file     = "/etc/rws/clients.crl.pem"  # RWS_CONFIG_TLS_CLIENT_CRL_FILE: revoked client certificates (PEM or DER, comma-separated)
tls_client_crl_strict   = false      # RWS_CONFIG_TLS_CLIENT_CRL_STRICT: refuse stale CRLs and certificates no CRL covers
compression_dictionaries = ["/js/app.*.js"]  # RWS_CONFIG_COMPRESSION_DICTIONARIES: later versions as dcz/dcb deltas
early_hints_http1       = true       # RWS_CONFIG_EARLY_HINTS_HTTP1: also send 103 Early Hints over HTTP/1.1 (default false)
directory_listing       = false      # RWS_CONFIG_DIRECTORY_LISTING: 404 instead of listing a directory without index.html (default true)
client_hints            = false      # RWS_CONFIG_CLIENT_HINTS: send Accept-CH / Critical-CH
tls_client_ca_file      = "/etc/rws/clients-ca.pem"  # RWS_CONFIG_TLS_CLIENT_CA_FILE: require client certs (mTLS)
template_dir            = "templates"            # RWS_CONFIG_TEMPLATE_DIR
serve_dotfiles          = false        # RWS_CONFIG_SERVE_DOTFILES: serve .git/, .env, … (.well-known/ always)
spa_fallback            = "index.html"   # RWS_CONFIG_SPA_FALLBACK: file served for client-side routes
spa_fallback_exclude_prefixes = ["/api"]   # RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES

[fleet]                              # RWS_CONFIG_FLEET_*: pull a signed config (18.23.0)
source        = "https://config.example.com/rws/{node}.toml"
public_key    = "kJ0pQ…="              # rws fleet keygen
interval_secs = 30
report_url    = "https://config.example.com/rws/report"

[cluster]                            # RWS_CONFIG_CLUSTER_*: runtime state shared by replicas (18.22.0)
redis  = "10.0.0.20:6379"            # default: [acme] redis
prefix = "rws:cluster:"
node   = "web-1"                     # default: the host name

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
# protocol = "h3"        # HTTP/3 (QUIC) to the https:// backends, TCP fallback — since 18.13.0

[upstream.health_check]
path                = "/health"  # HTTP path polled on each backend (default: /health)
interval_secs       = 30         # Poll interval (default: 30)
timeout_ms          = 5000       # Connect + read timeout per poll (default: 5000)
healthy_threshold   = 2          # Consecutive passes before marking healthy (default: 2)
unhealthy_threshold = 3          # Consecutive failures before marking unhealthy (default: 3)
```

When the health checker removes all backends, the route returns `502 Bad Gateway`
until at least one backend recovers.

## `[authz]`

Relationship-based authorization for routes with `[route.middleware.authz]`
(since 18.14.0) — see [Authorization](/features/authorization/#in-the-config-file).

```toml
[authz]
store  = "memory"                        # or "sql": RWS_DB_* (a model-* feature)
tuples = ["doc:42#owner@user:olga"]      # written at start and on reload; may span lines

[authz.type.doc]                         # one table per object type
owner  = "user"
viewer = "user | group#member | owner"   # terms: type, type#relation, relation, rel->rel
```

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
preserve_host       = true           # Send the client's Host upstream (default: false)
upstream_host       = "app.internal" # Send this Host upstream (optional; wins over preserve_host)
```

The upstream gets the backend's address as its `Host` unless the route says
otherwise: `preserve_host = true` passes on the client's (nginx's
`proxy_set_header Host $host`, for an application that builds its links from
the host it's asked for), `upstream_host` sends a fixed name. Over HTTP/3
upstreams they set `:authority`. The proxy writes `Host` itself, so a
`header_set` rewrite of `Host` doesn't reach the upstream — `rws --check`
warns about one. The client's host also goes upstream as `X-Forwarded-Host`
either way.

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

An [MCP gateway](/mcp/gateway/): one MCP endpoint in front of several upstream MCP servers, their tools prefixed, with per-caller access rules, rate limits and an audit log (since 18.9.0; before, `mcp` was accepted and not served). The route needs an exact `path`. Secrets stay out of the file: caller tokens and upstream header values come from the environment variables named.

```toml
[[route]]
  [route.match]
  path = "/mcp"

  [route.action]
  type = "mcp"

  [route.action.mcp]
  name            = "gateway"             # announced to clients (default rws-mcp-gateway)
  refresh_secs    = 300                   # re-read upstream tools; 0 = only at start
  audit_log       = true                  # one JSON line per tool call in the access log
  allowed_origins = ["https://app.example.com"]

  [[route.action.mcp.upstream]]
  prefix      = "github"                  # tools appear as github__<tool>
  url         = "https://mcp.github.example/mcp"
  timeout_ms  = 30000
  headers_env = { Authorization = "GITHUB_MCP_AUTHORIZATION" }   # header = env var with its value

  [[route.action.mcp.openapi]]             # an HTTP API's operations as tools (since 18.19.0)
  prefix      = "billing"
  spec        = "https://billing.internal/openapi.json"
  base_url    = "https://billing.internal"
  include     = ["getInvoice"]            # operationIds; default: every GET
  headers_env = { Authorization = "BILLING_API_AUTHORIZATION" }

  [[route.action.mcp.caller]]             # who may connect: their bearer token
  identity  = "alice"
  token_env = "ALICE_MCP_TOKEN"

  [[route.action.mcp.allow]]              # what they may use
  identity = "alice"
  tools    = ["github__*"]                # full names, prefixes ending in *, or *

  [[route.action.mcp.limit]]              # per caller and tool
  tools       = "github__create_issue"
  max_calls   = 5
  window_secs = 3600
```

The gateway lists its upstreams in the background at start: until it has, and while an upstream is down then, the route answers `503` and the build is retried every 10 s. `rws --check` reports unknown keys, bad prefixes and URLs, and unset token and header variables.

### `[route.action]` — a2a

An [A2A gateway](/mcp/a2a/#gateway): a public address for an A2A agent running elsewhere (since 18.18.0). Its card is served at `<path>/.well-known/agent-card.json` with the JSON-RPC interface pointing at this route, and JSON-RPC `POST`s to `<path>` go to the agent, streams included. Use the route's middleware for auth and rate limits; the caller's `Authorization` isn't passed on.

```toml
[[route]]
  [route.match]
  path = "/agents/billing/*"

  [route.action]
  type = "a2a"

  [route.action.a2a]
  upstream    = "http://10.0.0.7:9000"   # the agent's base URL (its card at /.well-known/agent-card.json)
  timeout_ms  = 300000                   # default; SendMessage may wait for its task
  audit_log   = true                     # one JSON line per call: client, method, status, duration
  headers_env = { Authorization = "BILLING_AGENT_AUTHORIZATION" }  # sent to the agent, from the environment
```

### `[route.action]` — llm

An [LLM proxy](/proxy/llm-proxy/) in front of OpenAI-compatible and Anthropic providers: virtual keys, fallback, token usage, budgets (since 18.9.0). Provider keys, virtual keys and the Redis password come from the environment variables named.

```toml
[[route]]
  [route.match]
  path = "/v1/*"

  [route.action]
  type = "llm"

  [route.action.llm]
  audit_log          = true
  redis              = "127.0.0.1:6379"   # budgets shared between replicas (default: in memory)
  redis_password_env = "REDIS_PASSWORD"
  redis_prefix       = "rws:llm:"
  connect_timeout_ms = 10000
  read_timeout_ms    = 300000
  allow_anonymous    = false

  [[route.action.llm.provider]]           # tried in order, per API
  name    = "openai"
  api     = "openai"                      # or "anthropic"
  url     = "https://api.openai.com"
  key_env = "OPENAI_API_KEY"              # optional (local servers)

  [[route.action.llm.key]]                # a virtual key clients send
  identity      = "alice"
  key_env       = "ALICE_LLM_KEY"
  budget_tokens = 2000000                 # optional, with window_secs
  window_secs   = 86400
```

### `[route.middleware]`

Per-route middleware applied before the action. All sub-sections are optional.

```toml
[route.middleware]
timeout_ms    = 5000      # 504 Gateway Timeout past this (0/unset = none)
max_body_size = 1048576   # 413 above this many bytes (0/unset = only the global limit)
early_hints   = ["</app.css>; rel=preload; as=style"]   # sent as a 103 before the route answers (18.12.0)

[route.middleware.rate_limit]
max_requests = 100
window_secs  = 60

[route.middleware.cache]
ttl_secs = 300
vary_by  = ["Accept-Encoding", "Authorization"]
# dir      = "/var/cache/rws/api"   # on disk instead of in memory (18.11.0)
# max_size = "512M"                 # disk bound, default 256M
# purge_token_env = "RWS_PURGE_TOKEN"  # accept HTTP PURGE with this bearer token (18.11.0)

[route.middleware.authz]                 # caller needs relation on object (18.14.0)
relation = "viewer"
object   = "doc:{path:2}"

[route.middleware.auth]
type      = "bearer"        # "bearer" | "jwt" | "basic"
token_env = "API_TOKEN"     # env var name holding the expected token (bearer)
# secret_env = "JWT_SECRET" # env var name holding the JWT secret (jwt)
# previous_secret_env = "JWT_SECRET_OLD" # also accepted while rotating (jwt, 18.20.0)
# htpasswd_file = "users.htpasswd" # path to htpasswd file (basic)

[route.middleware.ip_filter]
allow = ["10.0.0.0/8", "192.168.1.0/24"]
deny  = ["10.0.0.5"]

[route.middleware.wasm]             # a WebAssembly plugin (feature wasm-plugins, 18.10.0)
module    = "plugins/tag.wasm"
fuel      = 50000000                # CPU per hook call
memory_mb = 16
config    = "region=eu"             # passed to the plugin's rws_init
```

See [WebAssembly plugins](/features/wasm-plugins/) for the plugin ABI and sandbox.

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
