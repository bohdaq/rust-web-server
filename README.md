# rws

[![CI](https://github.com/bohdaq/rust-web-server/actions/workflows/ci.yml/badge.svg)](https://github.com/bohdaq/rust-web-server/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rust-web-server.svg)](https://crates.io/crates/rust-web-server)
[![docs.rs](https://docs.rs/rust-web-server/badge.svg)](https://docs.rs/rust-web-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![MSRV: 1.88](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://www.rust-lang.org)

**A single-binary web server and reverse proxy, written in Rust.** Automatic HTTPS from Let's Encrypt, HTTP/3, load balancing with health checks, auth and rate limiting — configured in one TOML file, no code required. When you outgrow configuration, the same engine is a Rust library.

**Website:** [rws8.tech](https://rws8.tech/)

```bash
brew install bohdaq/tap/rws          # macOS / Linux — or: cargo binstall rust-web-server
docker run -p 7878:7878 -v "$PWD:/srv" bohdaq/rws
```

Prebuilt binaries for Linux (static, any distro), macOS, and Windows are on [GitHub Releases](https://github.com/bohdaq/rust-web-server/releases/latest).

---

## Serve a folder

```bash
cd my-site && rws
```

Serves the current directory on `http://localhost:7878` — `index.html`, directory listings, ETags and browser caching (`304` revalidation), gzip, byte ranges (video seeking, resumable downloads), and an optional SPA fallback for client-side routers. `rws --help` lists every option.

## Automatic HTTPS

```toml
# rws.config.toml
port               = 443
http_redirect_port = 80

[acme]
domains = ["example.com", "www.example.com"]
email   = "you@example.com"
```

`rws` gets certificates from Let's Encrypt on first start, serves HTTPS with HTTP/2 and HTTP/3, redirects HTTP to HTTPS, and renews in the background with zero-downtime reloads. Retries until DNS is ready, IPv4 and IPv6, private ACME CAs supported. → [Automatic HTTPS](https://rws8.tech/features/acme/)

## Reverse proxy

```toml
[[upstream]]
name     = "app"
backends = ["10.0.0.10:8080", "10.0.0.11:8080"]
strategy = "least_connections"

  [upstream.health_check]
  path = "/healthz"

[[route]]
  [route.match]
  host = "api.example.com"
  path = "/v1/*"

  [route.action]
  type     = "proxy"
  upstream = "app"

  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  auth       = { type = "bearer", token_env = "API_TOKEN" }
```

Load balancing (round-robin, least-connections, random, IP hash), active health checks, bearer / JWT / basic auth, rate limits, caching, IP allow/deny lists, header and URL rewrites, per-route timeouts and body limits — plus TCP, UDP, WebSocket, and gRPC proxying. Misconfigured auth fails closed. Circuit breakers, canary splits, forward auth, and service discovery are available from the library. Combine with `[acme]` for HTTPS in front of your apps. → [Config-driven proxy](https://rws8.tech/proxy/config-driven/) · [full config reference](https://rws8.tech/configuration/config-file/)

---

## Need custom logic? It's also a Rust library

The server above is the `rust-web-server` crate. Use it directly when you need routes, middleware, or an MCP server in code:

```toml
# Cargo.toml
[dependencies]
rust-web-server = "17"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use rust_web_server::prelude::*;

fn hello(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
    Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "Hello, world!")
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

Routing with path parameters, typed extractors, one `Middleware` trait for everything (auth, rate limiting, caching, tracing, the reverse proxy itself), WebSocket, SSE, an MCP server, an async ORM, background jobs, object storage, a mailer, and OAuth/OIDC/SAML login. See [DEVELOPER.md](DEVELOPER.md) for 89 runnable use cases.

## Why rws

- **One binary, one file.** Static site, HTTPS, reverse proxy, and gateway features in a single executable configured by one `rws.config.toml` — no separate certbot, cron job, or proxy process.
- **No hyper, no Tower, no Axum.** HTTP/1.1 parsing, routing, middleware, WebSocket, SSE, CORS, range requests, JWT, and the ACME client are implemented in this crate. HTTP/2 and HTTP/3 framing use the widely deployed `h2` and `quinn`/`h3` crates, and TLS uses `rustls` — no OpenSSL.
- **Config and code share one engine.** Everything the TOML file does is a type you can use from Rust, so moving from configuration to code doesn't mean switching tools.
- **Tested against the real world.** CI fuzzes every parser that sees untrusted input, checks HTTP/2 conformance with h2spec, and runs the ACME flow end to end against Let's Encrypt's test CA on every push. Report vulnerabilities privately — see [SECURITY.md](SECURITY.md).
- **Fast.** 2.2–3.9× faster than Caddy, close to nginx, and the smallest memory footprint of the three — see [Performance](#performance).
- **SemVer since v1.** Frequent releases (currently v17) are additive; see [releases](https://github.com/bohdaq/rust-web-server/releases) for the changelog.

## Performance

Requests per second (p99 latency), on one 4-CPU GitHub Actions runner with
the load generator on the same machine: `oha`, 64 keep-alive connections,
20 s per scenario, no compression, access logs off; nginx with
`open_file_cache`.

| Scenario | rws | Caddy 2.11 | nginx |
|---|---:|---:|---:|
| Static file, 1 KiB | 141,677 (**1.1 ms**) | 38,812 (7.2 ms) | **145,701** (3.2 ms) |
| Static file, 1 MiB | 14,657 (**9.7 ms**) | 3,753 (82 ms) | **15,180** (10.2 ms) |
| Reverse proxy, 1 KiB | 58,610 (**2.7 ms**) | 26,966 (8.5 ms) | **61,521** (3.0 ms) |
| CPU per request (1 KiB / 1 MiB / proxy) | **11.0** / 146 / 26.4 µs | 70.4 / 699 / 89.8 µs | 13.2 / **140** / **25.4** µs |
| Peak memory | **9–14 MiB** | 67–76 MiB | 24–25 MiB |

`rws` is 2.2–3.9× faster than Caddy and within 3–5% of nginx, with the
lowest p99 of the three in every scenario and the smallest memory
footprint. On small files it needs 17% less CPU per request than nginx; on
large files and proxying 4% more. Shared CI runners are noisy, so
compare servers within a run, not across runs. Reproduce it with
[`bench/run.sh`](bench/run.sh) or the manual
[Benchmark workflow](.github/workflows/bench.yml).

## Contents

- [Performance](#performance)
- [Building apps with AI](#building-apps-with-ai)
- [What's in the box](#whats-in-the-box)
- [Optional features](#optional-features)
- [Build from source](#build-from-source)
- [Further reading](#further-reading)

---

## Building apps with AI

`rws` is written to be easy for AI coding assistants (Claude Code, Cursor, GitHub Copilot, ChatGPT, etc.) to generate correct code against on the first try — one consistent `Router`/`AppWithState` routing pattern, one `Middleware` trait for auth/rate-limiting/caching/everything else, and no hyper/Tower/Axum types whose APIs a model might confuse with this crate's own.

Three files make that possible — point your AI tool at them before asking it to build something:

- **[llms.txt](llms.txt)** — a flat, LLM-optimized reference: every public type, middleware, and feature flag, with a runnable snippet for nearly every capability in the crate. This is the file to paste into a chat or system prompt, or fetch directly: `https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms.txt`.
- **[DEVELOPER.md](DEVELOPER.md)** — 89 numbered, runnable use cases (`### N. Title`). Ask your assistant to follow the closest-matching use case instead of inventing a pattern from scratch.
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
- Automatic HTTPS (ACME) — Let's Encrypt certificates from an `[acme]` section in `rws.config.toml`, background renewal with hot reload, private ACME CAs (`acme` feature; included in the prebuilt binaries)
- mTLS — set `RWS_CONFIG_TLS_CLIENT_CA_FILE`; client cert required on HTTPS and QUIC
- Virtual hosting / SNI — per-domain TLS certs; `Router::with_host()` for per-host routing
- WebSocket (RFC 6455) — handshake, frame codec, SHA-1 + base64 built in, no extra dep
- Server-Sent Events — `Sse` builder with correct headers; ideal for AI token streaming
- Outbound HTTP client — `Client` (sync) and `AsyncClient` (async, `http2` feature); HTTPS via rustls; `.form(&[(k,v)])` for `application/x-www-form-urlencoded` bodies (OAuth token/JWKS calls, etc.)

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
- One-line responses — `Response::json(status, bytes)` / `Response::text(status, &str)`, no feature flag required
- HTML templates — Tera engine (Jinja2 syntax); `template::render()` one-liner; `template::reload()` hot-reloads edited templates from disk without a restart, wired into the same `SIGHUP` hook as CORS/rate-limit/TLS reload (`tera` feature)
- Dependency injection — `Container` keyed by `TypeId`; concrete types and `dyn Trait`
- In-process test client — `TestClient::new(app)` dispatches without a TCP socket
- Per-instance typed config — `ServerConfig` struct; `App::with_config(config)`, `AppWithState::with_config`, `AsyncAppWithState::with_config`, and `ConfigDrivenApp::with_config` all pin an app to explicit settings for parallel-safe integration tests without env-var writes
- OpenAPI / Swagger docs — `.openapi(OpenApiConfig)` generates `GET /openapi.json` + `GET /docs` (Swagger UI) from registered routes; `openapi` feature
- Per-route timeouts — `with_timeout`/`with_timeout_state`/`with_timeout_async` wrap a handler with its own deadline; `TimeoutLayer` + config-driven proxy's `timeout_ms`
- Per-route max body size — config-driven proxy's `[route.middleware] max_body_size` returns `413` for one route's oversized requests, stricter than the global `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`
- Request ID middleware — `RequestIdLayer` injects/echoes `X-Request-Id` on every request and response; `RequestId` extractor to read it
- Directory listing — any requested directory without an `index.html` renders a dark/light-adaptive listing page (breadcrumb, parent link, sortable-by-eye table, client-side filter) instead of `404`; default, always on; CSS/JS served same-origin at `/rws-directory-listing.css`/`.js` (CSP-compliant, no inline `<style>`/`<script>`), overridable by dropping a same-named file on disk
- SPA fallback — `RWS_CONFIG_SPA_FALLBACK=index.html` serves a fallback file for unmatched `GET`/`HEAD` routes instead of `404`, for React Router/Vue Router/etc. deep links; opt-in (unset by default); skips paths that look like a missed asset (a `.` in the last segment) and any `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`

</details>

<details>
<summary><strong>Proxy & gateway</strong></summary>

- Config-driven proxy — `rws.config.toml` with `[[route]]` / `[[upstream]]`; per-route middleware including bearer/JWT/Basic auth (`auth` feature for JWT/Basic — no Rust code needed)
- Reverse proxy middleware — `ReverseProxy`; round-robin; `502` when all backends fail; built-in `ConnPool` reuses keep-alive connections, plain and TLS (`https://` backends pay one handshake per connection, not per request); private-CA backends via `tls_ca_file`; SSE, chunked AI streams, and large downloads are streamed without buffering via `Response::stream_pipe`
- HTTP/2 reverse proxy — `H2ReverseProxy` (`h2://`, `h2s://`, `https://`); `GrpcProxy` wraps it for `Content-Type: application/grpc*` (`grpc://`, `grpcs://`); TLS upstreams via rustls + ALPN `h2`; async-native sync/async bridge works under any tokio runtime flavor, not just `multi_thread`
- L4 TCP proxy — `TcpProxy` bidirectional relay, any TCP protocol (databases, legacy HTTP)
- UDP proxy — `UdpProxy` datagram proxy; DNS / syslog style
- WebSocket proxy — `WsProxy` performs the HTTP upgrade and relays frames bidirectionally; `wss://` backends connect over TLS via rustls
- Health checks — per-upstream background checker; live backend list via `Arc<RwLock<Vec<String>>>`; `[ws_proxy.health_check]` applies the same checker to `ws://`/`wss://` proxy backends (`503` if all are unhealthy); `WsProxy::with_live_backends()` for library use outside the config file
- Canary / traffic splitting — `CanaryLayer` distributes requests by weight using smooth weighted round-robin (no bursts for skewed weights); backends can be plain HTTP or TLS (`https://`/`h2s://`/`grpcs://`); `.update()` changes weights live with no restart; `.add_pool()`/`WeightedPool` sources a group from a `BackendPool` instead of a fixed URL
- Circuit breaker — Closed → Open → HalfOpen with a concurrency cap on HalfOpen probes; `RetryLayer` retries on 502/503/504; `RedisCircuitBreaker` persists state across restarts and shares it across `rws` instances (hand-rolled RESP client); `ReverseProxy::with_circuit_breaker(breaker)` auto-wires either kind — no manual `is_available`/`record_success`/`record_failure` calls; state exposed via `rws_circuit_breaker_state{backend}` on `/metrics`
- Service discovery — `Static`, `EnvPrefix`, `File`, `Dns`, `DnsSrv` (weight-expanded SRV lookup), `Consul` (agent health-check endpoint), `Docker` (label-value-as-address, Unix-only) sources; background refresh thread; `EtcdWatch` gets a dedicated live watch stream instead of polling
- Kubernetes Ingress — `KubernetesIngressWatcher` resyncs + watches the K8s API (low-latency updates); `pathType` Exact/Prefix, `ingressClassName` filtering; `.from_service_account()` connects in-cluster over TLS (`http-client`/`http2`); routes to cluster services

</details>

<details>
<summary><strong>Security</strong></summary>

- Per-IP rate limiting — sliding-window `RateLimiter` + `RateLimitLayer`; hot-reloadable; `429` responses carry `Retry-After`
- Distributed rate limiting — `RedisRateLimiter`, a fixed-window limiter backed by a Redis server (hand-rolled RESP client), for a shared budget across multiple `rws` instances behind a load balancer
- Max request body size — `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` rejects oversized bodies with `413` before buffering them, across HTTP/1.1, HTTP/2, and HTTP/3; `0` (default) is unlimited
- HTTP/1.1 request framing — request heads up to 64 KiB across any number of reads (`431` beyond) and within 30 s of their first byte (`408` beyond — a client trickling bytes can't hold a connection), pipelined requests, chunked request bodies; a request with both `Content-Length` and `Transfer-Encoding` or conflicting lengths is rejected (`400`) so a proxy in front can't be desynced (request smuggling)
- `Expect: 100-continue` (HTTP/1.1) — sends the `100 Continue` interim response before reading the body, so large uploads aren't sent needlessly ahead of a `413`/`417` rejection
- CORS — configurable origins, methods, headers; updated live via `SIGHUP`
- Auth — `BasicAuthLayer` (HTTP Basic), `JwtLayer` (HS256 Bearer), `ForwardAuthLayer` (delegate to an external auth service, Traefik/nginx `auth_request` style) (`auth` feature); `JwtLayer::rs256`/`::es256` (RS256/ES256 against a static public key, no JWKS needed; RSA verified in-crate, no `rsa` crate) (`auth-asymmetric` feature)
- IP filter — `IpFilter::allow([...])` / `deny([...])`; exact IPv4/IPv6 addresses and CIDR ranges
- CSRF — double-submit cookie, `SameSite=Strict`, constant-time compare (`csrf` feature)
- Password hashing — Argon2id + CSPRNG token generation (`crypto` feature)
- Signed and encrypted cookies — `signed_cookie` (HMAC-SHA256, tamper-evident) and `encrypted_cookie` (AES-256-GCM, confidential) (`crypto` feature)
- OAuth2 / OIDC SSO — authorization-code + PKCE flow; RS256/ES256 JWT via JWKS; `OidcAuth` middleware; presets for Google, Microsoft, GitHub, Okta, Auth0, Keycloak; `from_env()`; `sso` feature
- OAuth 2.0 Authorization Server — `AuthServer` issues its own HS256 JWTs (`client_credentials`, `authorization_code` + PKCE, `refresh_token` grants); `ClientStore` registers clients; `sso-server` feature
- SAML 2.0 SSO — `SamlSp` Service Provider middleware; ACS handler; RSA-SHA256 XML signature verification; `AttributeMap` for IdP attribute translation; `sso-saml` feature
- Webhook signature verification — `verify_webhook_signature` for GitHub (`X-Hub-Signature-256`), Shopify (`X-Shopify-Hmac-Sha256`), and Stripe (`Stripe-Signature`, with replay-window tolerance) (`webhook` feature)
- Request / response rewriting — `RewriteLayer` rewrites headers, URI, status, body bytes; `.request_uri_regex_rewrite()` for nginx-style regex URI rewrites with capture-group expansion (`rewrite-regex` feature)

</details>

<details>
<summary><strong>Observability & ops</strong></summary>

- Prometheus metrics — `GET /metrics`; `MetricsLayer` adds per-route counters + histograms
- OpenTelemetry tracing — `OtelLayer`; W3C `traceparent`; stdout or OTLP (Jaeger, Tempo); nested child spans via `otel::span`/`otel::client_span`
- Access log — structured JSON (default) or Combined Log Format (`RWS_CONFIG_LOG_FORMAT=combined`; `off` disables it); buffered per thread, flushed every 100 ms
- Hot config reload — `SIGHUP` or `POST /admin/config/reload`; no restart required. In proxy mode routes, upstreams and `[middleware]` reload too — validated first, a file with errors leaves the running routes serving
- `rws --check` — validates `rws.config.toml` (unknown keys, bad values, TLS files, undeclared upstreams, unset auth secrets, duplicate virtual hosts, CORS origins that can never match, a missing SPA fallback file) and exits 1 on errors, like `nginx -t`; `--format=json` for CI and editors
- Graceful shutdown — SIGTERM drains connections; `/readyz` returns `503` during drain
- Background scheduler — fixed-rate, fixed-delay, 6-field cron; one thread per task
- Background job queue — `JobQueue` (in-memory) or `PersistentJobQueue` (crash-safe, model-backed); retry with exponential backoff; `jobs` feature
- Kubernetes-ready — `/healthz`, `/readyz`, `/metrics`; `0.0.0.0` default bind; Dockerfile included; parameterized Helm chart at [`helm/rws/`](helm/rws/)
- Non-blocking proxying — config-file proxy routes without middleware wait for the backend (HTTP or HTTPS) on the connection's task, holding no thread (`Application::execute_async`)
- Fast static files — open files reused for a second (like nginx's `open_file_cache`; `open_file_cache_secs`), `sendfile` (zero-copy) for downloads over plain HTTP on Linux and macOS, and quick requests (static files, health checks) served on the event loop without a thread handoff
- Compression — automatic gzip for text types of 512 bytes or more; skipped when it wouldn't shrink the body or the backend already encoded it; static files up to 256 KiB are compressed once per version and served from cache; chunked streaming for large files
- WASM guest component — `rws-wasm-shim` runs `App`/`Router`/middleware inside a `wasi:http/proxy` component (Wasmtime, Spin, Fastly Compute); the host owns the socket and TLS, calling the guest once per request; outbound HTTP/HTTPS, streaming file downloads, `sso`/`secrets`/`storage-s3`/`storage-azure`/`ForwardAuthLayer` all verified working; see `spec/WASM_SHIM.md`

</details>

<details>
<summary><strong>AI & MCP</strong></summary>

- MCP server — `McpServer` serves tools, resources, and prompts over MCP Streamable HTTP (`POST /mcp`); bearer token auth; connects to Claude, Cursor, and other MCP clients; `initialize` negotiates the protocol version down to whichever of client/server is lower instead of always claiming its own
- Per-request tool context — `.tool_with_context(...)` gives a tool handler an `McpContext` (caller's `clientInfo`, `Mcp-Session-Id`) tracked across a session from `initialize` to later `tools/call`s
- Tool annotations (MCP 2025-03-26) — `.tool_annotated(...)` attaches `ToolAnnotations` (`readOnlyHint`/`destructiveHint`/`idempotentHint`/`openWorldHint`) to a tool, surfaced in `tools/list` so clients like Claude Desktop can decide whether to warn or ask for confirmation before calling it
- Image and embedded-resource content — `McpContent::image(data, mime_type)` and `McpContent::embedded(uri, text, mime_type)` cover the MCP spec's `image` and `resource` content types alongside the original `text`/`json`
- JSON-RPC batch requests — `POST /mcp` with a top-level JSON array dispatches each element independently and returns one combined `[...]` response array, per element success/error preserved, notifications omitted
- Pagination for list methods — `.page_size(n)` caps `tools/list`/`resources/list`/`prompts/list` to `n` items per response with an opaque `nextCursor`/`params.cursor` for the next page; unset means every item comes back in one response
- SSE streaming transport (`GET /mcp`) — a long-lived `text/event-stream` connection for server → client push; `.notify(method, params)` broadcasts a JSON-RPC notification to every connected client from anywhere in your code (HTTP/1.1 only)
- `logging/setLevel` + `notifications/message` — `.logging_enabled()` advertises the logging capability; `.log(level, logger, data)` pushes RFC 5424-leveled log entries over the SSE stream, filtered by whatever minimum level the client last requested
- Dynamic tool/resource/prompt registration — `.register_tool(...)`/`.register_resource(...)`/`.register_prompt(...)` and matching `.remove_*(...)` methods add or remove entries at runtime from any thread, pushing `notifications/{tools,resources,prompts}/list_changed` over SSE
- `notifications/progress` — `ctx.report_progress(progress, total, message)` in a `.tool_with_context(...)` handler pushes progress updates over SSE for a `tools/call` that included `params._meta.progressToken`
- `completion/complete` — `.completion(ref_type, ref_name, handler)` registers an argument-autocomplete provider for a tool or prompt, so clients like Cursor and VS Code can offer suggestions while the user types
- Request cancellation — `ctx.is_cancelled()` in a `.tool_with_context(...)` handler cooperatively checks whether the client sent `notifications/cancelled` for this call, so a long-running handler can stop early between its own steps
- `resources/subscribe` / `resources/unsubscribe` — `.notify_resource_updated(uri)` pushes `notifications/resources/updated` over SSE to just the sessions subscribed to that URI, for live-updating resource panels (e.g. Claude Desktop watching a config file)
- `sampling/createMessage` (server-side sampling) — `ctx.sample(request, timeout)` in a `.tool_with_context(...)` handler asks the connected client to run LLM inference and blocks for its answer, reversing the usual request direction
- `roots/list` + `notifications/roots/list_changed` — `ctx.list_roots(timeout)` asks the client which filesystem roots it has access to, cached per session until the client signals a change
- Async tool handlers (`http2` feature) — `.async_tool(...)`/`.register_async_tool(...)` register a tool whose handler is `async fn`, bridged into `tools/call` via the same `block_on_isolated` mechanism as `H2ReverseProxy`/`AsyncAppWithState`
- OAuth 2.0 authorization (MCP 2025-03-26, `sso` feature) — `.require_oauth(provider, audience)` verifies a client's bearer JWT via a live JWKS endpoint (reusing `sso::JwksCache`) instead of one static shared secret; serves `GET /.well-known/oauth-authorization-server`; verified claims land in `McpContext.auth_claims`
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

<details>
<summary><strong>Secrets manager integration</strong></summary>

- `secrets::resolve` — resolves a secret-reference string (`vault://path#field`, `aws-sm://name[#field]`, `azkv://vault-name/secret-name`) to its live value over HTTP(S); anything else passes through unchanged; `secrets` feature
- HashiCorp Vault — KV v2 HTTP API, `VAULT_ADDR`/`VAULT_TOKEN`
- AWS Secrets Manager — reuses `storage-s3`'s SigV4 signer and IRSA/ECS/IMDSv2 credential chain, no separate AWS auth story
- Azure Key Vault — service-principal client-credentials grant, or the same Managed Identity auto-detection `AzureBlobStorage` uses
- `resolve_env_vars()` rewrites every `RWS_`-prefixed env var whose value matches a secret-reference prefix, in place — wired into `Server::setup()` automatically, so any `RWS_CONFIG_*` value can be a secret reference with no code changes
- Fails fast on a broken reference — a startup error, not a server silently running with a literal `"vault://..."` string as its JWT key

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
| `sso-server` | `AuthServer` — `rws` as its own OAuth 2.0 Authorization Server (HS256 token issuer); implies `sso` and `auth` |
| `sso-saml` | `SamlSp` — SAML 2.0 Service Provider; ACS handler; RSA-SHA256 XML signature verification; implies `sso` |
| `mailer` | SMTP email — `Mailer::from_env()` + `Email::builder()`; plain, STARTTLS, and SMTPS; multipart text+HTML; AUTH PLAIN; no third-party mail library (STARTTLS/SMTPS additionally require `http-client` or `http2`) |
| `jobs` | `JobQueue` — in-memory background job queue with retry + exponential backoff. `PersistentJobQueue` (additionally requires a `model-*` feature) persists jobs to survive a crash/restart. |
| `storage-local` | `LocalStorage` — file storage on local disk; no new deps |
| `storage-s3` | `S3Storage` — S3-compatible object storage (AWS S3, R2, MinIO); AWS SigV4 signing via `hmac` + `sha2`; static keys or auto-detected workload identity (EKS IRSA / ECS task role / EC2 IMDSv2), no AWS SDK |
| `storage-azure` | `AzureBlobStorage` — Azure Blob Storage; Shared Key HMAC-SHA256 signing via `hmac` + `sha2`; static account key or auto-detected Managed Identity (App Service/Container Apps identity endpoint / VM/AKS IMDS), no Azure SDK |
| `secrets` | `secrets::resolve` — resolves `vault://`/`aws-sm://`/`azkv://` secret references (HashiCorp Vault, AWS Secrets Manager, Azure Key Vault) at startup; reuses `storage-s3`'s SigV4 signer and `storage-azure`'s Managed Identity token fetch; no vendor SDK |
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

Binary is at `target/release/rws`. MSRV is 1.88.

---

## Further reading

- [CONFIGURE](CONFIGURE.md) — all configuration options (env vars, config file, CLI flags)
- [DEVELOPER](DEVELOPER.md) — building blocks reference and 89 use-case examples
- [FAQ](FAQ.md) — common problems and solutions
- [spec/PROXY_SERVER_CONFIG.md](spec/PROXY_SERVER_CONFIG.md) — annotated proxy config reference
- [spec/AI_ADOPTION.md](spec/AI_ADOPTION.md) — AI adoption strategy
- [docs.rs/rust-web-server](https://docs.rs/rust-web-server) — API reference

## Contributing

Issues labelled [good first issue](https://github.com/bohdaq/rust-web-server/labels/good%20first%20issue) are a good place to start; questions go to [Discussions](https://github.com/bohdaq/rust-web-server/discussions). See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test and send a change.

## License

MIT
