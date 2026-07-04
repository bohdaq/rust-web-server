# rws

[![Crates.io](https://img.shields.io/crates/v/rust-web-server.svg)](https://crates.io/crates/rust-web-server)
[![docs.rs](https://docs.rs/rust-web-server/badge.svg)](https://docs.rs/rust-web-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![MSRV: 1.75](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

**Website:** [rws8.tech](https://rws8.tech/)

A dependency-minimal Rust web platform: HTTP/1.1, HTTP/2, and HTTP/3 server, reverse proxy, and application framework with routing, middleware (auth, rate limiting, tracing), an async ORM, background jobs, object storage, and a mailer. Runs as a zero-code config-driven proxy or as a library crate. No third-party HTTP dependencies.

| Mode | Setup | Code required |
|---|---|---|
| **Static file server** | `cargo install rust-web-server && rws` | None |
| **Config-driven proxy** | `rws.config.toml` with `[[route]]` / `[[upstream]]` | None |
| **Library crate** | `cargo add rust-web-server` | Yes |

---

## Why rws

- **No third-party HTTP stack.** HTTP parsing, JSON, CORS, MIME, range requests, WebSocket, SSE, and routing are all implemented from scratch in this one crate — instead of pinning Axum + Tower + Hyper + a proxy crate + a JWT crate + a base64 crate and keeping their versions compatible.
- **One `Middleware` trait, not ten Tower layers.** Auth, rate limiting, caching, tracing, rewriting, and the reverse proxy itself all implement the same `handle(request, connection, next)` signature — one pattern to learn, one pattern for an AI assistant to generate correctly.
- **The gateway is in the binary.** Reverse proxy, TCP/UDP/WebSocket proxying, health checks, circuit breakers, and canary routing ship in the same crate as the app framework — no separate Traefik/Nginx process to run in front of it for common cases.
- **SemVer since v1.** Frequent releases (currently v17) are additive; breaking changes only land on major version bumps. See [releases](https://github.com/bohdaq/rust-web-server/releases) for the changelog.

## Contents

- [Quick start — library](#quick-start--library)
- [Quick start — static file server](#quick-start--static-file-server)
- [Quick start — config-driven proxy](#quick-start--config-driven-proxy)
- [Building apps with AI](#building-apps-with-ai)
- [What's in the box](#whats-in-the-box)
- [Optional features](#optional-features)
- [Build from source](#build-from-source)
- [Further reading](#further-reading)

---

## Quick start — library

```toml
# Cargo.toml
[dependencies]
rust-web-server = "17"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use rust_web_server::prelude::*;

fn hello(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
    Response::get_response(
        STATUS_CODE_REASON_PHRASE.n200_ok,
        None,
        Some(vec![Range::get_content_range(
            b"Hello, world!".to_vec(),
            MimeType::TEXT_PLAIN.to_string(),
        )]),
    )
}

#[tokio::main]
async fn main() {
    let app = routes! {
        App::with_state(()),
        GET "/hello" => hello,
    };
    let (listener, pool) = Server::setup().unwrap();
    tokio::join!(
        Server::run_tls(listener, pool, app.clone()),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}
```

```bash
$ curl http://localhost:7878/hello
Hello, world!
```

See [DEVELOPER](DEVELOPER.md) for 59 use-case examples covering JSON, auth, WebSocket, SSE, middleware, ORM, MCP, and more.

---

## Quick start — static file server

```bash
cargo install rust-web-server
rws
```

Starts on `http://127.0.0.1:7878`. Place files in the working directory and open the URL.

### With HTTPS + HTTP/2 + HTTP/3

Generate a self-signed cert for local development:

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```

HTTP/2 and HTTP/3 are negotiated automatically — no extra configuration needed. See [CONFIGURE](CONFIGURE.md) for all options.

---

## Quick start — config-driven proxy

Drop `rws.config.toml` in the working directory and run `rws` — no code required:

```toml
[[upstream]]
name     = "api"
backends = ["10.0.0.10:8080", "10.0.0.11:8080"]

  [upstream.health_check]
  path                = "/healthz"
  interval_secs       = 10
  timeout_ms          = 2000
  healthy_threshold   = 2
  unhealthy_threshold = 3

[[route]]
  [route.match]
  host = "api.example.com"
  path = "/v1/*"

  [route.action]
  type     = "proxy"
  upstream = "api"

  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  auth       = { type = "bearer", token_env = "API_TOKEN" }

[[route]]
  [route.match]
  path = "/*"

  [route.action]
  type   = "respond"
  status = 404
  body   = "Not Found"
```

See [`spec/PROXY_SERVER_CONFIG.md`](spec/PROXY_SERVER_CONFIG.md) for the full annotated config reference.

---

## Building apps with AI

`rws` is written to be easy for AI coding assistants (Claude Code, Cursor, GitHub Copilot, ChatGPT, etc.) to generate correct code against on the first try — one consistent `Router`/`AppWithState` routing pattern, one `Middleware` trait for auth/rate-limiting/caching/everything else, and no third-party HTTP dependencies whose APIs a model might confuse with this crate's own.

Three files make that possible — point your AI tool at them before asking it to build something:

- **[llms.txt](llms.txt)** — a flat, LLM-optimized reference: every public type, middleware, and feature flag, with a runnable snippet for nearly every capability in the crate. This is the file to paste into a chat or system prompt, or fetch directly: `https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms.txt`.
- **[DEVELOPER.md](DEVELOPER.md)** — 72 numbered, runnable use cases (`## Use Case #N: Title`). Ask your assistant to follow the closest-matching use case instead of inventing a pattern from scratch.
- **[CLAUDE.md](CLAUDE.md)** — architecture, request lifecycle, and coding conventions. Claude Code reads this automatically when working inside this repo.

**Example prompt:**

```
I'm building on the rust-web-server (rws) crate. Read llms.txt at
https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms.txt
for the API surface, then build a REST API with:
- GET/POST /todos backed by SQLite (model-sqlite feature)
- JWT auth on POST (auth feature)
- Per-IP rate limiting on every route
```

Building an AI-powered *backend* rather than using AI to build the backend? See [AI & MCP](#ai--mcp) below — `McpServer` turns your app into a tool Claude, Cursor, and other MCP clients can call directly.

---

## What's in the box

<details>
<summary><strong>Protocol & transport</strong></summary>

- HTTP/3 over QUIC (UDP) + HTTP/2 + HTTP/1.1 on the same port via ALPN
- TLS via [rustls](https://github.com/rustls/rustls) — aws-lc-rs crypto, no OpenSSL
- Automatic TLS (ACME) — Let's Encrypt provisioning + background renewal (`acme` feature)
- mTLS — set `RWS_CONFIG_TLS_CLIENT_CA_FILE`; client cert required on HTTPS and QUIC
- Virtual hosting / SNI — per-domain TLS certs; `Router::with_host()` for per-host routing
- WebSocket (RFC 6455) — handshake, frame codec, SHA-1 + base64 built in, no extra dep
- Server-Sent Events — `Sse` builder with correct headers; ideal for AI token streaming
- Outbound HTTP client — `Client` (sync) and `AsyncClient` (async, `http2` feature); HTTPS via rustls

</details>

<details>
<summary><strong>Routing & app building</strong></summary>

- `routes!` macro + `App::with_state(S)` — typed shared state (`Arc<S>`) across handlers
- `Router` with `:param` / `*wildcard` path matching; `PathParams::get("name")`
- Async handlers via `App::with_async_state(S)` (`http2` feature)
- Middleware pipeline — `app.wrap(layer)` stacks composable `Middleware` layers
- Typed extractors — `Body`, `BodyText`, `Query`, `RequestHeaders`; `#[derive(FromRequest)]`
- Request validation — `#[derive(Validate)]` with `length`, `range`, `email`, `url`; returns `422`
- Typed errors — `AppError` enum (400–500); `IntoResponse` trait for custom error types
- Cookie jar — `CookieJar` parses; `SetCookie` builder writes all RFC 6265 attributes
- Sessions — `SessionStore` in-memory TTL sessions; `DbSessionStore` persistent sessions backed by the model layer (survives restarts, multi-instance); `RedisSessionStore` Redis-backed sessions with automatic TTL expiry; cookie helpers included
- JSON — `Json<T>` extractor + responder via `serde_json` (`serde` feature)
- HTML templates — Tera engine (Jinja2 syntax); `template::render()` one-liner; `template::reload()` hot-reloads edited templates from disk without a restart, wired into the same `SIGHUP` hook as CORS/rate-limit/TLS reload (`tera` feature)
- Dependency injection — `Container` keyed by `TypeId`; concrete types and `dyn Trait`
- In-process test client — `TestClient::new(app)` dispatches without a TCP socket
- Per-instance typed config — `ServerConfig` struct; `App::with_config(config)`, `AppWithState::with_config`, `AsyncAppWithState::with_config`, and `ConfigDrivenApp::with_config` all pin an app to explicit settings for parallel-safe integration tests without env-var writes
- OpenAPI / Swagger docs — `.openapi(OpenApiConfig)` generates `GET /openapi.json` + `GET /docs` (Swagger UI) from registered routes; `openapi` feature
- Per-route timeouts — `with_timeout`/`with_timeout_state`/`with_timeout_async` wrap a handler with its own deadline; `TimeoutLayer` + config-driven proxy's `timeout_ms`
- Request ID middleware — `RequestIdLayer` injects/echoes `X-Request-Id` on every request and response; `RequestId` extractor to read it

</details>

<details>
<summary><strong>Proxy & gateway</strong></summary>

- Config-driven proxy — `rws.config.toml` with `[[route]]` / `[[upstream]]`; per-route middleware including bearer/JWT/Basic auth (`auth` feature for JWT/Basic — no Rust code needed)
- Reverse proxy middleware — `ReverseProxy`; round-robin; `502` when all backends fail; built-in `ConnPool` reuses keep-alive TCP streams; SSE, chunked AI streams, and large downloads are streamed without buffering via `Response::stream_pipe`
- HTTP/2 reverse proxy — `H2ReverseProxy` (`h2://`, `h2s://`, `https://`); `GrpcProxy` wraps it for `Content-Type: application/grpc*` (`grpc://`, `grpcs://`); TLS upstreams via rustls + ALPN `h2`; async-native sync/async bridge works under any tokio runtime flavor, not just `multi_thread`
- L4 TCP proxy — `TcpProxy` bidirectional relay, any TCP protocol (databases, legacy HTTP)
- UDP proxy — `UdpProxy` datagram proxy; DNS / syslog style
- WebSocket proxy — `WsProxy` performs the HTTP upgrade and relays frames bidirectionally; `wss://` backends connect over TLS via rustls
- Health checks — per-upstream background checker; live backend list via `Arc<RwLock<Vec<String>>>`
- Canary / traffic splitting — `CanaryLayer` distributes requests by weight, lock-free; backends can be plain HTTP or TLS (`https://`/`h2s://`/`grpcs://`)
- Circuit breaker — Closed → Open → HalfOpen; `RetryLayer` retries on 502/503/504
- Service discovery — `Static`, `EnvPrefix`, `File`, `Dns` sources; background refresh thread
- Kubernetes Ingress — `KubernetesIngressWatcher` polls K8s API; routes to cluster services

</details>

<details>
<summary><strong>Security</strong></summary>

- Per-IP rate limiting — sliding-window `RateLimiter` + `RateLimitLayer`; hot-reloadable
- Distributed rate limiting — `RedisRateLimiter`, a fixed-window limiter backed by a Redis server (hand-rolled RESP client), for a shared budget across multiple `rws` instances behind a load balancer
- Max request body size — `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` rejects oversized bodies with `413` before buffering them, across HTTP/1.1, HTTP/2, and HTTP/3; `0` (default) is unlimited
- CORS — configurable origins, methods, headers; updated live via `SIGHUP`
- Auth — `BasicAuthLayer` (HTTP Basic), `JwtLayer` (HS256 Bearer), `ForwardAuthLayer` (delegate to an external auth service, Traefik/nginx `auth_request` style) (`auth` feature); `JwtLayer::rs256`/`::es256` (RS256/ES256 against a static public key, no JWKS needed) (`auth-asymmetric` feature)
- IP filter — `IpFilter::allow([...])` / `deny([...])`; exact IPv4 and CIDR ranges
- CSRF — double-submit cookie, `SameSite=Strict`, constant-time compare (`csrf` feature)
- Password hashing — Argon2id + CSPRNG token generation (`crypto` feature)
- Signed and encrypted cookies — `signed_cookie` (HMAC-SHA256, tamper-evident) and `encrypted_cookie` (AES-256-GCM, confidential) (`crypto` feature)
- OAuth2 / OIDC SSO — authorization-code + PKCE flow; RS256/ES256 JWT via JWKS; `OidcAuth` middleware; presets for Google, Microsoft, GitHub, Okta, Auth0, Keycloak; `from_env()`; `sso` feature
- Webhook signature verification — `verify_webhook_signature` for GitHub (`X-Hub-Signature-256`), Shopify (`X-Shopify-Hmac-Sha256`), and Stripe (`Stripe-Signature`, with replay-window tolerance) (`webhook` feature)
- Request / response rewriting — `RewriteLayer` rewrites headers, URI, status, body bytes; `.request_uri_regex_rewrite()` for nginx-style regex URI rewrites with capture-group expansion (`rewrite-regex` feature)

</details>

<details>
<summary><strong>Observability & ops</strong></summary>

- Prometheus metrics — `GET /metrics`; `MetricsLayer` adds per-route counters + histograms
- OpenTelemetry tracing — `OtelLayer`; W3C `traceparent`; stdout or OTLP (Jaeger, Tempo); nested child spans via `otel::span`/`otel::client_span`
- Access log — Combined Log Format or `RWS_CONFIG_LOG_FORMAT=json`
- Hot config reload — `SIGHUP` or `POST /admin/config/reload`; no restart required
- Graceful shutdown — SIGTERM drains connections; `/readyz` returns `503` during drain
- Background scheduler — fixed-rate, fixed-delay, 6-field cron; one thread per task
- Background job queue — `JobQueue` (in-memory) or `PersistentJobQueue` (crash-safe, model-backed); retry with exponential backoff; `jobs` feature
- Kubernetes-ready — `/healthz`, `/readyz`, `/metrics`; `0.0.0.0` default bind; Dockerfile included
- Compression — automatic gzip for text types; chunked streaming for files > 8 MB

</details>

<details>
<summary><strong>AI & MCP</strong></summary>

- MCP server — `McpServer` serves tools, resources, and prompts over MCP Streamable HTTP (`POST /mcp`); bearer token auth; connects to Claude, Cursor, and other MCP clients
- 8 built-in rws tools — `server_config`, `feature_flags`, `server_metrics`, `rate_limit_config`, `check_rate_limit`, `cors_config`, `list_static_files`, `reload_config`
- SSE streaming — `Sse` builder makes forwarding AI token streams to the browser trivial
- Response caching — `CacheLayer` TTL cache; vary-by-header; `Cache-Control` opt-out

</details>

<details>
<summary><strong>Database / ORM</strong></summary>

- `#[derive(Model)]` — maps structs to tables; async `Repository<T, i64>` for zero-boilerplate CRUD (all methods `.await`)
- `QueryBuilder<T>` — `.where_eq()`, `.order_by()`, `.limit()`, `.fetch_all().await`, `.count().await`
- Pagination — `.paginate(page, per_page)` → `Page<T>` (with `total_pages`) or `.paginate_after(cursor, per_page)` → `CursorPage<T>` (keyset, for large tables); both build an RFC 8288 `Link` response header
- Migrations — `pool.migrate("migrations/").await` runs `*.sql` files in lexicographic order, idempotent; `pool.rollback_last()` / `pool.rollback(dir, n)` undo them via companion `.down.sql` files
- Relations — `HasMany<T>`, `HasOne<O>`, `BelongsTo<O>`; explicit async load, no hidden N+1
- Backends — SQLite (`model-sqlite`), PostgreSQL (`model-postgres`), MySQL (`model-mysql`); all imply `http2` (tokio runtime); not mutually exclusive — enable more than one to hold a `DbPool` to each backend in the same binary (`Backend` selects which)
- Backed by `sqlx` — async-native driver; `DbPool` is a cheap-to-clone `Arc`-wrapped `sqlx::Pool`
- In-memory SQLite — `DbPool::memory().await` for isolated per-test databases

</details>

<details>
<summary><strong>File / object storage</strong></summary>

- `Storage` trait — `put`/`get`/`delete`/`url`; write handler code once, swap backends
- `LocalStorage` — stores files on local disk; `storage-local` feature, no new deps
- `S3Storage` — AWS S3, Cloudflare R2, MinIO via AWS Signature V4 over the outbound HTTP client; no AWS SDK; `storage-s3` feature
- Workload identity — EKS IRSA, ECS task roles, EC2 IMDSv2 auto-detected when static keys aren't set; no static keys required in cloud deployments
- `AzureBlobStorage` — Azure Blob Storage via Shared Key HMAC signing over the outbound HTTP client, or auto-detected Managed Identity (App Service/Container Apps, VM/AKS IMDS); no Azure SDK; `storage-azure` feature

</details>

---

## Optional features

| Feature | What it adds |
|---------|--------------|
| `http-client` | HTTPS for outbound `Client` — adds `rustls` + `webpki-roots` |
| `serde` | `Json<T>` extractor and responder via `serde_json` |
| `auth` | `BasicAuthLayer` (HTTP Basic, plus `from_htpasswd_file`), `JwtLayer` (HS256), and `ForwardAuthLayer` (delegates auth decisions to an external HTTP service); also wires `type = "jwt"`/`"basic"` in the config-driven proxy's `[route.middleware.auth]` |
| `auth-asymmetric` | `JwtLayer::rs256`/`::es256` — verify RS256/ES256 JWTs against a static RSA/P-256 public key (PEM), no JWKS endpoint or full `sso` feature required; implies `auth` |
| `macros` | `#[get]`, `#[post]`, …, `#[derive(FromRequest)]`, `#[derive(Validate)]`, `#[derive(Config)]` |
| `acme` | Automatic TLS via Let's Encrypt (ACME RFC 8555); implies `http2` |
| `tera` | Tera HTML template engine (Jinja2/Django syntax) |
| `model-sqlite` | Async ORM backed by SQLite (via `sqlx`); implies `http2`. Combinable with the other two `model-*` features — see `Backend` |
| `model-postgres` | Async ORM backed by PostgreSQL (via `sqlx`); implies `http2`. Combinable with the other two `model-*` features — see `Backend` |
| `model-mysql` | Async ORM backed by MySQL (via `sqlx`); implies `http2`. Combinable with the other two `model-*` features — see `Backend` |
| `crypto` | Argon2id password hashing + CSPRNG token generation; `signed_cookie`/`verify_signed_cookie` (HMAC-SHA256) and `encrypted_cookie`/`decrypt_cookie` (AES-256-GCM) in `cookie` |
| `csrf` | Double-submit cookie CSRF protection |
| `sso` | OAuth2/OIDC SSO — `OidcAuth` middleware, RS256/ES256 JWT via JWKS, PKCE, provider presets (Google · Microsoft · GitHub · Okta · Auth0 · Keycloak) |
| `mailer` | SMTP email — `Mailer::from_env()` + `Email::builder()`; plain, STARTTLS, and SMTPS; multipart text+HTML; AUTH PLAIN; no third-party mail library (STARTTLS/SMTPS additionally require `http-client` or `http2`) |
| `jobs` | `JobQueue` — in-memory background job queue with retry + exponential backoff. `PersistentJobQueue` (additionally requires a `model-*` feature) persists jobs to survive a crash/restart. |
| `storage-local` | `LocalStorage` — file storage on local disk; no new deps |
| `storage-s3` | `S3Storage` — S3-compatible object storage (AWS S3, R2, MinIO); AWS SigV4 signing via `hmac` + `sha2`; static keys or auto-detected workload identity (EKS IRSA / ECS task role / EC2 IMDSv2), no AWS SDK |
| `storage-azure` | `AzureBlobStorage` — Azure Blob Storage; Shared Key HMAC-SHA256 signing via `hmac` + `sha2`; static account key or auto-detected Managed Identity (App Service/Container Apps identity endpoint / VM/AKS IMDS), no Azure SDK |
| `openapi` | `AppWithState`/`AsyncAppWithState::openapi(config)` — generates `GET /openapi.json` + `GET /docs` (Swagger UI) from registered routes; no new deps |
| `webhook` | `verify_webhook_signature` — HMAC signature verification for GitHub, Shopify, and Stripe webhooks; `hmac` + `sha2`, no new deps beyond what `auth`/`crypto` already use |
| `rewrite-regex` | `RewriteLayer::request_uri_regex_rewrite` — regex URI rewriting with capture-group expansion, nginx `rewrite`-directive style; adds `regex` |

```toml
[dependencies]
rust-web-server = { version = "17", features = ["serde", "auth", "macros"] }
```

---

## Build from source

```bash
git clone https://github.com/bohdaq/rust-web-server.git
cd rust-web-server
cargo build --release
```

| Build | Flags | Approx. size |
|-------|-------|-------------|
| HTTP/3 + HTTP/2 + HTTP/1.1 + TLS | _(default)_ | ~12 MB |
| HTTP/2 + HTTP/1.1 + TLS | `--no-default-features --features http2` | ~8 MB |
| HTTP/1.1 only, no TLS | `--no-default-features --features http1` | ~3 MB |

Binary is at `target/release/rws`. MSRV is 1.75.

---

## Further reading

- [CONFIGURE](CONFIGURE.md) — all configuration options (env vars, config file, CLI flags)
- [DEVELOPER](DEVELOPER.md) — building blocks reference and 72 use-case examples
- [FAQ](FAQ.md) — common problems and solutions
- [spec/PROXY_SERVER_CONFIG.md](spec/PROXY_SERVER_CONFIG.md) — annotated proxy config reference
- [spec/AI_ADOPTION.md](spec/AI_ADOPTION.md) — AI adoption strategy
- [docs.rs/rust-web-server](https://docs.rs/rust-web-server) — API reference

## License

MIT
