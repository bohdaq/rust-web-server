# Documentation Site Plan

## Technology: Astro + Starlight

Starlight is purpose-built for documentation, outputs pure static HTML (philosophically aligned with a
zero-dependency Rust server), ships with dark mode by default, Expressive Code syntax highlighting,
Pagefind full-text search, and a sidebar. Used by projects like Astro itself, Biome, and Tauri — the
look is exactly "cutting edge" for a developer tool in 2026.

---

## Structure

```
docs/
├── astro.config.mjs
├── package.json
├── public/
│   └── logo.svg
└── src/
    ├── content/
    │   └── docs/
    │       ├── index.mdx               ← Landing page (hero + feature grid)
    │       ├── getting-started/
    │       │   ├── installation.md
    │       │   ├── quick-start.md
    │       │   └── features.md
    │       ├── configuration/
    │       │   ├── overview.md
    │       │   ├── env-vars.md
    │       │   ├── config-file.md
    │       │   └── cli-args.md
    │       ├── building-apps/
    │       │   ├── overview.md
    │       │   ├── controllers.md
    │       │   ├── routing.md
    │       │   ├── request-response.md
    │       │   ├── extractors.md
    │       │   ├── error-handling.md
    │       │   ├── middleware.md
    │       │   ├── validation.md
    │       │   ├── forms-uploads.md
    │       │   ├── json.md
    │       │   ├── cookies.md
    │       │   └── async-handlers.md
    │       ├── features/
    │       │   ├── cors-security.md
    │       │   ├── rate-limiting.md
    │       │   ├── compression.md
    │       │   ├── https-tls.md
    │       │   ├── http2.md
    │       │   ├── http3-quic.md
    │       │   ├── websocket.md
    │       │   ├── sse.md
    │       │   ├── auth.md
    │       │   ├── sessions.md
    │       │   ├── reverse-proxy.md
    │       │   ├── caching.md
    │       │   ├── metrics.md
    │       │   └── hot-reload.md
    │       ├── testing/
    │       │   └── test-client.md
    │       ├── deployment/
    │       │   ├── docker.md
    │       │   ├── kubernetes.md
    │       │   └── observability.md
    │       └── reference/
    │           ├── api.md
    │           └── roadmap.md
    └── styles/
        └── custom.css                  ← Brand accent color, font overrides
```

---

## Page-by-Page Content Plan

### Landing Page (`index.mdx`)

- Hero: tagline, 1-line install snippet, `cargo run` output in a terminal animation
- Protocol matrix badge row: HTTP/1.1 · HTTP/2 · HTTP/3/QUIC · TLS (rustls) · CORS · Gzip · ETag
- Feature grid (8 cards): Zero dependencies · Thread-pool HTTP/1.1 · Middleware pipeline · Kubernetes-ready · Prometheus metrics · WebSocket & SSE · In-process test client · No OpenSSL
- "Get started in 60 seconds" code block
- Link to Quick Start

---

### Getting Started

| Page | Key content |
|------|-------------|
| **Installation** | `cargo add rust-web-server`, build from source, feature flags table (`http1` / `http2` / `http3`), binary sizes, MSRV 1.75 |
| **Quick Start** | Run plain HTTP; serve static files; write and wire your first `Controller` in ~20 lines; verify with `curl` |
| **Features** | Full feature checklist table with checkmarks; "Coming Soon" callout for MCP server |

---

### Configuration

| Page | Key content |
|------|-------------|
| **Overview** | 4-layer priority diagram: Defaults → Env vars → `rws.config.toml` → CLI args |
| **Environment Variables** | Full table of all `RWS_CONFIG_*` vars, types, defaults |
| **Config File** | Annotated `rws.config.toml` with all keys and notes |
| **CLI Args** | All flags (`--ip`, `--port`, `--thread-count`, `--tls-cert-file`, `--tls-key-file`, `--cors-*`, `--request-allocation-size-in-bytes`) with examples |

---

### Building Apps

| Page | Key content |
|------|-------------|
| **Overview** | Mental model: `Controller` trait → `App::execute()` dispatch chain → `Response`. Diagram of request lifecycle from `main.rs` to TCP write. |
| **Controllers** | `Controller` trait (`is_matching` + `process`); annotated minimal example; adding to `App::execute()`; built-in controllers list |
| **Routing** | Static URI matching in `is_matching`; dynamic `Router` with `:param` and `*wildcard`; `PathParams::get()`; method guards; `routes!` declarative macro; `#[get]`, `#[post]`, etc. attribute macros |
| **Request & Response** | `Request` struct fields; `Response` builder; `Header` constants (50+); status code constants; content type via `MimeType`; `ContentRange` / `Range::get_content_range()` |
| **Typed Extractors** | `FromRequest` trait; `Body`, `BodyText`, `Query`, `RequestHeaders`; `#[derive(FromRequest)]` for named-field structs; implementing a custom extractor |
| **Error Handling** | `AppError` enum variants → HTTP status codes; `IntoResponse` trait; typed errors in controllers |
| **Middleware** | `Middleware` trait (`handle`); `WithMiddleware` / `App::new().wrap()`; built-in layers: `RateLimitLayer`, `CacheLayer`, `MetricsLayer`, `IpFilter`, `ReverseProxy`; writing a custom layer |
| **Validation** | `Validate` trait; `ValidationErrors`; `Validated<T>` extractor (`422` on failure); `#[derive(Validate)]` with `length`, `range`, `email`, `required`, `url` annotations |
| **Forms & File Uploads** | `FormUrlEncoded::parse()`; `FormMultipartData::parse()`; reading file bytes from multipart parts; size limits |
| **JSON** | Custom JSON parser (`json::object`, `json::array`, `json::property`); `Json<T>` extractor and responder via `serde_json` (`features = ["serde"]`) |
| **Cookies** | `CookieJar::parse()` for reading; `SetCookie` builder for writing; all RFC 6265 attributes (`Secure`, `HttpOnly`, `SameSite`, `Max-Age`, `Domain`, `Path`) |
| **Async Handlers** | `App::with_async_state(S)` for `async fn` handlers; requires `http2` feature; tokio runtime; when to use vs sync |

---

### Features

| Page | Key content |
|------|-------------|
| **CORS & Security Headers** | `cors_allow_all` vs explicit origins; all `RWS_CONFIG_CORS_*` vars; automatic HSTS, CSP (`default-src 'self'`), `X-Frame-Options`, `X-Content-Type-Options`, Client Hints |
| **Rate Limiting** | Per-IP sliding-window `RateLimiter`; `global()`; `check()` / `remaining()` / `reset()`; `RWS_CONFIG_RATE_LIMIT_*` vars; `RateLimitLayer` middleware; live update via SIGHUP |
| **Compression** | Automatic gzip on `Accept-Encoding: gzip`; which content types trigger it; chunked streaming for files > 8 MB |
| **HTTPS / TLS** | Generating a self-signed cert; `rustls` (no OpenSSL); `--tls-cert-file` / `--tls-key-file`; HTTP → HTTPS redirect port |
| **HTTP/2** | `--features http2` build; ALPN negotiation on same port; forbidden headers stripped automatically; `Alt-Svc` advertisement |
| **HTTP/3 / QUIC** | Default build includes HTTP/3; QUIC UDP listener; `Alt-Svc: h3=":PORT"`; `quinn` + `h3-quinn`; when to use |
| **WebSocket** | RFC 6455 handshake; `WebSocket::server_handshake()`; `Frame::read()` / `Frame::write()`; SHA-1 + base64 built in; no extra dep |
| **Server-Sent Events** | `Sse` builder; `SseEvent`; fields: `data`, `event`, `id`, `retry`; `text/event-stream` headers set automatically; use case: AI token streaming |
| **Auth** | `BasicAuthLayer<F>` — validates `Authorization: Basic` via closure (`features = ["auth"]`); `JwtLayer` — HS256 `Authorization: Bearer` token verification; `build_jwt` / `verify_jwt` / `Claims` utilities |
| **Sessions** | `SessionStore` thread-safe in-memory sessions with TTL; `Session` read/write; cookie helpers: `session_id_from_request`, `session_cookie`, `destroy_cookie` |
| **Reverse Proxy** | `ReverseProxy` middleware; round-robin `LoadBalancing`; `path_prefix` for selective proxying; `connect_timeout_ms` / `read_timeout_ms`; hop-by-hop header stripping; `X-Forwarded-For` + `Via` injection; `502 Bad Gateway` when all backends fail |
| **Response Caching** | `CacheLayer::memory(capacity).ttl(secs).vary_by_header("Accept")`; what is cached (GET, 2xx); `Cache-Control: no-store/private` opt-out; `Cache-Control: no-cache` revalidation; `Age` header on hits; oldest-first eviction |
| **Per-Route Metrics** | `MetricsLayer` middleware; `rws_route_requests_total{method,path,status}` counter; `rws_route_duration_seconds{method,path}` histogram (11 buckets); query strings stripped; `record_route()` for custom instrumentation |
| **Hot Config Reload** | SIGHUP trigger (`kill -HUP $(pidof rws)`); `POST /admin/config/reload`; what reloads (CORS, rate limits, log format, allocation size) vs requires restart (port, TLS cert, thread count); `config_reload::current()` |
| **Distributed Tracing** | `OtelLayer` middleware; W3C `traceparent` propagation; `setup()` / `setup_from_env()`; `ExporterConfig::Stdout` (dev) vs `Otlp { endpoint }` (prod); `current_traceparent()` for downstream propagation; `shutdown()` at exit |

---

### Testing

| Page | Key content |
|------|-------------|
| **Test Client** | `TestClient::new(App::new())`; `get()`, `post()`, `put()`, `delete()`, `send()`; `status()`, `body()`; in-process (no TCP); complete example with assertions |

---

### Deployment

| Page | Key content |
|------|-------------|
| **Docker** | Annotated `Dockerfile` (multi-stage); image size per feature flag; `EXPOSE 7878`; env var injection |
| **Kubernetes** | `/healthz` liveness probe; `/readyz` readiness probe (503 during shutdown); `/metrics` Prometheus scrape; graceful shutdown (SIGTERM → 503); HPA config snippet; example `Deployment` + `Service` YAML |
| **Observability** | Server-wide Prometheus counters (`rws_requests_total`, `rws_errors_total`, `rws_active_connections`); per-route counters and latency histograms via `MetricsLayer`; JSON vs Combined Log Format; configuring `log_format`; OpenTelemetry tracing with `OtelLayer` |

---

### Reference

| Page | Key content |
|------|-------------|
| **API Reference** | Link to `docs.rs`; quick-reference table of all public types, traits, and key constants |
| **Roadmap** | Categorized "Coming Soon" list (see below) |

---

## Design Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| Framework | Astro + Starlight | Static HTML output, fast, dark-first, Pagefind search built-in |
| Default theme | Dark | Standard for Rust/systems dev tools |
| Accent color | Electric blue (`#3B82F6`) or rust-orange (`#F97316`) | Matches "cutting edge" aesthetic — pick one |
| Code highlighting | Expressive Code (ships with Starlight) | Inline diffs, file names, line numbers out of the box |
| Coming Soon treatment | Yellow `:::caution[Coming Soon]` admonition blocks | Native Starlight callout, visually distinct without being a dead end |
| Search | Pagefind (built-in, zero JS bundle) | Fast, works offline, no external service |
| API reference | Link out to `docs.rs` | Don't duplicate what rustdoc generates |

---

## Coming Soon Items

These appear as callout blocks within relevant pages — not omitted, not separate stubs.

### Infrastructure
- MCP (Model Context Protocol) server controller
- Virtual hosting / SNI routing

---

## Page Count

| Section | Pages |
|---------|-------|
| Landing | 1 |
| Getting Started | 3 |
| Configuration | 4 |
| Building Apps | 12 |
| Features | 15 |
| Testing | 1 |
| Deployment | 3 |
| Reference | 2 |
| **Total** | **41** |
