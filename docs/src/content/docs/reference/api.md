---
title: API Reference
description: Quick-reference index of all major public types in rust-web-server, grouped by area.
---

Full rustdoc documentation is published at [docs.rs/rust-web-server](https://docs.rs/rust-web-server).

This page is a quick-reference index — click through to docs.rs for the complete method-level API. Module paths are relative to `rust_web_server::`; a feature in parentheses means the item is compiled only with that Cargo feature.

## Application layer

| Type / Trait | Module | Description |
|---|---|---|
| `Application` | `application` | Trait implemented by all app types: `execute(&Request, &ConnectionInfo) -> Result<Response, String>`. `runs_inline(request, connection)` (default `false`) lets the async servers run a request on the event loop instead of the blocking pool — only for work that never waits. See [Async handlers](/building-apps/async-handlers/) |
| `AsyncResponse` | `application` (`http2`) | Return type of the optional `Application::execute_async`, awaited on the connection's task instead of holding a blocking-pool thread |
| `Controller` | `controller` | Trait with `is_matching(request, connection)` and `process(request, response, connection)`. See [Controllers](/building-apps/controllers/) |
| `App` | `app` | Zero-config application with all built-in controllers. `App::with_config(ServerConfig)` pins it to a fixed config (no env reads per request — the lock-free pattern for parallel tests) |
| `AppWithState<S>` | `state` | State-aware app with `.get/.post/.put/.patch/.delete` route builders; state shared as `Arc<S>`. Entry point `App::with_state(S)`; `.with_config(ServerConfig)` pins the fallback `App`. See [State](/building-apps/state/) |
| `AsyncAppWithState<S>` | `async_state` (`http2`) | Same with `async fn` handlers. Entry point `App::with_async_state(S)` |
| `Middleware` | `middleware` | Layer trait: `handle(request, connection, next)`. `runs_inline` / `is_passthrough` (default `false`) declare a layer non-blocking or a no-op for a request. See [Middleware](/building-apps/middleware/) |
| `WithMiddleware<A>` | `middleware` | Wraps any `Application` with a layer stack; `app.wrap(layer)`, first-pushed is outermost |
| `Container` | `di` | `TypeId`-keyed dependency injection container: `register::<T>`, `provide::<dyn Trait>`, `*_named`, `get`. Pass it directly to `App::with_state(container)` — not `into_arc()`, which double-wraps. See [Dependency injection](/building-apps/dependency-injection/) |
| `TeraEngine` | `template` (`tera`) | Jinja2-style templates: `from_dir`, `from_raw`; global `template::init(dir)` / `template::render(name, &ctx)` / `template::reload()` (also run on `SIGHUP`). See [Templates](/building-apps/templates/) |

## Macros

| Item | Module | Description |
|---|---|---|
| `routes!` | `macros` | Declarative routing table — builds `AppWithState`, `AsyncAppWithState` or `Router` from `METHOD "path" => handler` lines. See [Routing](/building-apps/routing/) |
| `#[route]`, `#[get]`, `#[post]`, `#[put]`, `#[patch]`, `#[delete]` | crate root (`macros`) | Attribute macros annotating a handler with its method and path |
| `#[derive(FromRequest)]` | crate root (`macros`) | Derives `FromRequest` for a named-field struct, extracting fields in order and stopping at the first error |
| `#[derive(JsonSchema)]` | crate root (`macros`) | Derives `schema::JsonSchema` for a named-field struct or unit enum, following its `#[serde]` renames (18.0.0). See [Typed tools](/mcp/tools/#typed-tools) |
| `#[derive(Validate)]` | crate root (`macros`) | Derives `Validate` from `#[validate(length, range, email, required, url)]` field annotations |
| `#[derive(Config)]` | crate root (`macros`) | Typed env-var binding: generates `load() -> Result<Self, String>`; `#[config(env, default, prefix)]`. See [Config binding](/features/config-binding/) |
| `#[derive(Model)]` | crate root (`macros` + a `model-*` feature) | Maps a struct to a table (`#[table]`, `#[primary_key(auto_increment)]`, `#[column]`, `#[ignore]`) and generates `T::repository(&pool)` / `T::query(&pool)` |

## HTTP primitives

| Type / Trait | Module | Description |
|---|---|---|
| `Request` | `request` | Parsed HTTP request: method, URI, version, headers, body bytes. `request.header(name)` is a case-insensitive, allocation-free lookup; `Request::is_supported_method` checks a method token |
| `Response` | `response` | HTTP response: status code, headers, body as `Vec<ContentRange>`. `Response::json(status, bytes)` / `Response::text(status, &str)` are one-line builders; `response.header(name)` looks a header up |
| `STATUS_CODE_REASON_PHRASE` | `response` | Every HTTP status code as a typed constant |
| `Header` | `header` | Name/value pair (`Cow<'static, str>` fields since 18.0.0; build with `Header::new(name, value)`, read via deref); constants for all standard header names. See [Migrating to 18.0](/reference/migrating-18/) |
| `HTTP` | `http` | Version constants; `HTTP::is_supported_version` |
| `MimeType` | `mime_type` | MIME type constants and detection from a file extension |
| `Range` / `ContentRange` | `range` | Byte ranges and the body parts of a `Response` (bytes, files, multipart) |
| `URL` | `url` | Parse and build URLs; percent-encode/decode |
| `FormUrlEncoded` | `body::form_urlencoded` | Parses `application/x-www-form-urlencoded` bodies. See [Forms & uploads](/building-apps/forms-uploads/) |
| `FormMultipartData` | `body::multipart_form_data` | Parses `multipart/form-data` uploads |
| `Json<T>` | `json` (`serde`) | Serde JSON extractor and responder. See [JSON](/building-apps/json/) |
| `CookieJar` | `cookie` | Parses the `Cookie` request header. See [Cookies](/building-apps/cookies/) |
| `SetCookie` | `cookie` | Builds `Set-Cookie` values with all RFC 6265 attributes |
| `signed_cookie` / `verify_signed_cookie` | `cookie` (`crypto`) | HMAC-SHA256 tamper-evident cookie value `"<value>.<hex-signature>"`; verification returns `None` on tampering |
| `encrypted_cookie` / `decrypt_cookie` | `cookie` (`crypto`) | AES-256-GCM confidential cookie value; fresh nonce per call, `None` on tampering or wrong key |

## Routing

| Type / Trait | Module | Description |
|---|---|---|
| `Router` | `router` | Path-pattern router with `:param` and `*wildcard`; `.with_host(name)` restricts it to one virtual host; `.matches(request, connection)` tells whether `handle()` would run a handler |
| `PathParams` | `router` | Extracted path parameters; `.get(name) -> Option<&str>` |
| `ConnectionInfo` | `server` | Client/server IP, port, and SNI hostname per connection |
| `VirtualHostConfig` | `virtual_host` | Per-domain certificate config `{ domain, cert_file, key_file }` for SNI. See [Virtual hosting](/features/virtual-hosting/) |

## Typed extractors and errors

| Type / Trait | Module | Description |
|---|---|---|
| `FromRequest` | `extract` | Trait for typed extraction from a request, returning a ready error response on failure. See [Extractors](/building-apps/extractors/) |
| `Body` | `extract` | Raw request body bytes (never fails) |
| `Storage::put_stream` / `S3_PART_SIZE` | `storage` | Store a `Read` stream: S3 multipart upload (8 MiB parts), local temp file + rename; default buffers (18.0.0) |
| `request::Body` | `request` | `Request.body` (18.0.0): bytes, or a stream (`Body::stream`, `body_bytes()`, `into_reader()`); derefs to `&[u8]`. Not the `extract::Body` extractor |
| `BodyText` | `extract` | UTF-8 request body (returns 400 on invalid UTF-8) |
| `Query` | `extract` | Parsed query string as `HashMap<String, String>` |
| `RequestHeaders` | `extract` | All request headers with case-insensitive `.get(name)` |
| `RequestId` | `extract` / `request_id` | The ID `RequestIdLayer` put on the request; empty string without it (never fails) |
| `IntoResponse` | `error` | Trait for mapping error enums to `Response` values. See [Error handling](/building-apps/error-handling/) |
| `AppError` | `error` | Built-in error variants: `BadRequest`, `NotFound`, `Internal`, etc. |
| `Validate` | `validate` | Field-level validation trait. See [Validation](/building-apps/validation/) |
| `ValidationErrors` | `validate` | Collects every validation failure before returning |
| `Validated<T>` | `validate` | Extractor that extracts then validates: 400 on extraction failure, 422 with a JSON error body on validation failure |
| `is_email` / `is_url` | `validate` | Format checks used by the derive; callable directly |

## Server and configuration

| Type / Trait | Module | Description |
|---|---|---|
| `Server` | `server` | TCP listener, thread pool, TLS acceptor, and request loop. `Server::continue_response` / `expectation_failed_response` implement `Expect: 100-continue` / 417 on HTTP/1.1 |
| `server::REQUEST_HEAD_TIMEOUT` | `server` | 30 s: a request head must complete within this of its first byte, else `408` |
| `ServerConfig` | `server_config` | Typed snapshot of per-instance config (CORS, CSP, `client_hints`, `static_cache_control`, log format). `ServerConfig::current()` is the shared per-request snapshot; call `server_config::invalidate()` after a direct `std::env::set_var` |
| `ConfigSnapshot` | `config_reload` | Hot-reloadable values, read with `config_reload::current()`. See [Hot reload](/features/hot-reload/) |
| `config_reload::reload` / `config_reload::on_reload` | `config_reload` | Re-read `rws.config.toml` (also on `SIGHUP` and `POST /admin/config/reload`); register a closure run after every reload |
| `ConfigReport` | `config_check` | Result of `config_check::check_file` / `check_str` (what `rws --check` runs): `errors`, `warnings`, `.is_ok()`, `.render(path)`, `.render_json(path)`. See [Config file](/configuration/config-file/) |
| `get_max_body_size` | `entry_point` | Reads `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` (default `0`, unlimited); larger requests get `413` before buffering |
| `get_spa_fallback` / `get_spa_fallback_exclude_prefixes` | `entry_point` | Read `RWS_CONFIG_SPA_FALLBACK*`. See [Static files](/features/static-files/) |
| `FromEnvStr` | `config_binding` | Implement for custom field types used with `#[derive(Config)]` |
| `secrets::resolve` / `resolve_env_vars` | `secrets` (`secrets`) | Resolve `vault://`, `aws-sm://` and `azkv://` references; `Server::setup()` rewrites matching `RWS_*` env vars at startup and fails hard on errors. See [Secrets](/features/secrets/) |
| `SecretsError` | `secrets` (`secrets`) | Error type for secret resolution |
| `StaticResourceController::STREAM_THRESHOLD` | `app::controller::static_resource` | 256 KiB: larger static files are streamed from disk instead of read into memory |
| `StaticResourceController::render_directory_listing` | `app::controller::static_resource` | Renders the listing `App` serves for a directory without `index.html` |
| `DirectoryListingAssetsController` | `app::controller::directory_listing` | Serves the listing's CSS/JS at `/rws-directory-listing.{css,js}` (same-origin, so the default CSP allows them); a file on disk overrides the built-in |

## Middleware

| Type / Trait | Module | Description |
|---|---|---|
| `RateLimiter` | `rate_limit` | Sliding-window per-key limiter; `rate_limit::global()` for the singleton; `.retry_after()` / `.limits()`. See [Rate limiting](/features/rate-limiting/) |
| `RedisRateLimiter` | `rate_limit` | Fixed-window limiter shared across instances through Redis; `from_env()` |
| `RateLimitLayer` | `middleware` | Enforces the global rate limiter per client IP; `429` with `Retry-After` |
| `CacheLayer` | `cache` | In-memory TTL response cache for GET (`CacheLayer::memory(n).ttl(s).vary_by_header(h)`); skips `no-store`/`private`/`no-cache`, `Range` and out-of-key `Vary` responses. See [Caching](/features/caching/) |
| `MetricsLayer` | `metrics` | Per-route Prometheus counters and latency histograms. See [Metrics](/features/metrics/) |
| `RewriteLayer` | `rewrite` | Request/response header, URI, body, and status rewriting; `.request_uri_regex_rewrite` needs `rewrite-regex`. See [Rewrite](/features/rewrite/) |
| `BasicAuthLayer` | `auth` (`auth`) | HTTP Basic authentication via a closure or `from_htpasswd_file` (plain or rws's `{SHA256}` scheme — not Apache `{SHA}`/`$apr1$`/bcrypt). See [Auth](/features/auth/) |
| `JwtLayer` | `auth` (`auth`) | HS256 Bearer JWT validation; `::rs256`/`::es256` verify against a static public key (`auth-asymmetric`) |
| `ForwardAuthLayer` | `auth::forward` (`auth`) | Delegates allow/deny to an external HTTP service; verbatim pass-through on non-2xx, 502 if unreachable |
| `IpFilter` | `ip_filter` | IPv4/IPv6 address or CIDR allowlist/denylist; `ip_filter::is_valid_entry` checks an entry. See [IP filter](/features/ip-filter/) |
| `Blocklist` | `blocklist` | Runtime IP blocklist: `blocklist::global().block(ip)` / `.unblock` / `.is_blocked` / `.list` / `.clear` |
| `BlocklistLayer` | `blocklist` | Answers `403` for blocked client IPs |
| `MaintenanceLayer` / `MAINTENANCE_MODE` | `maintenance` | While the `AtomicBool` is `true`, answers `503` for everything but `/healthz` and `/readyz` |
| `RequestLog` | `request_log` | In-memory buffer of recent requests (`request_log::global()`, capacity 1000); `.recent(n)` / `.recent_errors(n)` |
| `LogLayer` | `request_log` | Records each request into `RequestLog` |
| `RequestIdLayer` | `request_id` | Correlation-ID middleware — echoes or generates `X-Request-Id` (`generate_request_id()`, UUID-v4-shaped, not crypto-random). See [Request ID](/features/request-id/) |
| `CsrfLayer` | `csrf` (`csrf`) | Double-submit cookie CSRF protection; 403 on mismatch. See [CSRF](/features/csrf/) |
| `CsrfToken` | `csrf` (`csrf`) | Extractor for the current token in a GET handler; implements `Display` |
| `TimeoutLayer` | `timeout` | Wraps a whole `Application` with a deadline (`504`). See [Timeouts](/features/timeouts/) |
| `with_timeout` / `with_timeout_state` / `with_timeout_async` | `timeout` | Per-handler timeouts. The sync ones bound the client's wait but can't cancel the work; `with_timeout_async` (`http2`) cancels the future |
| `FeatureStore` | `feature` | Runtime feature toggles: `feature::global().set(name, bool)` / `.is_enabled` / `.list` |

## Authentication and crypto

| Type / Trait | Module | Description |
|---|---|---|
| `build_jwt` / `verify_jwt` | `auth` (`auth`) | Sign and verify HS256 JWTs |
| `Claims` | `auth` (`auth`) | Decoded JWT claims: `sub`, `exp`, raw JSON payload |
| `verify_jwt_rs256` / `verify_jwt_es256` | `auth` (`auth-asymmetric`) | Standalone RS256/ES256 verification backing `JwtLayer::rs256`/`::es256` |
| `RsaPublicKey` | `rsa_key` (`auth-asymmetric` / `sso`) | In-crate RSA public key (PEM, SPKI/PKCS#1 DER or components; 2048–8192 bits); `.verify_pkcs1v15_sha256(message, signature)` |
| `hash_password` / `verify_password` | `crypto` (`crypto`) | Argon2id hashing (PHC string, salt embedded) and constant-time verification |
| `generate_token` | `crypto` (`crypto`) | CSPRNG hex token for reset links and API keys |
| `CryptoError` | `crypto` (`crypto`) | Error type for password hashing |
| `verify_webhook_signature` | `webhook` (`webhook`) | Verifies an inbound webhook HMAC for a `WebhookProvider`. See [Webhooks](/features/webhooks/) |
| `WebhookProvider` | `webhook` (`webhook`) | `GitHub`, `Shopify`, `Stripe` (300 s default replay tolerance) |
| `verify_github_signature` / `verify_shopify_signature` / `verify_stripe_signature` / `verify_stripe_signature_with_tolerance` | `webhook` (`webhook`) | Per-provider functions when the provider is known at the call site |

## SSO

| Type / Trait | Module | Description |
|---|---|---|
| `OidcAuth` | `sso` (`sso`) | OAuth2/OIDC middleware: intercepts `/auth/login`, `/auth/callback`, `/auth/logout`; `OidcAuth::claims(req)` reads the claims. See [SSO](/features/sso/) |
| `OidcConfig` | `sso` (`sso`) | Client configuration; presets `google`, `microsoft`, `github`, `okta`, `auth0`, `keycloak`, and `from_env()` |
| `OidcProvider` | `sso` (`sso`) | Provider endpoints; same presets plus `OidcProvider::discover(issuer)` |
| `OidcClaims` | `sso` (`sso`) | Standard OIDC claims from a verified `id_token` or UserInfo response (no `groups`) |
| `JwksCache` | `sso` (`sso`) | JWKS key cache; `verify_jwt(token, &VerifyOptions)` checks RS256/ES256 signature, `exp`, `iss`, `aud`; refetches once on a key miss |
| `VerifyOptions` | `sso` (`sso`) | `audience`, `issuer`, `leeway_secs` for `JwksCache::verify_jwt` |
| `OidcClient` | `sso` (`sso`) | `authorization_url`, `exchange_code`, `fetch_user_info` |
| `TokenResponse` | `sso` (`sso`) | Token endpoint response returned by `exchange_code` |
| `PkceVerifier` | `sso` (`sso`) | RFC 7636 code verifier; `.challenge()` |
| `PkceChallenge` | `sso` (`sso`) | S256 code challenge for the authorization URL |
| `AuthServer` | `sso` (`sso-server`) | rws as an OAuth 2.0 Authorization Server (`/oauth/token`, `/oauth/authorize`, discovery). Tokens are HS256, so `jwks.json` is always empty; `/oauth/authorize` trusts an existing app session, with no built-in login page |
| `AuthServerConfig` | `sso` (`sso-server`) | `issuer`, `signing_secret`, access/refresh token lifetimes, `clients` |
| `ClientStore` | `sso` (`sso-server`) | Registry of OAuth clients: `ClientStore::new().add(client).get(id)` (no `from_env`) |
| `OAuthClient` | `sso` (`sso-server`) | One client; `client_secret: None` is a public PKCE-only client |
| `GrantType` | `sso` (`sso-server`) | `ClientCredentials`, `AuthorizationCode`, `RefreshToken` |
| `SamlSp` | `sso::saml` (`sso-saml`) | SAML 2.0 Service Provider middleware (`/saml/metadata`, `/saml/login`, `/saml/acs`, `/saml/logout`). Verifies RSA-SHA256 signatures byte-exact against the transmitted XML (not full C14N — fails closed), requires exactly one `Assertion`, rejects `EncryptedAssertion`; unsigned HTTP-POST `AuthnRequest`s; local-only logout. See [SAML](/features/saml/) |
| `SamlConfig` | `sso::saml` (`sso-saml`) | `sp_entity_id`, `sp_acs_url`, `idp_metadata`, `sessions` |
| `SamlIdpMetadata` | `sso::saml` (`sso-saml`) | Parsed IdP metadata (`entity_id`, `sso_url`, `signing_key`); `from_file` / `from_url` / `from_str` |
| `AttributeMap` | `sso::saml` (`sso-saml`) | Maps IdP attribute names to claim fields |
| `SamlClaims` | `sso::saml` (`sso-saml`) | `name_id` plus `attributes: HashMap<String, String>`; read with `SamlSp::claims` / `name_id` / `attr` |

## Sessions

| Type / Trait | Module | Description |
|---|---|---|
| `SessionStore` | `session` | In-memory TTL session store; helpers `session_id_from_request`, `session_cookie`, `destroy_cookie`. See [Sessions](/features/sessions/) |
| `Session` | `session` | One session's data |
| `DbSessionStore` | `session` (a `model-*` feature) | Persistent store on the model-layer `DbPool` (`rws_sessions` table); async methods |
| `RedisSessionStore` | `session` | Persistent store on Redis; `new(addr, password, ttl)` or `from_env()` |

## Real-time protocols

| Type / Trait | Module | Description |
|---|---|---|
| `Sse` | `sse` | `Sse::stream()` → a streaming response and its `SseSender`; `Sse::new()` builds a buffered `text/event-stream` body. See [SSE](/features/sse/) |
| `SseStream` | `sse` | Options for a streaming response: `.buffer(events)`, `.keep_alive(interval)`, `.start()` |
| `SseSender` / `SseClosed` | `sse` | Feeds a streaming response: `send` (waits for room), `try_send`, `send_async` (`http2`), `data`, `comment`; `Err(SseClosed)` once the client is gone |
| `SseEvent` | `sse` | One event: data plus optional `event`, `id`, `retry` |
| `WebSocket` | `websocket` | RFC 6455: `WebSocket::upgrade(request, handler)` for a route, plus the handshake and frame read/write. See [WebSocket](/features/websocket/) |
| `WebSocketConnection` / `WebSocketSender` / `Message` | `websocket` | An upgraded connection: `recv()` → whole `Message::Text`/`Binary` (pings, fragments and the close handshake handled); `sender()` for other threads |
| `Frame` | `websocket` | A WebSocket frame: `Text`, `Binary`, `Ping`, `Pong`, `Close`, `Continuation` |

## MCP

| Type / Trait | Module | Description |
|---|---|---|
| `Encoding` / `Level` | `compression` | Content codings (`gzip`, `br`, `zstd`) and levels; `negotiate(request, preference)`, `encode`, `apply_compression` (18.1.0). See [Compression](/features/compression/) |
| `JsonSchema` / `ObjectSchema` | `schema` | A type's JSON Schema as text (primitives, `Option`, collections, smart pointers built in); `ObjectSchema` builds an object schema (18.0.0) |
| `McpGateway` / `Upstream` | `mcp::gateway` | One MCP endpoint federating upstream MCP servers: `prefix__tool` names, per-caller `allow` rules, upstream credentials kept server-side (18.0.0). See [MCP gateway](/mcp/gateway/) |
| `InputRequired` / `PromptReply` | `mcp` | MCP 2026-07-28 multi round-trip requests: what a tool (`McpContent::input_required`) or prompt (`.prompt_with_context`) needs from the client — `.elicit`, `.sample`, `.list_roots`, `.state` (signed). See [Asking the client for input](/mcp/tools/#asking-the-client-for-input-mcp-2026-07-28) |
| `TaskContext` / `TaskError` | `mcp` | The MCP Tasks extension: what `McpContext::start_task`'s work runs with (`input`, `is_cancelled`, `status_message`) and how it ends. See [Long-running work: tasks](/mcp/tools/#long-running-work-tasks-mcp-tasks-extension) |
| `McpServer` / `McpIcon` | `mcp` | MCP Streamable HTTP server (`POST /mcp`, JSON-RPC 2.0, plus `GET /mcp` SSE); `.tool` / `.typed_tool` (`serde`) / `.resource` / `.prompt`, `.require_bearer`, `.require_bearer_tokens`, `.require_oauth` (`sso`), `.tool_filter`, `.session_ttl` / `.max_sessions` / `.max_sse_clients`. Entry point `McpServer::new(name, version)` or `app.mcp(name, version)`. See [MCP](/mcp/overview/) |
| `McpContent` | `mcp` | Tool result content: `text`, `json`, `image`, `embedded` |
| `McpContext` | `mcp` | Per-session context for `.tool_with_context` handlers: client info, `auth_claims`, progress, cancellation, `sample`, `list_roots` |
| `ToolAnnotations` | `mcp` | `read_only_hint`, `destructive_hint`, `idempotent_hint`, `open_world_hint` for `.tool_annotated` |

## Proxy

| Type / Trait | Module | Description |
|---|---|---|
| `ReverseProxy` | `proxy` | HTTP/1.1 reverse proxy middleware: round-robin with failover (502 when all fail), pooled connections, unbuffered streaming of SSE/chunked/large bodies; `.with_circuit_breaker(Arc<dyn Breaker>)`. See [Reverse proxy](/proxy/reverse-proxy/) |
| `LoadBalancing` | `proxy` | Strategy enum passed to `ReverseProxy::strategy()` |
| `ConnPool` / `Pool<S>` | `proxy` | Per-backend idle-connection pool (`ConnPool = Pool<TcpStream>`); `ConnPool::new(max_idle, idle_timeout)`, share with `ReverseProxy::with_pool`. A stale pooled connection is retried once only for idempotent methods |
| `UpstreamTls` | `proxy` | TLS settings for upstreams: a PEM CA file instead of WebPKI and/or a client certificate (`UpstreamTls::new(ca, client)`; `tls_ca_file`, `tls_client_cert` on `ReverseProxy`/`H2ReverseProxy`/`GrpcProxy`, `[[upstream]] tls_ca_file`/`tls_client_cert_file`/`tls_client_key_file`); unloadable → 502 |
| `H2ReverseProxy` | `proxy` (`http2`) | HTTP/2 upstream proxy: `h2://` plain, `h2s://`/`https://` TLS; pooled connections, bodies streamed both ways, trailers kept (18.3.0) |
| `Trailers` | `response` | Trailer fields sent after a body (`Response::trailers`, HTTP/2 and HTTP/3): a shared slot a streamed body's producer fills before it ends (18.3.0) |
| `GrpcWebLayer` | `proxy` (`http2`) | Translates gRPC-web (`application/grpc-web[-text]`) to gRPC for the layers after it and the response back, trailers as the body's last frame (18.3.0). See [gRPC-web](/proxy/grpc-proxy/#grpc-web-from-browsers) |
| `GrpcProxy` | `proxy` (`http2`) | gRPC proxy (wraps `H2ReverseProxy`, filters on `application/grpc*`). See [gRPC proxy](/proxy/grpc-proxy/) |
| `TcpProxy` | `tcp_proxy` | Standalone L4 TCP proxy with round-robin backends. See [TCP proxy](/proxy/tcp-proxy/) |
| `UdpProxy` | `udp_proxy` | Standalone request-reply UDP proxy. See [UDP proxy](/proxy/udp-proxy/) |
| `WsProxy` | `ws_proxy` | WebSocket proxy, `ws://` and `wss://` (`http-client` or `http2`); `with_live_backends` for health-aware selection (503 when empty). See [WebSocket proxy](/proxy/websocket-proxy/) |
| `CanaryLayer` | `canary` | Weighted traffic splitting (smooth weighted round-robin); `Clone` shares state, `.update()` changes weights live. See [Canary](/proxy/canary/) |
| `WeightedBackend` | `canary` | One weighted backend URL |
| `WeightedPool` | `canary` | A weighted group whose members come from a `BackendPool` |
| `CircuitBreaker` | `circuit_breaker` | Per-backend Closed/Open/HalfOpen breaker in memory; `circuit_breaker::global()`. See [Circuit breaker](/proxy/circuit-breaker/) |
| `RedisCircuitBreaker` | `circuit_breaker` | Same state machine shared through Redis; methods return `Result`; `from_env()` |
| `Breaker` | `circuit_breaker` | Object-safe trait over both breakers, taken by `ReverseProxy::with_circuit_breaker` |
| `RetryLayer` | `circuit_breaker` | Re-dispatches a request whose status is retryable (default 502/503/504); push it before the proxy |
| `BackendPool` | `service_discovery` | Backend list refreshed by a background thread. See [Service discovery](/proxy/service-discovery/) |
| `DiscoverySource` | `service_discovery` | `Static`, `EnvPrefix`, `File`, `Dns`, `DnsSrv`, `Consul`, `Docker`, `EtcdWatch` |
| `KubernetesIngressWatcher` | `ingress` | Resyncs and watches Ingress resources; `.from_service_account()` connects in-cluster (`http-client`/`http2`). See [Kubernetes Ingress](/deployment/kubernetes-ingress/) |
| `IngressRouter` | `ingress` | `Application` routing requests by the watched Ingress rules |
| `IngressRule` | `ingress` | One parsed Ingress rule |
| `PathType` | `ingress` | `Exact`, `Prefix`, `ImplementationSpecific` |
| `ProxyConfig` | `proxy_config` | Parsed `[[route]]`/`[[upstream]]` config; `ProxyConfig::is_proxy_mode()`, `.validate(&mut ConfigReport)`. See [Config-driven proxy](/proxy/config-driven/) |
| `ConfigDrivenApp` | `proxy_config` | The app `proxy_config::build_from_file()` returns: first-match router with per-route middleware and per-upstream `strategy` (`round_robin`, `random`, `ip_hash`, `least_connections`); `.with_config(ServerConfig)` |
| `StickyConfig`, `BreakerConfig`, `RetryConfig` | `proxy_config` | An `[[upstream]]`'s `[upstream.sticky]`, `[upstream.circuit_breaker]` and `[upstream.retry]` (fields of `UpstreamConfig`, beside `weights`). See [Weights, stickiness, retries and the circuit breaker](/proxy/config-driven/#weights-stickiness-retries-and-the-circuit-breaker) |
| `ReloadOutcome` | `proxy_config` | Result of `ConfigDrivenApp::reload_from_str` / `reload_from_file` (validated first; the running routes stay on errors) |

## HTTP client

| Type / Trait | Module | Description |
|---|---|---|
| `Client` | `http_client` | Synchronous HTTP/1.1 client; follows redirects, 30 s default timeout. HTTPS needs `http-client` or `http2`. See [HTTP client](/building-apps/http-client/) |
| `RequestBuilder` | `http_client` | `.header`, `.body`, `.body_text`, `.body_json`, `.form`, `.timeout_ms`, `.send()` |
| `Response` | `http_client` | `.status()`, `.is_success()`, `.header(name)`, `.bytes()`, `.text()`, `.json::<T>()` (`serde`) |
| `AsyncClient` | `http_client` (`http2`) | Async client with `.send().await` |
| `AsyncRequestBuilder` | `http_client` (`http2`) | Async request builder (same methods, including `.form`) |
| `HttpClientError` | `http_client` | Error type; implements `std::error::Error` |

## Observability

| Type / Trait | Module | Description |
|---|---|---|
| `Log` | `log` | Access log lines: `Log::json` / `Log::combined` return one line; `Log::log_access` buffers per thread; `Log::flush_access_log()` writes now. See [Observability](/deployment/observability/) |
| `OtelLayer` | `otel` | HTTP server root span, W3C `traceparent` propagation, stdout or OTLP export. See [Tracing](/features/tracing/) |
| `otel::setup` / `setup_from_env` / `setup_with_exporter` | `otel` | Initialize the global tracer once at startup (`OTEL_SERVICE_NAME`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_TRACES_SAMPLER_ARG`) |
| `otel::shutdown` / `otel::flush` | `otel` | Flush buffered spans; call `shutdown()` before exit |
| `otel::span` / `otel::client_span` | `otel` | Start an Internal / Client child span under the active span |
| `otel::current_traceparent` | `otel` | `traceparent` of the innermost active span, for outbound calls |
| `Baggage`, `otel::current_baggage` / `set_baggage` | `otel` | W3C Baggage: parse/format the header; read and extend the current request's. See [Baggage](/features/tracing/#baggage) |
| `otel::propagation_headers`, `otel::is_tracing` | `otel` | `traceparent` + `baggage` for the current span; whether a traced request is current |
| `Context`, `ContextGuard` | `otel` | Capture the current span and baggage; `attach()` it on another thread |
| `DetachedSpan`, `otel::detached_span` | `otel` | A `Send` span for async work, parented where it starts; `.enter(f)` |
| `Span` | `otel` | RAII child span: `.set_attribute`, `.set_error`, `.record_error`, ends on drop |
| `SpanKind` | `otel` | `Internal`, `Server`, `Client` |
| `AttributeValue` | `otel` | `String`, `Int`, `Float`, `Bool`, with `From` impls |
| `TracingConfig` | `otel` | Service name, exporter, sample rate, batch size |
| `ExporterConfig` | `otel` | Built-in exporter choice |
| `otel::metrics::MetricsExportConfig`, `otel::metrics::start` / `start_from_env` | `otel` | Push `http.server.request.duration` over OTLP/HTTP every interval. See [OTLP metrics export](/deployment/observability/#otlp-metrics-export) |
| `otel::parse_headers`, `otel::traces_configured` | `otel` | `OTEL_EXPORTER_OTLP_HEADERS` syntax; whether the environment asks for trace export |
| `metrics::http::observe` | `metrics` | Record a request in `http_server_request_duration_seconds` (the server core does it for every request) |
| `metrics::exposition::protobuf` / `accepts_protobuf` | `metrics` | Prometheus protobuf exposition (native histograms) and its `Accept` negotiation |
| `Exporter` | `otel` | Trait for span exporters, accepted by `setup_with_exporter` |
| `StdoutExporter` / `OtlpHttpExporter` | `otel` | Built-in exporters; `OtlpHttpExporter::new(endpoint, service_name, service_version)` |
| `TraceContext` | `otel` | Parsed `traceparent`: `TraceContext::parse`, `.as_header(span_id)` |
| `SpanData` | `otel` | A finished span ready for export |

## Database / ORM

All model-layer types need `model-sqlite`, `model-postgres` or `model-mysql` (each implies `http2`; they combine).

| Type / Trait | Module | Description |
|---|---|---|
| `DbPool` | `model` | Async `sqlx` pool, one variant per compiled-in backend: `new(DbConfig)`, `from_env()`, `memory()` (SQLite `:memory:`); `execute`, `query`, `begin`, `transaction`, `migrate`. See [Database](/database/overview/) |
| `DbTransaction` | `model` | Open transaction from `DbPool::begin`; `commit` / `rollback` |
| `DbConfig` | `model` | Connection settings; `from_env()` reads `RWS_DB_*` |
| `Backend` | `model` | `Sqlite`, `Postgres`, `MySql` (compiled-in only); `Backend::parse`; pick it explicitly only with several backends compiled in |
| `Repository<T, ID>` | `model` | Async CRUD trait (`find_by_id`, `find_all`, `save`, `delete_by_id`, `count`, …). See [Repository](/database/repository/) |
| `ModelRepository<T, ID>` | `model` | `Repository` implementation from `T::repository(&pool)`; `save` inserts when the key is 0, else updates |
| `QueryBuilder<T>` | `model` | Fluent query builder: `where_eq`, `filter`, `order_by`, `limit`, `fetch_all`, `count`, `paginate`, `paginate_after`. See [Query builder](/database/query-builder/) |
| `HasMany<T>` / `HasOne<T>` / `BelongsTo<O>` | `model` | Explicit-load relations with `.load(&pool).await` — no hidden N+1. See [Relations](/database/relations/) |
| `MigrationStatus` | `model` | Applied/pending state from `DbPool::migration_status`. See [Migrations](/database/migrations/) |
| `Page<T>` | `pagination` (re-exported from `model`) | Offset page: totals, `has_next`/`has_prev`, `.map(f)`, `.link_header(base_url)`; usable without the model layer. See [Pagination](/database/pagination/) |
| `CursorPage<T>` | `pagination` (re-exported from `model`) | Keyset page with `next_cursor`, `.link_header(base_url, cursor_param)` |

## Background work

| Type / Trait | Module | Description |
|---|---|---|
| `Scheduler` | `scheduler` | Recurring tasks: `.every(d, f)`, `.after(d, f)`, `.cron(expr, f)`. See [Scheduler](/features/scheduler/) |
| `CronSchedule` | `scheduler` | Parsed six-field cron expression (`sec min hour day month weekday`) |
| `Job` | `jobs` (`jobs`) | Job trait, implemented for `Fn() -> Result<(), String> + Send` closures. See [Jobs](/features/jobs/) |
| `JobQueue` | `jobs` (`jobs`) | In-memory worker pool with retry and exponential backoff; not crash-safe |
| `PersistentJobQueue` | `jobs` (`jobs` + a `model-*` feature) | Crash-safe queue in a `rws_jobs` table; `(job_type, payload)` jobs dispatched to registered handlers |

## Storage and mail

| Type / Trait | Module | Description |
|---|---|---|
| `Storage` | `storage` | Trait: `put`, `get`, `delete`, `url`. See [Storage](/features/storage/) |
| `StorageError` | `storage` | Error type for storage backends |
| `LocalStorage` | `storage` (`storage-local`) | Files under a root directory; rejects `..`; `.with_base_url(prefix)` |
| `S3Storage` | `storage` (`storage-s3`) | S3-compatible storage with SigV4, no AWS SDK; static keys or IRSA / ECS / IMDSv2 credentials |
| `S3Config` | `storage` (`storage-s3`) | Bucket, region, endpoint, credentials; `from_env()` |
| `AzureBlobStorage` | `storage` (`storage-azure`) | Azure Blob Storage with Shared Key signing or Managed Identity, no Azure SDK |
| `AzureBlobConfig` | `storage` (`storage-azure`) | Account, container, endpoint, credentials; `from_env()` |
| `Mailer` | `mailer` (`mailer`; STARTTLS/SMTPS also need `http-client` or `http2`) | SMTP client; `Mailer::from_env()` reads `RWS_SMTP_*`; `.send(&email)`. See [Email](/features/email/) |
| `Email` | `mailer` (`mailer`) | An RFC 5322 message; `Email::builder()` |
| `EmailBuilder` | `mailer` (`mailer`) | `.to`, `.cc`, `.bcc`, `.reply_to`, `.subject`, `.text`, `.html`, `.build()` (`multipart/alternative` when both bodies are set) |
| `SmtpTls` | `mailer` (`mailer`) | `None` (25), `Starttls` (587, default), `Smtps` (465) |
| `MailerError` | `mailer` (`mailer`) | `MissingConfig`, `Io`, `Smtp`, `Build` |

## Other

| Type / Trait | Module | Description |
|---|---|---|
| `TestClient` | `test_client` | In-process test client — dispatches requests without a TCP socket. See [TestClient](/testing/test-client/) |
| `OpenApiConfig` / `build_spec` | `openapi` (`openapi`) | OpenAPI 3.1 from registered routes; `AppWithState::openapi(config)` adds `/openapi.json` and `/docs`; `.operation(method, pattern, Operation)` adds bodies, responses and parameters. See [OpenAPI](/features/openapi/) |
| `Operation` | `openapi` (`openapi`) | One operation's summary, request body, responses, query and path parameters, with schemas from `JsonSchema` types |
| `AcmeConfig` | `acme` (`acme`) | ACME / Let's Encrypt settings; `AcmeConfig::from_env()` reads `RWS_CONFIG_ACME_*`. See [ACME](/features/acme/) |
| `AcmeManager` | `acme` (`acme`) | HTTP-01 provisioning and renewal: `.provision_until_ready().await`, `.run_renewal_loop()` |

:::note[Feature flags]
Items marked with a feature in parentheses compile only with that Cargo feature. See [Features](/getting-started/features/) and `Cargo.toml` for the full list.
:::
