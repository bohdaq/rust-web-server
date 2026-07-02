# Documentation Site Plan

## Technology: Astro + Starlight

Starlight is purpose-built for documentation, outputs pure static HTML (philosophically aligned with a zero-dependency Rust server), ships with dark mode by default, Expressive Code syntax highlighting, Pagefind full-text search, and a sidebar. Used by Astro itself, Biome, and Tauri.

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
    │       ├── index.mdx                        ← Landing page (hero + feature grid)
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
    │       │   ├── state.md
    │       │   ├── extractors.md
    │       │   ├── error-handling.md
    │       │   ├── middleware.md
    │       │   ├── validation.md
    │       │   ├── forms-uploads.md
    │       │   ├── json.md
    │       │   ├── cookies.md
    │       │   ├── async-handlers.md
    │       │   ├── templates.md
    │       │   └── dependency-injection.md
    │       ├── features/
    │       │   ├── cors-security.md
    │       │   ├── rate-limiting.md
    │       │   ├── compression.md
    │       │   ├── https-tls.md
    │       │   ├── acme.md
    │       │   ├── mtls.md
    │       │   ├── virtual-hosting.md
    │       │   ├── http2.md
    │       │   ├── http3-quic.md
    │       │   ├── websocket.md
    │       │   ├── sse.md
    │       │   ├── auth.md
    │       │   ├── sessions.md
    │       │   ├── caching.md
    │       │   ├── metrics.md
    │       │   ├── tracing.md
    │       │   ├── hot-reload.md
    │       │   ├── rewrite.md
    │       │   ├── ip-filter.md
    │       │   ├── scheduler.md
    │       │   └── config-binding.md
    │       ├── proxy/
    │       │   ├── overview.md
    │       │   ├── config-driven.md
    │       │   ├── reverse-proxy.md
    │       │   ├── load-balancing.md
    │       │   ├── health-checks.md
    │       │   ├── circuit-breaker.md
    │       │   ├── canary.md
    │       │   ├── service-discovery.md
    │       │   ├── tcp-proxy.md
    │       │   ├── udp-proxy.md
    │       │   ├── websocket-proxy.md
    │       │   └── grpc-proxy.md
    │       ├── database/
    │       │   ├── overview.md
    │       │   ├── model-derive.md
    │       │   ├── repository.md
    │       │   ├── query-builder.md
    │       │   ├── raw-sql.md
    │       │   ├── transactions.md
    │       │   ├── migrations.md
    │       │   └── relations.md
    │       ├── mcp/
    │       │   ├── overview.md
    │       │   ├── tools.md
    │       │   ├── resources.md
    │       │   ├── prompts.md
    │       │   └── auth.md
    │       ├── testing/
    │       │   └── test-client.md
    │       ├── deployment/
    │       │   ├── docker.md
    │       │   ├── kubernetes.md
    │       │   ├── kubernetes-ingress.md
    │       │   └── observability.md
    │       └── reference/
    │           ├── api.md
    │           └── roadmap.md
    └── styles/
        └── custom.css
```

---

## Page-by-Page Content Plan

### Landing Page (`index.mdx`)

- Hero: tagline, 1-line install snippet, `cargo run` output in a terminal animation
- Protocol matrix: HTTP/1.1 · HTTP/2 · HTTP/3/QUIC · TLS (rustls) · CORS · Gzip · ETag
- Feature grid (12 cards): Zero HTTP dependencies · Config-driven proxy · MCP server · Middleware pipeline · Model layer ORM · Kubernetes-ready · Prometheus metrics · WebSocket & SSE · In-process test client · Dependency injection · Background scheduler · No OpenSSL
- "Get started in 60 seconds" code block (static file server or first controller)
- "Use as a proxy — no code required" code block (minimal `rws.config.toml`)
- Link to Quick Start

---

### Getting Started

| Page | Key content |
|------|-------------|
| **Installation** | `cargo add rust-web-server`, build from source, feature flags table (`http1` / `http2` / `http3` / `serde` / `auth` / `macros` / `acme` / `tera` / `model-sqlite` / `model-postgres` / `model-mysql`), binary sizes, MSRV 1.75 |
| **Quick Start** | Three paths: (1) static file server, (2) first `Controller`, (3) config-driven proxy with a 5-line `rws.config.toml`. Verify each with `curl`. |
| **Features** | Full feature checklist organized by category (server, library, proxy, database, MCP, AI); links to relevant pages |

---

### Configuration

| Page | Key content |
|------|-------------|
| **Overview** | 4-layer priority diagram: Defaults → Env vars → `rws.config.toml` → CLI args; hot-reload with SIGHUP; what reloads vs requires restart |
| **Environment Variables** | Full table of all `RWS_CONFIG_*` and `RWS_DB_*` vars, types, defaults; grouped by subsystem |
| **Config File** | Annotated `rws.config.toml` with all keys: server, TLS, CORS, rate-limit, log, virtual hosts, upstreams, routes, tcp/udp/ws proxies |
| **CLI Args** | All flags with examples (`--ip`, `--port`, `--thread-count`, `--tls-cert-file`, `--tls-key-file`, `--cors-*`, `--request-allocation-size-in-bytes`) |

---

### Building Apps

| Page | Key content |
|------|-------------|
| **Overview** | Mental model: `Controller` trait → `App::execute()` dispatch chain → `Response`. Request lifecycle diagram from `main.rs` to TCP write. Three ways to add routes: Controller, Router, `App::with_state`. |
| **Controllers** | `Controller` trait (`is_matching` + `process`); annotated minimal example; adding to `App::execute()`; built-in controllers list |
| **Routing** | Static URI matching; `Router` with `:param` / `*wildcard`; `PathParams::get()`; method guards; `routes!` declarative macro; `#[get]`, `#[post]` etc. attribute macros (`macros` feature); `Router::with_host()` for virtual-host routing |
| **Request & Response** | `Request` struct fields; `Response` builder; `Header` constants; status code constants; `MimeType`; `ContentRange` / `Range::get_content_range()` |
| **Shared State** | `App::with_state(S)` — `Arc<S>` shared across handlers; `S: Send + Sync + 'static`; `AppWithState<S>` builder methods; `AsyncAppWithState<S>` for async (http2 feature); choosing `Arc<Mutex<T>>` vs `Arc<RwLock<T>>` |
| **Typed Extractors** | `FromRequest` trait; built-ins: `Body`, `BodyText`, `Query`, `RequestHeaders`; `#[derive(FromRequest)]` for structs; implementing a custom extractor |
| **Error Handling** | `AppError` enum variants → HTTP status codes; `IntoResponse` trait; returning `Result<Response, AppError>` from handlers |
| **Middleware** | `Middleware` trait (`handle`); `App::new().wrap(layer)` stacking; all built-in layers with one-liner each; writing a custom layer |
| **Validation** | `Validate` trait; `ValidationErrors`; `Validated<T>` extractor (400 on extraction, 422 on validation); `#[derive(Validate)]` with `length`, `range`, `email`, `required`, `url` |
| **Forms & File Uploads** | `FormUrlEncoded::parse()`; `FormMultipartData::parse()`; file bytes from multipart parts; size limits |
| **JSON** | Custom built-in JSON parser; `Json<T>` extractor and responder via `serde_json` (`serde` feature); error handling on bad JSON |
| **Cookies** | `CookieJar::parse()` for reading; `SetCookie` builder for writing; all RFC 6265 attributes |
| **Async Handlers** | `App::with_async_state(S)` for `async fn` handlers; requires `http2` feature; tokio runtime; when to use vs sync |
| **HTML Templates** | `TeraEngine` (`tera` feature); `template::init(dir)` global singleton; `template::render(name, &ctx)` in handlers; `Context::new()` / `ctx.insert(k, v)`; Jinja2/Django syntax — variables, loops, conditionals, inheritance |
| **Dependency Injection** | `Container` — `TypeId`-keyed service store; `register::<T>(val)` for concrete types; `provide::<dyn Trait>(Arc::new(...))` for trait objects; `register_named` / `get_named`; `into_arc()` + `App::with_state`; full example with a `UserRepository` trait |

---

### Features

| Page | Key content |
|------|-------------|
| **CORS & Security Headers** | `cors_allow_all` vs explicit origins; all `RWS_CONFIG_CORS_*` vars; automatic HSTS, CSP, `X-Frame-Options`, `X-Content-Type-Options`; Client Hints |
| **Rate Limiting** | Per-IP sliding-window `RateLimiter`; `global()`; `check()` / `remaining()` / `reset()`; `RWS_CONFIG_RATE_LIMIT_*` vars; `RateLimitLayer` middleware; live update via SIGHUP |
| **Compression** | Automatic gzip on `Accept-Encoding: gzip`; which content types trigger it; chunked streaming for files > 8 MB; no body buffering for large files |
| **HTTPS / TLS** | Generating a self-signed cert; `rustls` (no OpenSSL); `--tls-cert-file` / `--tls-key-file`; HTTP → HTTPS redirect port; ALPN negotiation |
| **Automatic TLS (ACME)** | `acme` feature; `RWS_CONFIG_ACME_DOMAINS`, `RWS_CONFIG_ACME_EMAIL`; HTTP-01 challenge built in; background renewal; `RWS_CONFIG_ACME_STAGING` for Let's Encrypt staging; SIGHUP hot-reload after renewal |
| **Mutual TLS (mTLS)** | `RWS_CONFIG_TLS_CLIENT_CA_FILE`; `WebPkiClientVerifier`; applies to both HTTPS and QUIC listeners; how to test with `curl --cert` |
| **Virtual Hosting** | `[[virtual_host]]` in `rws.config.toml`; `SniCertResolver` per-domain cert selection; `Router::with_host("example.com")`; `ConnectionInfo::sni_hostname`; SIGHUP cert hot-reload |
| **HTTP/2** | `--features http2` build; ALPN negotiation on same TCP port; forbidden headers stripped automatically; `Alt-Svc` advertisement; `H2ReverseProxy` |
| **HTTP/3 / QUIC** | Default build; QUIC UDP on same port as TCP; `Alt-Svc: h3=":PORT"`; `quinn` + `h3-quinn`; when to use; no extra config for clients that support it |
| **WebSocket** | RFC 6455 handshake; `WebSocket::is_upgrade_request()`; `WebSocket::handshake_response()`; frame read/write; `Frame` enum; SHA-1 + base64 built in; no extra dep |
| **Server-Sent Events** | `Sse` builder; `SseEvent` fields (`data`, `event`, `id`, `retry`); headers set automatically; use case: AI token streaming; multi-line data |
| **Auth** | `BasicAuthLayer<F>` via closure (`auth` feature); `JwtLayer` HS256 Bearer verification; `build_jwt` / `verify_jwt` / `Claims`; `extract_bearer_token`; `IpFilter::allow` / `deny` |
| **OAuth2 / OIDC SSO** | `sso` feature; `OidcConfig::from_env()`; provider presets (`OidcConfig::google()`, `microsoft()`, `github()`, `okta()`, `auth0()`, `keycloak()`); authorization-code + PKCE flow; RS256/ES256 JWT verification via JWKS endpoint; `OidcAuth` middleware; `Claims` extraction in handlers |
| **CSRF Protection** | `csrf` feature; `CsrfLayer` double-submit cookie; `CsrfToken::from_request` extractor for embedding token in HTML forms; `X-CSRF-Token` header for AJAX; `SameSite=Strict`; constant-time comparison |
| **Sessions** | `SessionStore` TTL in-memory sessions; `Session` get/set; `store.create()`, `save()`, `load()`, `destroy()`, `purge_expired()`; cookie helpers |
| **Response Caching** | `CacheLayer::memory(capacity).ttl(secs).vary_by_header("Accept")`; what is cached; `Cache-Control: no-store/private` opt-out; `no-cache` revalidation; `Age` header on hits; oldest-first eviction |
| **Per-Route Metrics** | `MetricsLayer` middleware; `rws_route_requests_total{method,path,status}` counter; `rws_route_duration_seconds{method,path}` histogram (11 buckets); query strings stripped; `GET /metrics` Prometheus format |
| **Distributed Tracing** | `OtelLayer` middleware; W3C `traceparent` propagation; `setup()` / `setup_from_env()`; `ExporterConfig::Stdout` (dev) vs `Otlp { endpoint }` (prod); `current_traceparent()` for downstream propagation; `shutdown()` at exit |
| **Hot Config Reload** | SIGHUP trigger; `POST /admin/config/reload`; what reloads (CORS, rate limits, log format, allocation size, TLS certs) vs requires restart (port, thread count); `config_reload::current()` anywhere in the handler stack |
| **Request / Response Rewriting** | `RewriteLayer` builder methods: `request_header_set/remove`, `request_uri_set/strip_prefix/add_prefix`, `response_header_set/remove`, `response_status`, `response_body_replace`; composition with other middleware |
| **IP Filtering** | `IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16"])` / `IpFilter::deny(["1.2.3.4"])`; exact IPv4 and CIDR; IPv6 pass-through |
| **Background Scheduler** | `Scheduler::new()`; `.every(Duration, fn)`, `.after(Duration, fn)`, `.cron("s m h d M wd", fn)`; full cron field syntax; `.initial_delay()`; `.start()` spawns one thread per task |
| **Typed Config Binding** | `#[derive(Config)]` (`macros` feature); `#[config(env = "KEY", default = "v")]`; `Option<T>` optional fields; `load() -> Result<Self, String>`; `FromEnvStr` for custom types |

---

### Proxy / Gateway

| Page | Key content |
|------|-------------|
| **Overview** | Two modes: (1) `rws.config.toml` proxy mode — no code, drop a config file and run; (2) library mode — use `ReverseProxy` and other middleware programmatically. When to choose each. |
| **Config-Driven Proxy** | Full annotated `rws.config.toml` reference: `[[upstream]]`, `[[route]]`, `[route.match]` (host, path, method, content-type), `[route.action]` (proxy, redirect, respond), `[route.middleware]` (rate_limit, auth), `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]`; `ProxyConfig::is_proxy_mode()` detection |
| **Reverse Proxy** | `ReverseProxy` middleware; `LoadBalancing::RoundRobin`; `path_prefix` for selective proxying; `connect_timeout_ms` / `read_timeout_ms`; hop-by-hop header stripping; `X-Forwarded-For` + `Via` injection; `502 Bad Gateway` fallback |
| **Load Balancing** | Current: round-robin (atomic counter, lock-free). Coming soon: `least_connections`, `ip_hash`, `random` strategies. `DynamicProxy` in config-driven mode tracks live backends from health checks. |
| **Health Checks** | `[upstream.health_check]` in `rws.config.toml`; `path`, `interval_secs`, `timeout_ms`, `healthy_threshold`, `unhealthy_threshold`; background daemon thread per upstream; live backend list via `Arc<RwLock<Vec<String>>>` |
| **Circuit Breaker** | `CircuitBreaker` state machine: Closed → Open → HalfOpen; `global()` singleton; threshold and recovery window; `RetryLayer` middleware — retries on 502/503/504 up to `max_retries` |
| **Canary / Traffic Splitting** | `CanaryLayer` middleware; `WeightedBackend::new(url, weight)`; weight-0 removes from rotation; deterministic lock-free round-robin; example: 90/10 split for gradual rollout |
| **Service Discovery** | `BackendPool` with four sources: `Static` (fixed list), `EnvPrefix` (scan `PREFIX_0`, `PREFIX_1`, …), `File` (one host:port per line, polled), `Dns` (A-record lookup via `ToSocketAddrs`); `.start()` spawns background refresh thread |
| **TCP Proxy (L4)** | `TcpProxy` standalone listener; `bind(addr)` blocks; `relay(client)` two-thread `io::copy`; round-robin backends via `AtomicUsize`; `[[tcp_proxy]]` config section |
| **UDP Proxy** | `UdpProxy` datagram proxy; ephemeral socket per datagram; `set_read_timeout()` controls reply wait; round-robin backends; `[[udp_proxy]]` config section |
| **WebSocket Proxy** | `WsProxy` standalone listener; reads HTTP upgrade from client, connects to backend, exchanges upgrade handshake, relays raw bytes bidirectionally; `[[ws_proxy]]` config section |
| **gRPC Proxy** | `GrpcProxy` wraps `H2ReverseProxy`; filters on `Content-Type: application/grpc*`; `http2` feature required; trailer handling status |

---

### Database

| Page | Key content |
|------|-------------|
| **Overview** | Feature flags: `model-sqlite` (bundled SQLite via rusqlite), `model-postgres` (postgres crate, pure Rust), `model-mysql` (mysql crate); one driver per compilation; `DbPool::from_env()` reads `RWS_DB_*` vars; `pool.get()` returns `PooledConnection` returned to pool on drop |
| **`#[derive(Model)]`** | Struct-level `#[table(name = "users")]`; field attributes: `#[primary_key(auto_increment)]`, `#[column(name = "first_name")]`, `#[column(unique)]`, `#[ignore]`; what the macro generates: `table_name()`, `column_names()`, `from_row()`, `to_values()`, `repository()`, `query()`; supported field types |
| **Repository** | `Repository<T, ID>` trait; `User::repository(&mut conn)` → `ModelRepository`; all CRUD methods: `find_by_id`, `find_all`, `save` (INSERT if pk==0 or auto-inc else UPDATE), `save_all`, `delete_by_id`, `delete_all_by_id`, `count`, `exists_by_id`; auto-increment PK retrieval per backend (`last_insert_rowid` / `RETURNING id` / `last_insert_id`) |
| **Query Builder** | `User::query(&mut conn)` → `QueryBuilder`; `.where_eq("role", "admin")`, `.filter("age >= ?", vec![Value::Int(18)])`, `.order_by("name", Order::Asc)`, `.limit(20)`, `.offset(40)`, `.fetch_all()`, `.fetch_one()`, `.count()`, `.delete()`; placeholder adapts to driver (`?` vs `$N`) |
| **Raw SQL** | `db.query::<T>(sql, &[Value::Int(18)])` → `Vec<T>`; `db.query_raw(sql, params)` → `Vec<ModelRow>`; `db.execute(sql, params)` → rows affected; `ModelRow::get::<T>("column")` |
| **Transactions** | Closure-based: `conn.transaction(|c| { … })` — rolls back on `Err`; manual: `conn.begin()` / `conn.commit()` / `conn.rollback()`; nested closure example (User + Profile insert) |
| **Migrations** | `migrations/*.sql` files in lexicographic order; `db.migrate("migrations/")` creates `_schema_migrations(version, applied_at)` if absent, runs each unapplied file in a transaction; `db.migration_status("migrations/")` → `Vec<MigrationStatus { version, applied }>`; call on startup before serving requests |
| **Relations** | `HasMany<T>` / `HasOne<O>` / `BelongsTo<O>` explicit-load helpers; `user.posts.load(&mut conn)` → `Vec<Post>`; no lazy loading, no hidden N+1 queries; `#[has_many(Post, foreign_key = "user_id")]` attribute |

---

### MCP Server

| Page | Key content |
|------|-------------|
| **Overview** | What is MCP (Model Context Protocol); JSON-RPC 2.0 over HTTP (`POST /mcp`); `McpServer::new(name, version)` / `app.mcp(name, version)`; `.wrap(app)` to fall through non-MCP requests; 8 built-in rws tools; connecting Claude, Cursor, and other MCP clients |
| **Tools** | `.tool(name, description, schema_json, handler)` where `handler: Fn(Value) -> Result<McpContent, String>`; `McpContent::text()`, `McpContent::json()`, `McpContent::image()`; input schema as JSON Schema object; listing and calling from Claude |
| **Resources** | `.resource(uri_template, name, description, handler)`; URI templates with `{variable}` placeholders; reading files, config values, live metrics; resource listing and reading |
| **Prompts** | `.prompt(name, description, handler)` where `handler: Fn(Vec<PromptArg>) -> Result<Vec<PromptMessage>, String>`; `PromptMessage::user()` / `PromptMessage::assistant()`; `extract_arg(args, "name")`; prompt listing |
| **Auth** | `.require_bearer(token)` — static Bearer token gates all MCP requests; `RWS_CONFIG_MCP_TOKEN` env var pattern; 401 response with `WWW-Authenticate: Bearer` header |

---

### Testing

| Page | Key content |
|------|-------------|
| **Test Client** | `TestClient::new(App::new())`; builder: `.get(path)`, `.post(path)`, `.put()`, `.delete()`; `.with_header(name, value)`, `.with_body(bytes)`; `.send()` → `TestResponse`; `resp.status()`, `resp.body_text()`, `resp.body_bytes()`; no TCP socket; complete CRUD test example |

---

### Deployment

| Page | Key content |
|------|-------------|
| **Docker** | Annotated `Dockerfile` (multi-stage, rust:1.75 → debian-slim); image size per feature flag (http1 ≈ 3 MB, http3 ≈ 12 MB); `EXPOSE 7878`; env var injection; healthcheck `CMD curl /healthz` |
| **Kubernetes** | `/healthz` liveness; `/readyz` readiness (503 during shutdown); `/metrics` Prometheus scrape; graceful shutdown (SIGTERM → 503 → drain); HPA config; example `Deployment` + `Service` + `PodDisruptionBudget` YAML |
| **Kubernetes Ingress** | `KubernetesIngressWatcher` polls `/apis/networking.k8s.io/v1/ingresses`; `RWS_K8S_API_SERVER`, `RWS_K8S_TOKEN`, `RWS_K8S_NAMESPACE`; `from_service_account()` in-cluster; `IngressRouter` forwards to `service.namespace.svc.cluster.local:port`; poll interval; `pathType: Prefix` support |
| **Observability** | Server-wide counters (`rws_requests_total`, `rws_errors_total`, `rws_active_connections`); per-route counters + histograms via `MetricsLayer`; JSON vs Combined Log Format (`RWS_CONFIG_LOG_FORMAT`); OpenTelemetry tracing with `OtelLayer`; Grafana dashboard snippet |

---

### Reference

| Page | Key content |
|------|-------------|
| **API Reference** | Link to `docs.rs`; quick-reference table of all public types, traits, key constants grouped by module |
| **Roadmap** | Coming-soon items as `:::caution[Coming Soon]` admonition blocks; items from `spec/IDEAS.md`: upstream TLS for proxy, load balancing strategies (least_conn/ip_hash/random), JWT/basic auth from config, static site action, regex URI rewriting, forward-auth middleware, multi-span tracing, admin UI, access log rotation |

---

## Design Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| Framework | Astro + Starlight | Static HTML, fast, dark-first, Pagefind search built-in |
| Default theme | Dark | Standard for Rust/systems dev tools |
| Accent color | Electric blue (`#3B82F6`) or rust-orange (`#F97316`) | Matches "cutting edge" aesthetic — pick one |
| Code highlighting | Expressive Code (ships with Starlight) | Inline diffs, file names, line numbers out of the box |
| Coming Soon treatment | Yellow `:::caution[Coming Soon]` admonition blocks | Visually distinct without being a dead end |
| Search | Pagefind (built-in, zero JS bundle) | Fast, works offline, no external service |
| API reference | Link out to `docs.rs` | Don't duplicate what rustdoc generates |

---

## Coming Soon Items (Roadmap Page)

From `spec/IDEAS.md` — appear as callout blocks in the relevant sections, and listed together on the Roadmap page.

| Item | Relevant page |
|---|---|
| Upstream TLS for config proxy (`https://` backends) | Proxy / Health Checks; Proxy / Config-Driven |
| Load balancing strategies (`least_connections`, `ip_hash`, `random`) | Proxy / Load Balancing |
| JWT and Basic auth from `rws.config.toml` | Proxy / Config-Driven; Features / Auth |
| Static site action in config proxy (`type = "static"`) | Proxy / Config-Driven |
| Regex URI rewriting | Features / Rewrite |
| Forward-auth middleware (`ForwardAuthLayer`) | Features / Auth |
| Multi-span distributed tracing (child spans) | Features / Tracing |
| Admin UI (`GET /admin`) | Features / Metrics |
| Access log rotation | Deployment / Observability |

---

## Page Count

| Section | Pages |
|---------|-------|
| Landing | 1 |
| Getting Started | 3 |
| Configuration | 4 |
| Building Apps | 15 |
| Features | 23 |
| Proxy / Gateway | 12 |
| Database | 8 |
| MCP Server | 5 |
| Testing | 1 |
| Deployment | 4 |
| Reference | 2 |
| **Total** | **78** |
