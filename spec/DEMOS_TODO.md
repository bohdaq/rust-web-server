[Read Me](../README.md) > [Spec](.) > Demos TODO

# Demo Apps TODO — rws v17.56.0+

A set of standalone reference applications, each a complete deployable thing built around one persona rather than a feature checklist. Together they exercise every building block listed in `README.md`'s "What's in the box" and every `## Use Case` in `DEVELOPER.md`, so a newcomer can find "the app that looks like mine" and read working code instead of piecing features together from isolated snippets.

Each app should live under `examples/<name>/` as its own `Cargo.toml` depending on `rust-web-server` with the feature flags it needs, plus a `README.md` explaining what it demonstrates and how to run it. Every app must follow the same four-artifact rule as the main crate (tests, `DEVELOPER.md` cross-link, README/llms.txt mention, docs page) — see `CLAUDE.md`.

Ordered by priority: Tier 1 covers the widest surface area and the strongest "look what this can do" story; Tier 2 is infra-layer / config-only and cheaper to build since most of it requires no Rust code.

---

## Tier 1 — primary reference apps

- [ ] **Task Tracker API** — the canonical CRUD reference app; the one most newcomers should read first.
  - `AppWithState`/`Router` with `:param` routes
  - Typed extractors: `Query`, `Body`, `Json<T>` (`serde` feature)
  - `#[derive(Model)]` + async `Repository<T, i64>` + `QueryBuilder` + migrations (`model-sqlite`)
  - `#[derive(Validate)]` request validation → `422` (`macros` feature)
  - Typed errors — `AppError` enum + `IntoResponse`
  - Sessions — `DbSessionStore` + `CookieJar`/`SetCookie`
  - OpenAPI — `.openapi(OpenApiConfig)` → `GET /openapi.json` + `GET /docs` (`openapi` feature)
  - `TestClient` integration tests as the reference testing pattern

- [ ] **Realtime Chat** — streaming/transport primitives.
  - WebSocket handshake + frame relay (RFC 6455)
  - Server-Sent Events (`Sse` builder)
  - HTTP/3 + HTTP/2 + HTTP/1.1 on one port via ALPN, TLS via rustls
  - `RequestIdLayer` to correlate log lines across a session

- [ ] **SaaS Auth Gateway** — every auth mechanism side by side.
  - `BasicAuthLayer`, `JwtLayer` (issue via `build_jwt` + verify), `ForwardAuthLayer` (`auth` feature)
  - OAuth2/OIDC SSO — `OidcAuth` middleware, PKCE, a real provider preset e.g. GitHub (`sso` feature)
  - CSRF double-submit cookie (`csrf` feature)
  - Signed/encrypted cookies (`signed_cookie`/`encrypted_cookie`) + Argon2id password hashing (`crypto` feature)
  - IP filter (`IpFilter::allow`/`deny`) + per-IP `RateLimitLayer`
  - DI `Container` wiring a user repository into handlers

- [ ] **AI Assistant Backend** — the MCP + streaming story; currently the strongest marketing surface.
  - `McpServer` with custom tools/resources/prompts, `.require_bearer(token)`
  - SSE token streaming to a browser
  - `ReverseProxy`/`H2ReverseProxy` fronting a real LLM API upstream with streaming passthrough (`Response::stream_pipe`)
  - `CacheLayer` for repeated-prompt caching

- [ ] **Media / File Upload Service** — storage + async jobs + mail.
  - `Storage` trait with both `LocalStorage` (`storage-local`) and `S3Storage` (`storage-s3`) backends, swappable via config
  - `Mailer`/`Email::builder()` — notification email on upload complete (`mailer` feature)
  - `JobQueue` / `PersistentJobQueue` — background thumbnail/transcode job with retry + exponential backoff (`jobs` feature, `model-*` for persistence)
  - Large file streaming (`Response.stream_file`) + automatic gzip compression

## Tier 2 — infra-layer demos (config-only or near-zero Rust code)

- [ ] **API Gateway** — pure config-driven proxy, deliberately zero application code.
  - `rws.config.toml` with `[[route]]` / `[[upstream]]`; host/path/method/content-type matching
  - Per-route `type = "jwt"` / `"basic"` / `"bearer"` auth, rate limiting, `timeout_ms`
  - Health checks, load-balancing strategies (`RoundRobin`/`Random`/`IpHash`/`LeastConnections`), canary/traffic-splitting, circuit breaker, retry
  - Redirect / fixed-response / static-site actions
  - Service discovery (`File`/`Dns`/`EnvPrefix`) or a `KubernetesIngressWatcher` variant

- [ ] **Polyglot Proxy** — shows rws isn't just an HTTP tool.
  - `TcpProxy` in front of a Postgres/Redis instance
  - `UdpProxy` for a syslog/DNS-style backend
  - `WsProxy` standalone relay
  - `GrpcProxy` fronting a real gRPC service

- [ ] **Production-Ops Reference App** — "how to run this in prod," not a business domain.
  - Prometheus `GET /metrics` + `MetricsLayer` per-route counters/histograms; sample Grafana dashboard
  - OpenTelemetry `OtelLayer` exporting via OTLP (Jaeger/Tempo)
  - Hot config reload (`SIGHUP` / `POST /admin/config/reload`), `/healthz` + `/readyz`, graceful shutdown
  - Background scheduler (fixed-rate/fixed-delay/6-field cron) for a periodic maintenance task
  - Sample Dockerfile + Kubernetes manifests

---

## Cross-reference

Each app, once built, should get a row in `DEVELOPER.md`'s building-blocks table pointing to it as the canonical example, plus a link from the relevant `docs/` page (`building-apps/`, `proxy/`, `database/`, `deployment/`) — see the "Required for every change" checklist in `CLAUDE.md`.
