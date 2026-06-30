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
    │       │   ├── forms-uploads.md
    │       │   ├── json.md
    │       │   └── cookies.md
    │       ├── features/
    │       │   ├── cors-security.md
    │       │   ├── rate-limiting.md
    │       │   ├── compression.md
    │       │   ├── https-tls.md
    │       │   ├── http2.md
    │       │   └── http3-quic.md
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
- Feature grid (6 cards): Zero dependencies · Thread-pool HTTP/1.1 · Kubernetes-ready · Prometheus metrics · In-process test client · No OpenSSL
- "Get started in 60 seconds" code block
- Link to Quick Start

---

### Getting Started

| Page | Key content |
|------|-------------|
| **Installation** | `cargo add rust-web-server`, build from source, feature flags table (`http1` / `http2` / `http3`), binary sizes, MSRV 1.75 |
| **Quick Start** | Run plain HTTP; serve static files; write and wire your first `Controller` in ~20 lines; verify with `curl` |
| **Features** | Full feature checklist table with checkmarks; at the bottom a "Coming Soon" section for WebSockets, SSE, ACME, reverse proxy, etc. |

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
| **Routing** | Static URI matching in `is_matching`; dynamic `Router` with `:param` and `*wildcard`; `PathParams::get()`; method guards |
| **Request & Response** | `Request` struct fields; `Response` builder; `Header` constants (50+); status code constants; content type via `MimeType`; `ContentRange` / `Range::get_content_range()` |
| **Typed Extractors** | `FromRequest` trait; `Body`, `BodyText`, `Query`, `RequestHeaders`; implementing a custom extractor — **(Coming Soon: derive macro)** |
| **Error Handling** | `AppError` enum variants → HTTP status codes; `IntoResponse` trait; typed errors in controllers |
| **Forms & File Uploads** | `FormUrlEncoded::parse()`; `FormMultipartData::parse()`; reading file bytes from multipart parts; size limits |
| **JSON** | Custom JSON parser (`json::object`, `json::array`, `json::property`); reading values; limitations vs serde; **(Coming Soon: serde integration)** |
| **Cookies** | `CookieJar::parse()` for reading; `SetCookie` builder for writing; all RFC 6265 attributes (`Secure`, `HttpOnly`, `SameSite`, `Max-Age`, `Domain`, `Path`) |

---

### Features

| Page | Key content |
|------|-------------|
| **CORS & Security Headers** | `cors_allow_all` vs explicit origins; all `RWS_CONFIG_CORS_*` vars; automatic HSTS, CSP (`default-src 'self'`), `X-Frame-Options`, `X-Content-Type-Options`, Client Hints |
| **Rate Limiting** | Per-IP sliding-window `RateLimiter`; `global()`; `check()` / `remaining()` / `reset()`; `RWS_CONFIG_RATE_LIMIT_*` vars; wiring into a controller |
| **Compression** | Automatic gzip on `Accept-Encoding: gzip`; which content types trigger it; chunked streaming for files > 8 MB |
| **HTTPS / TLS** | Generating a self-signed cert; `rustls` (no OpenSSL); `--tls-cert-file` / `--tls-key-file`; HTTP → HTTPS redirect port |
| **HTTP/2** | `--features http2` build; ALPN negotiation on same port; forbidden headers stripped automatically; `Alt-Svc` advertisement |
| **HTTP/3 / QUIC** | Default build includes HTTP/3; QUIC UDP listener; `Alt-Svc: h3=":PORT"`; `quinn` + `h3-quinn`; when to use |

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
| **Observability** | Prometheus text format from `/metrics` (`requests_total`, `errors_total`, `active_connections`); JSON vs Combined Log Format; configuring `log_format`; **(Coming Soon: OpenTelemetry tracing, per-route metrics)** |

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

### Framework / DX
- Declarative routing macros (`#[route(GET, "/users/:id")]`)
- `derive(FromRequest)` macro for custom extractors
- Serde JSON integration
- Middleware / filter chain
- Session management
- Request validation helpers

### Protocol / Transport
- WebSocket support
- Server-Sent Events (SSE) for streaming / AI token output
- Automatic TLS (ACME / Let's Encrypt)

### Security
- JWT authentication middleware
- Basic auth middleware
- IP allowlist / denylist

### Infrastructure
- OpenTelemetry distributed tracing
- Per-route metrics
- Hot config reload
- Response caching
- Reverse proxy / load balancing
- MCP (Model Context Protocol) server controller

---

## Page Count

| Section | Pages |
|---------|-------|
| Landing | 1 |
| Getting Started | 3 |
| Configuration | 4 |
| Building Apps | 9 |
| Features | 6 |
| Testing | 1 |
| Deployment | 3 |
| Reference | 2 |
| **Total** | **29** |
