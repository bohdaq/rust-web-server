[Read Me](../README.md) > [Spec](.) > Gaps V2

# Gaps V2 — What Is Left for a Self-Sufficient Framework

`GAPS.md` tracked server and proxy gaps against nginx/Traefik/Caddy. All of those are now closed. This document tracks what is missing for `rws` to build a complete production application — a SaaS, an API service, or a consumer web app — without reaching outside the framework.

---

## Critical — nearly every real app needs these

### 1. Outbound HTTP client ✅ Done

**The most painful missing piece.** There is no ergonomic way to call an external API from a handler. The proxy modules use raw TCP internally, but application code needs:

```rust
let resp = http::get("https://api.stripe.com/v1/charges")
    .header("Authorization", format!("Bearer {}", key))
    .json(&payload)?
    .send()?;
let charge: Charge = resp.json()?;
```

Without this, every handler that calls an external service (payment processor, AI provider, SMS gateway, internal microservice) must open a raw `TcpStream` and speak HTTP manually — a multi-page exercise for what should be one line.

**What to add:** `src/http_client/mod.rs` — a minimal synchronous HTTP/1.1 + HTTPS client backed by `rustls` (already in the dep tree for TLS). No new transport dependency needed. API:

```rust
use rust_web_server::http_client::{Client, Response};

let client = Client::new();
let resp: Response = client
    .get("https://api.example.com/users")
    .header("Authorization", "Bearer tok_…")
    .timeout_ms(5000)
    .send()?;

let body: String = resp.text()?;
let json: serde_json::Value = resp.json()?;  // requires "serde" feature
let status: u16 = resp.status();
```

For async handlers (`http2` feature), an `AsyncClient` that awaits with `tokio::net`.

**Feature flag:** none for the sync client (works in `http1`); `async-client` or covered by `http2` for the async variant.

---

### 2. Password hashing ✅ Done

Every app with user accounts needs to hash passwords at registration and verify them at login. There is no helper for this today. Handlers that need it must add `bcrypt` or `argon2` directly to `Cargo.toml` and wire them into state manually.

**What to add:** `src/crypto/mod.rs` — thin wrappers around industry-standard algorithms, gated on a `crypto` feature:

```rust
use rust_web_server::crypto::{hash_password, verify_password};

// At registration:
let hash = hash_password("hunter2")?;          // Argon2id, random salt, opaque string
db.save_user(User { password_hash: hash, … })?;

// At login:
let ok = verify_password("hunter2", &stored_hash)?;
if !ok { return Err(AppError::Unauthorized); }
```

Also expose `generate_token(n_bytes) -> String` (hex-encoded CSPRNG bytes) for password-reset tokens, email verification codes, and API keys.

**Dependency:** `argon2` crate (RustCrypto family, already used by `sha2` / `hmac` in `auth` feature). Alternatively `bcrypt` for wider tooling support. Argon2id is the current OWASP recommendation.

---

### 3. CSRF protection

Any web app that serves HTML forms is vulnerable to cross-site request forgery without a mitigation. There is no built-in CSRF token helper today.

**What to add:** `src/csrf/mod.rs` — double-submit cookie pattern:

```rust
use rust_web_server::csrf::{CsrfLayer, CsrfToken};

// Middleware — validates CSRF on mutating methods (POST/PUT/PATCH/DELETE):
let app = App::new().wrap(CsrfLayer::new(secret));

// In a GET handler — embed the token in the HTML form:
let token: CsrfToken = CsrfToken::from_request(&req)?;
// <input type="hidden" name="_csrf" value="{{ token }}">

// CsrfLayer reads the cookie and the form field / X-CSRF-Token header and
// returns 403 if they don't match.
```

Implementation: HMAC-SHA256 over a random session-scoped nonce, stored in a `SameSite=Strict; HttpOnly` cookie. Constant-time comparison. Zero new dependencies beyond `hmac` + `sha2` (already in the `auth` feature).

---

### 4. OAuth2 / SSO

`spec/SSO.md` is written but nothing is implemented. Social login (Google, GitHub) and enterprise SSO (SAML, OpenID Connect) are expected in production apps in 2026. Currently every app that wants "Sign in with Google" must implement the OAuth2 authorization-code flow from scratch.

**What to add:** `src/oauth2/mod.rs` — authorization-code + PKCE flow:

```rust
use rust_web_server::oauth2::{OAuthClient, Provider};

// Configuration
let client = OAuthClient::new(Provider::Google {
    client_id:     env::var("GOOGLE_CLIENT_ID")?,
    client_secret: env::var("GOOGLE_CLIENT_SECRET")?,
    redirect_uri:  "https://myapp.com/auth/callback".into(),
});

// GET /auth/login — redirect user to provider
fn login(_req: &Request, _p: &PathParams, _c: &ConnectionInfo, state: &AppState) -> Response {
    let (url, pkce_verifier) = state.oauth.authorization_url();
    // store pkce_verifier in session
    Response::redirect(&url)
}

// GET /auth/callback?code=…&state=…
fn callback(req: &Request, _p: &PathParams, _c: &ConnectionInfo, state: &AppState) -> Response {
    let code = Query::from_request(req)?.get("code").unwrap_or_default();
    let token = state.oauth.exchange_code(code, &pkce_verifier)?;
    let user_info = state.oauth.user_info(&token)?;
    // create session, redirect to app
}
```

**Phases:**
1. Authorization-code + PKCE flow (generic, works with any provider)
2. Built-in providers: Google, GitHub, Microsoft
3. OpenID Connect ID token parsing and verification
4. SAML 2.0 (enterprise; separate feature flag, adds `xmlsec` or equivalent)

**Feature flag:** `oauth2` (implies `http2` for async token exchange; sync path available for `http1`).

---

## Important — significant friction without these

### 5. Email (SMTP client)

Password reset, email verification, transactional notifications — all require sending email. Today there is no path to do this from a handler. Users must add `lettre` to `Cargo.toml` and initialize it themselves.

**What to add:** `src/mailer/mod.rs` — thin wrapper around an SMTP connection, gated on a `mailer` feature (`lettre` or a minimal SMTP handshake from scratch):

```rust
use rust_web_server::mailer::{Mailer, Email};

let mailer = Mailer::from_env()?;  // reads RWS_SMTP_HOST/PORT/USER/PASSWORD/FROM

mailer.send(Email {
    to:      "alice@example.com".into(),
    subject: "Reset your password".into(),
    html:    format!("<a href=\"{}\">Click here</a>", reset_url),
    text:    Some(format!("Visit: {}", reset_url)),
})?;
```

Environment variables: `RWS_SMTP_HOST`, `RWS_SMTP_PORT` (default 587), `RWS_SMTP_USER`, `RWS_SMTP_PASSWORD`, `RWS_SMTP_FROM`, `RWS_SMTP_TLS` (`"starttls"` / `"tls"` / `"none"`).

---

### 6. File storage abstraction

`FormMultipartData::parse()` hands back raw bytes for uploaded files. There is no abstraction for where those bytes go: local disk, S3-compatible object storage (AWS S3, Cloudflare R2, MinIO), or GCS. Users write `std::fs::write` directly today, which is fine for single-node but not multi-instance.

**What to add:** `src/storage/mod.rs` — a `Storage` trait with two built-in implementations:

```rust
pub trait Storage: Send + Sync {
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError>;
    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    fn delete(&self, key: &str) -> Result<(), StorageError>;
    fn url(&self, key: &str) -> String;
}

// Local disk
let store = LocalStorage::new("/var/data/uploads");

// S3-compatible (AWS S3, R2, MinIO)
let store = S3Storage::from_env()?;  // reads RWS_S3_BUCKET, RWS_S3_ENDPOINT, RWS_S3_REGION, RWS_S3_ACCESS_KEY, RWS_S3_SECRET_KEY
```

`S3Storage` uses the outbound HTTP client (item 1) to sign and send `PUT`/`GET`/`DELETE` requests to the S3 REST API — no AWS SDK dependency needed.

**Feature flags:** `storage-local` (no new deps), `storage-s3` (requires `http-client` + `crypto`).

---

### 7. Background job queue

`Scheduler` handles periodic/cron tasks. It does not handle ad-hoc jobs enqueued by request handlers — "send this email after the user registers", "resize this image in the background", "charge this card at end of trial". These need a queue with:

- Enqueue from any handler thread
- Worker pool drains the queue in the background
- Retry on failure with backoff
- Dead-letter queue for permanently failing jobs
- Job status visibility

**What to add:** `src/jobs/mod.rs` — in-process queue backed by `Mutex<VecDeque<Box<dyn Job>>>`:

```rust
pub trait Job: Send + 'static {
    fn run(&self) -> Result<(), JobError>;
    fn max_retries(&self) -> u32 { 3 }
    fn retry_delay_secs(&self) -> u64 { 30 }
}

// At startup:
let queue = JobQueue::new(worker_threads: 4);
queue.start();

// In a handler (state: Arc<AppState> where AppState has queue: JobQueue):
state.queue.enqueue(SendWelcomeEmail { user_id: new_user.id });
state.queue.enqueue_delayed(ChargeTrial { user_id }, Duration::from_secs(86400 * 14));
```

For persistence across restarts, a `PersistentJobQueue` backed by the model layer (SQLite/PostgreSQL) — stores jobs in a `rws_jobs` table, survives process crashes.

---

### 8. OpenAPI / Swagger

No automatic API schema generation from routes. API consumers have to read source code or maintain hand-written specs. In a team environment this becomes a sync problem.

**What to add:** `src/openapi/mod.rs` — schema builder and a `GET /openapi.json` route:

```rust
let app = routes! {
    App::with_state(db),
    GET  "/users"     => list_users,
    POST "/users"     => create_user,
}
.openapi(OpenApiConfig {
    title:   "My API".into(),
    version: "1.0.0".into(),
});
// Serves GET /openapi.json and GET /docs (Swagger UI via CDN)
```

`#[route]` attribute macros (`macros` feature) already annotate handlers with method + path — the OpenAPI builder reads those plus `#[derive(Validate)]` schema annotations to generate the spec. Schema extraction for request/response types requires `serde` feature.

**Feature flag:** `openapi`.

---

## Lower priority — framework-complete but not blockers

### 9. Admin UI

Already in `spec/IDEAS.md`. A `GET /admin` page showing live config, metrics, rate-limiter state, and a reload button. Zero new Cargo deps. Gate behind `BasicAuthLayer`.

### 10. i18n / localization

No string translation helpers. Apps targeting multiple locales must add a translation crate themselves. Could be a thin `src/i18n/mod.rs` that loads `locales/*.toml` files and resolves `Accept-Language` in a request context.

### 11. GraphQL

Increasingly expected as an alternative to REST. Would require a separate `src/graphql/` module integrating with `async-graphql` or `juniper`. Lower priority since the REST story is complete and GraphQL is still niche in Rust.

---

## Summary

| Gap | Priority | Effort | New deps? |
|---|---|---|---|
| ~~Outbound HTTP client~~ ✅ | ~~Critical~~ | ~~Medium~~ | ~~No (reuse rustls)~~ |
| Password hashing | Critical | Small | `argon2` |
| CSRF protection | Critical | Small | No (reuse hmac/sha2) |
| OAuth2 / SSO | Critical | Large | Minimal |
| Email (SMTP) | Important | Small | `lettre` or scratch |
| File storage | Important | Medium | No (S3 via HTTP client) |
| Background job queue | Important | Medium | No |
| OpenAPI / Swagger | Important | Medium | No |
| Admin UI | Nice | Small | No |
| i18n | Nice | Small | Probably none |
| GraphQL | Nice | Large | `async-graphql` |

**Shortest path to self-sufficient:** items 1–3 (HTTP client, password hashing, CSRF) are small-to-medium in scope, have no new external dependencies, and eliminate the most friction for a typical SaaS or API service. Implement in that order.
