# SSO — Single Sign-On Support

Covers everything needed for `rws` to act as an **OAuth 2.0 / OIDC client**
(the relying party in "Login with Google / Microsoft / Okta"), as an
**OAuth 2.0 resource server** (validate Bearer tokens issued by any IdP), and
optionally as a minimal **OAuth 2.0 Authorization Server** (issue tokens to
other services). SAML 2.0 is included for enterprise / B2B scenarios.

---

## What SSO means in practice

| Role | Who plays it | `rws` as… |
|---|---|---|
| Identity Provider (IdP) | Google, Microsoft Entra, Okta, Keycloak | upstream |
| Service Provider / Relying Party | Your web application | **client** (Phases 1–5) |
| Resource Server | API that accepts Bearer tokens | **validator** (Phase 3) |
| Authorization Server | Issues tokens to downstream services | **server** (Phase 6) |

---

## What already exists

| Existing | Location | Gap |
|---|---|---|
| HS256 JWT sign + verify | `src/auth` (`build_jwt`, `verify_jwt`) | Only symmetric keys — cannot verify RS256/ES256 tokens from external IdPs |
| `JwtLayer` middleware | `src/auth` | Same limitation: hardcoded HS256, no JWKS fetch |
| `SessionStore` | `src/session` | Ready to store authenticated identity after OIDC callback |
| Middleware pipeline | `src/middleware` | `OidcAuth` will be another `Middleware` impl |
| HTTP/1.1 TCP outbound | `src/proxy` | Can be reused for token endpoint calls; but needs a typed HTTP client wrapper |

---

## New dependencies required

| Crate | Why |
|---|---|
| `rsa` or `p256` | RS256 / ES256 public-key JWT verification |
| `base64ct` | PKCE `code_challenge` (SHA-256 + base64url) |
| `sha2` | Already present in `auth` feature; reused for PKCE |
| `quick-xml` | SAML 2.0 XML parsing (Phase 7 only) |
| HTTP client | Token endpoint, JWKS fetch, UserInfo, discovery (new `src/http_client`) |

All new deps are gated behind new Cargo features so existing builds are unaffected.

---

## Phases

### ✅ Phase 1 — Outbound HTTP Client — Done

**Already existed before this phase was explicitly worked, under different
names than this sketch used.** `src/http_client/mod.rs` predates the SSO
effort — it was built out for `ForwardAuthLayer`, `S3Storage`, and
`AzureBlobStorage`, all of which needed outbound HTTPS calls before SSO did.
By the time this phase was picked up, `JwksCache::fetch`, `OidcProvider::discover`,
and `OidcClient::exchange_code`/`fetch_user_info` were already using it for
every outbound call this phase describes (JWKS fetch, discovery, token
exchange, UserInfo) — so the actual gap was small and narrower than the
sketch above implies.

**Naming deviates from the sketch, deliberately, not by oversight:** the real
types are `Client`/`RequestBuilder`/`Response` (`src/http_client/mod.rs`), not
`HttpClient`/`HttpRequest`/`HttpResponse`. That naming predates this SSO spec
and is already used consistently elsewhere in the crate (`ForwardAuthLayer`,
storage backends, the docs). Introducing a second, spec-only name via a type
alias would add indirection with no behavioral benefit, so this phase did not
rename or alias anything. Likewise there is no separate `HttpRequest` type —
`RequestBuilder` fills that role — and no `.tls(bool)` toggle: TLS is selected
automatically from the URL scheme (`https://` vs `http://`), which is strictly
less to get wrong than a separate opt-in flag, and is what every existing SSO
call site already relied on. `.json::<T>()` on `Response` already existed
(gated on the `serde` feature) before this phase, for the same reason.

**What this phase actually added:** a `.form(&[(&str, &str)]) -> Self`
builder method on both `RequestBuilder` and `AsyncRequestBuilder`, matching
this entry's own example usage. Before this, `OidcClient::exchange_code`
hand-built its `application/x-www-form-urlencoded` body with `format!` +
its own `url_encode` helper — functionally correct, but exactly the kind of
boilerplate a typed client should absorb, and the literal shape this entry's
sketch (`.form(&[...])`) called for. `exchange_code` was refactored to use
the new `.form()` method; `url_encode` remains in `src/sso/client.rs` for the
query-string encoding `authorization_url` still needs (a different encoding
context — `.form()` is body-only).

**Tests:** 3 new tests in `src/http_client/tests.rs` (`.form()` sets the
Content-Type and encodes reserved characters correctly, including an
empty-pairs edge case) plus 3 new tests in `src/sso/tests.rs` exercising
`OidcClient::exchange_code` end-to-end against a loopback fake token
endpoint (success parses `TokenResponse` fields; a provider with no
`jwks_uri` omits `code_verifier`; a non-2xx response surfaces as an error
containing the status code) — the first tests in that file to perform any
I/O, called out explicitly in the module doc comment since the rest of that
file is deliberately pure/offline.

**Effort:** small, as this entry's own template would estimate for a mostly-
already-built dependency — the true remaining work was one builder method
plus dogfooding it in the one caller that still hand-rolled form encoding.

---

### ✅ Phase 2 — RS256 / ES256 JWT Verification and JWKS — Done (v17.92.0)

**Already implemented before this phase was explicitly worked, and essentially untested until now.** Like Phase 1, `src/sso/jwks.rs` (`JwksCache`, `OidcClaims`, `VerifyOptions`) predates this task — it was built alongside Phase 4's `OidcAuth` middleware, which already calls `JwksCache::verify_jwt` on every callback. What this phase actually added was **the test suite that proves it's correct**: before this task, `src/sso/tests.rs` had zero tests exercising real RSA/EC signature verification, JWKS parsing, or claim validation — every existing SSO test was pure string/encoding logic (PKCE, `url_encode`, provider presets). 12 new tests in a `jwks_tests` submodule generate real RSA-2048 and P-256 keypairs, sign JWTs with them (`rsa::pkcs1v15::SigningKey`/`p256::ecdsa::SigningKey`), serve the public half as JWKS JSON from a loopback fake HTTP server, and verify: RS256 success, ES256 success, tampered-signature rejection, expiry, `iat`-in-the-future rejection, leeway tolerance, issuer mismatch, audience mismatch, `aud` as a JSON array (matching one of several values), an unsupported `alg`, a malformed (non-3-part) token, and — the one genuinely load-bearing scenario — **that key rotation actually works**: a token signed with a key not yet in the cache still verifies, because a failed `try_verify` triggers exactly one `fetch()` retry before giving up. All 12 passed against the existing implementation with no code changes required, confirming Phase 2's logic was already correct.

**Naming and shape deviate from the sketch, matching Phase 1's pattern of the
sketch predating the real implementation's conventions:**
- No `JwtVerifier` type — `verify_jwt` is a method directly on `JwksCache`. A separate verifier object wrapping a `&JwksCache` reference would only add a lifetime parameter for no behavioral gain, since `JwksCache` already owns everything `verify_jwt` needs.
- No `.refresh_interval_secs()` background-refresh builder. Key rotation is **reactive**, not scheduled: a failed signature verification triggers exactly one refetch-and-retry (see `verify_jwt_refetches_and_succeeds_after_kid_miss`), which handles the actual failure mode (an IdP rotated its signing key) without a background thread, a shutdown-coordination story, or a staleness window between scheduled refreshes.
- `OidcClaims` has no `groups: Option<Vec<String>>` or `extra: HashMap<String, serde_json::Value>` fields. This module models only the standard OIDC claims (OpenID Connect Core §5.1); IdP-specific extension claims (Okta/Entra `groups`, etc.) are out of scope, and `extra` would require depositing this hand-rolled JSON parser's leftover key/value pairs into a `serde_json::Value` map — a second JSON representation alongside the parser this module already has, for a field nothing in this codebase currently reads.
- `nonce` is *not* validated inside `verify_jwt` despite this phase's own step 4 listing it alongside `exp`/`iat`/`aud`/`iss`. `OidcClaims.nonce` is extracted and returned, but the comparison against the session-stored nonce happens one layer up, in `oidc_auth::OidcAuth::callback_handler` (`src/sso/oidc_auth.rs`) — `JwksCache` has no session access and no notion of "the nonce this particular login attempt expects," so it couldn't validate it even in principle. `verify_jwt` validates everything a JWKS cache alone has enough context to check.

**Tests:** 12 new tests in `src/sso/tests.rs::jwks_tests` (listed above); the module's doc comment was updated to note this is the second departure (after `exchange_code`) from the file's otherwise network-free design, using the same loopback-`TcpListener` pattern. `cargo test --features sso` — 46 sso tests pass (34 pre-existing + 12 new).

**Effort:** small, per this entry's own estimate for what turned out to be a mostly-already-built dependency — the real work was proving correctness with real cryptographic material, not writing new verification logic.

**`OidcClaims` struct** (standard claims from OpenID Connect Core §5.1) — the fields that actually exist:

```rust
pub struct OidcClaims {
    pub sub:                String,           // unique user ID at the IdP
    pub iss:                String,           // issuer URL
    pub aud:                Vec<String>,      // audience (client IDs)
    pub exp:                u64,
    pub iat:                u64,
    pub nonce:              Option<String>,
    pub email:              Option<String>,
    pub email_verified:     Option<bool>,
    pub name:               Option<String>,
    pub given_name:         Option<String>,
    pub family_name:        Option<String>,
    pub picture:            Option<String>,
    pub locale:             Option<String>,
}
```

**Module:** `src/sso/jwks.rs`

---

### ✅ Phase 3 — OIDC Discovery — Done (v17.93.0)

**Already implemented before this phase was explicitly worked, and — like Phases 1 and 2 — its one network-dependent code path was untested.** `src/sso/discovery.rs`'s six hardcoded presets (`google`, `microsoft`, `github`, `okta`, `auth0`, `keycloak`) already had tests (`{provider}_preset_has_correct_endpoints`, pure in-memory, predating this task). `OidcProvider::discover(issuer)` — the actual `GET {issuer}/.well-known/openid-configuration` fetch + hand-rolled JSON parse — had none.

7 new tests in a `discovery_tests` submodule of `src/sso/tests.rs`, using the same loopback-fake-server pattern as the `jwks_tests`/`exchange_code` tests: full-field parse, optional fields (`userinfo_endpoint`/`end_session_endpoint`) absent → `None`, a trailing slash on `issuer` not producing a double slash before `.well-known` (`issuer.trim_end_matches('/')`), a missing required field (`token_endpoint`) surfacing as an error naming that field, a non-2xx HTTP status surfacing the status code, `issuer` itself being the one field the parser treats as optional-with-empty-default rather than a hard error (matching `parse_discovery_json`'s `.unwrap_or_default()` vs. every other field's `.ok_or_else(...)`), and a connection failure (nothing listening) surfacing a `"discovery fetch failed"` error. All 7 passed against the existing implementation with zero source changes required.

**`OidcProvider` has no `scopes_supported`/`response_types_supported` fields**, unlike this phase's own sketch. Nothing in this codebase reads either — `OidcConfig`'s own `scopes: Vec<String>` (Phase 5) is what actually drives the `scope` parameter sent to the authorization endpoint, populated from a preset default or `.scopes()`/`RWS_OIDC_SCOPES`, never from a discovery document's advertised capabilities. Parsing and storing two more array fields with no consumer would be dead data.

**Effort:** small, matching this entry's own estimate — again, a mostly-already-built dependency; the work was proving the network path with a fake server, not writing new discovery logic.

**Module:** `src/sso/discovery.rs`

---

### ✅ Phase 4 — OAuth 2.0 Authorization Code + PKCE Flow — Done (v17.94.0)

**Already implemented before this phase was explicitly worked, and — unlike Phases 1–3, which were narrow test-coverage gaps — had *zero* tests of any kind.** `src/sso/oidc_auth.rs`'s `OidcAuth` middleware (login/callback/logout interception, state/nonce/PKCE handling, session promotion, claims injection) predates this task in full. It was exercised only indirectly, by the fact that `handle_callback` calls `OidcClient::exchange_code`/`JwksCache::verify_jwt`, which Phases 1–2's tests covered in isolation — the middleware layer gluing them together (path routing, session lifecycle, CSRF/nonce checks, error surfacing) had never been run once.

19 new tests in an `oidc_auth_tests` submodule of `src/sso/tests.rs`, driving `OidcAuth::handle` (the `Middleware` trait method) directly against a real `SessionStore` and, for the callback tests, loopback fake token/JWKS/userinfo servers — the same combination of techniques from `jwks_tests` and `exchange_code`'s tests, now composed together for the first time:

- unauthenticated access redirects to login; excluded paths bypass the check entirely
- `/auth/login` generates a fresh PKCE verifier + `state` + `nonce` per call, sets a session cookie, and redirects to the provider's authorization endpoint with `code_challenge`/`state` present
- `return_to` is read from the query string or falls back to `post_login_redirect`
- `/auth/callback`: no cookie / unknown session / `state` mismatch all return `403`; a provider error (`?error=access_denied`, no `code`) surfaces as `500`; a full success round-trip (real RSA-signed `id_token` verified via a fake JWKS endpoint) stores claims, clears the four pre-auth session keys, and redirects to the saved `return_to`; a `nonce` mismatch on an otherwise-valid `id_token` returns `403`; a GitHub-style provider with no `jwks_uri` falls back to `fetch_user_info` against a fake UserInfo endpoint
- an authenticated request (session has `_oidc_claims`) passes through to the next `Application` with `X-Rws-Oidc-Claims` injected, verified by asserting on the request a capturing test `Application` actually received
- `/auth/logout` destroys the session and redirects home, with or without a cookie present
- `.login_path()`/`.callback_path()`/`.callback_path()`/`.logout_path()` overrides actually change which path is intercepted (the default `/auth/login` reverts to ordinary "redirect to login" handling once overridden)
- `OidcAuth::claims()`/`::sub()`/`::email()` read the injected header correctly, and return `None` when it's absent

All 19 passed against the existing implementation with zero source changes required. `cargo test --features sso` now runs 72 sso tests (53 + 19 new).

**The core round-trip below matches this phase's own sketch closely, with three real deviations:**
1. **No standalone `OidcAuth::login_handler`/`::callback_handler`/`::logout_handler` route functions exist**, unlike the "Usage" example's `.get("/auth/login", OidcAuth::login_handler)` wiring. `OidcAuth` intercepts all three paths *inside its own `Middleware::handle`* by comparing the request path against `self.login_path`/`callback_path`/`logout_path` — so `.wrap(OidcAuth::new(config, sessions))` alone is sufficient; registering the three routes yourself would be redundant (and the handler names the sketch references don't exist to register).
2. **Claims are not stored in "request extensions"** (step 2 of the sketch's per-request description) — this codebase has no such mechanism. `OidcClaims` is serialized to JSON and injected as a real header (`X-Rws-Oidc-Claims`), deserialized back out on demand by `OidcAuth::claims()`. This was an established pattern from before this phase (already documented in `CLAUDE.md`), not a new decision.
3. **The original URL is carried via a `return_to` query parameter on the redirect to `/auth/login`, not by pre-creating a session** — the unauthenticated-access branch that redirects to login does no session I/O at all; a pre-auth session (with `state`/`nonce`/`pkce`/`return_to`) is only created once `/auth/login` itself is actually hit.

`PkceVerifier::new()` always produces a 43-character verifier (32 random bytes, base64url, no padding) — a fixed length, not the "43–128 char" range this phase's PKCE description allows for (RFC 7636 permits up to 128; nothing in this codebase needs the extra entropy a longer verifier would add over 32 random bytes, already confirmed collision-resistant by `pkce_two_verifiers_are_different` from Phase 2's era).

**Effort:** matches this entry's own "core flow" framing — this was the first Phase where writing tests meant composing multiple already-tested pieces (PKCE, token exchange, JWKS verification, sessions) through a real middleware, rather than testing one function in isolation.

**Module:** `src/sso/oidc_auth.rs` (plus `src/sso/mod.rs`, `src/sso/pkce.rs`, both already covered by earlier phases' tests).

```
Browser                      rws                       IdP (Google, etc.)
  │                            │                            │
  │  GET /dashboard            │                            │
  │ ─────────────────────────► │                            │
  │                            │ (no session → redirect)    │
  │  302 → /auth/login         │                            │
  │ ◄───────────────────────── │                            │
  │                            │                            │
  │  GET /auth/login           │                            │
  │ ─────────────────────────► │                            │
  │  302 → IdP /authorize      │                            │
  │ ◄───────────────────────── │                            │
  │                            │                            │
  │  GET /authorize?...        │                            │
  │ ──────────────────────────────────────────────────────► │
  │  302 → /auth/callback?code │                            │
  │ ◄────────────────────────────────────────────────────── │
  │                            │                            │
  │  GET /auth/callback?code   │                            │
  │ ─────────────────────────► │                            │
  │                            │  POST /token (code, PKCE)  │
  │                            │ ─────────────────────────► │
  │                            │  {id_token, access_token}  │
  │                            │ ◄───────────────────────── │
  │                            │ (verify id_token via JWKS) │
  │                            │ (store claims in session)  │
  │  302 → /dashboard          │                            │
  │ ◄───────────────────────── │                            │
```

**Usage** (matches the actual API — no separate route registration for the
three auth paths; `OidcAuth` intercepts them itself, see the deviations
noted above):

```rust
use rust_web_server::sso::{OidcAuth, OidcConfig};

let app = App::with_state(my_state)
    .wrap(
        OidcAuth::new(
            OidcConfig::google(client_id, client_secret, "https://myapp.com/auth/callback")
                .post_login_redirect("/dashboard"),
            sessions,
        )
        .exclude("/healthz"),   // paths that bypass auth
    )
    .get("/dashboard", dashboard_handler);
    // No .get("/auth/login" | "/auth/callback" | "/auth/logout", ...) —
    // OidcAuth's Middleware::handle intercepts those three paths itself.

// Access claims inside any handler:
fn dashboard_handler(req: &Request, _: &PathParams, _: &ConnectionInfo, _: &MyState) -> Response {
    let claims = OidcAuth::claims(req).unwrap(); // Option<OidcClaims>, owned
    // claims.email, claims.name, claims.sub, etc.
}
```

**What `OidcAuth` does per request:**
1. Is the path `/auth/login`, `/auth/callback`, or `/auth/logout`? → handle it directly (see below)
2. Is the path excluded? → pass through
3. Is there a valid session with an `_oidc_claims` key? → pass through with claims injected as the `X-Rws-Oidc-Claims` request header (JSON), not "request extensions" (no such mechanism exists in this codebase)
4. No session → redirect to `/auth/login?return_to=<original-url>` (no session is created at this point — only `/auth/login` itself creates the pre-auth session)

**What handling `/auth/callback` does:**
1. Validate `state` parameter (matches what was stored to prevent CSRF)
2. POST to `token_endpoint` with `code`, `redirect_uri`, `code_verifier` (PKCE)
3. Receive `id_token` + `access_token`
4. Verify `id_token` via JWKS (Phase 2) — falls back to `fetch_user_info` when the provider has no `jwks_uri` (GitHub)
5. Verify `nonce` matches stored nonce
6. Store `OidcClaims` (as JSON) in session
7. Redirect to original `return_to` (or `post_login_redirect` if none was captured)

**PKCE implementation:**
```
code_verifier  = random 43–128 char base64url string
code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
```

Both stored in the pre-auth session; `code_verifier` sent in token exchange.

**New module:** `src/sso/mod.rs`, `src/sso/oidc_auth.rs`, `src/sso/pkce.rs`

---

### ✅ Phase 5 — Provider Presets and `OidcConfig` Builder — Done (v17.95.0)

**Already fully implemented before this phase was explicitly worked** — `src/sso/config.rs`'s six preset constructors, `::discover()`, and `::from_env()` predate this task, same as every phase before it. Unlike Phase 4 (zero tests) this one already had partial coverage: `google`/`github` were tested at the `OidcConfig` level, and all six presets were tested at the underlying `OidcProvider` level (Phase 3's `discovery_tests`) — but `microsoft`/`okta`/`auth0`/`keycloak` had no `OidcConfig`-level test, `OidcConfig::discover()` had no test at all (only the `OidcProvider::discover()` it wraps), and **`from_env()` had exactly one test — a single failure case.** The entire success path, every provider-specific required-variable branch, scope parsing, and the default/override behavior of `post_login_redirect` were unexercised.

16 new tests in a `config_tests` submodule of `src/sso/tests.rs`: `OidcConfig`-level preset tests for the four previously-uncovered providers (asserting `client_id`/`client_secret`/`redirect_uri` are plumbed through correctly, not just the provider endpoints Phase 3 already checked); `OidcConfig::discover()` success (via a loopback fake discovery document) and error-propagation; and a `from_env()` matrix — Google success with every field checked including the default scopes and `post_login_redirect`, Microsoft's `RWS_OIDC_TENANT_ID` requirement (both the missing-var error and the success path), Okta's `RWS_OIDC_ISSUER` requirement, an unrecognized/`custom` provider name discovering via a live (faked) issuer, the two base required vars (`RWS_OIDC_CLIENT_ID`, `RWS_OIDC_REDIRECT_URI`) failing by name when absent, `RWS_OIDC_CLIENT_SECRET` defaulting to empty (the PKCE-only-public-client case), custom space-separated `RWS_OIDC_SCOPES`, and a custom `RWS_OIDC_POST_LOGIN_REDIRECT`. All 16 passed against the existing implementation with zero source changes required — including three repeated runs specifically to check for `RWS_OIDC_*`-env-var races, since these are the first tests in this module to mutate process environment state.

**Every `from_env()` test holds `crate::test_env::lock()` for its full duration and clears every `RWS_OIDC_*` var it touched before returning** — `RWS_OIDC_*` vars aren't named in `CLAUDE.md`'s `RWS_CONFIG_*`-specific lock rule, but they're the same class of problem (process-wide mutable state read by a function under `cargo test`'s parallelism), so this phase applied the identical discipline rather than treating the letter of that rule as the whole of its intent. The pre-existing `oidc_config_from_env_fails_without_env_vars` test (predating this phase) did not hold the lock; it now does.

**Effort:** small, matching this entry's own estimate — once again a mostly-already-built dependency; the work was closing a test-coverage gap in a specific, already-known-risky corner (`from_env()`'s per-provider branching), not writing new preset logic.

Reduce boilerplate for the most common providers. Each preset fills in the
`OidcProvider` so only `client_id`, `client_secret`, and `redirect_uri` are
required.

```rust
// Google
OidcConfig::google(client_id, client_secret, redirect_uri)

// Microsoft Entra ID (Azure AD)
OidcConfig::microsoft(tenant_id, client_id, client_secret, redirect_uri)

// GitHub — OAuth 2.0 only (no OIDC; fetches user via /user API instead)
OidcConfig::github(client_id, client_secret, redirect_uri)

// Okta
OidcConfig::okta("mycompany.okta.com", client_id, client_secret, redirect_uri)

// Auth0
OidcConfig::auth0("mycompany.auth0.com", client_id, client_secret, redirect_uri)

// Keycloak
OidcConfig::keycloak("https://keycloak.example.com", "myrealm", client_id, client_secret, redirect_uri)

// Any OIDC-compliant provider (fetches discovery doc)
OidcConfig::discover("https://idp.example.com", client_id, client_secret, redirect_uri)
```

**Environment variable convention** (all providers; verified against `config.rs`'s actual `match` arms, which required correcting this table — `RWS_OIDC_ISSUER` is required for `okta`/`auth0`/`keycloak` too, not just `custom`, and `keycloak` additionally needs `RWS_OIDC_TENANT_ID` for its realm name):

| Variable | Description |
|---|---|
| `RWS_OIDC_PROVIDER` | One of: `google`, `microsoft`, `github`, `okta`, `auth0`, `keycloak`, `custom` |
| `RWS_OIDC_CLIENT_ID` | OAuth 2.0 client ID (required for all) |
| `RWS_OIDC_CLIENT_SECRET` | OAuth 2.0 client secret (optional; defaults to empty for public/PKCE-only clients) |
| `RWS_OIDC_REDIRECT_URI` | Callback URL registered at the IdP (required for all) |
| `RWS_OIDC_ISSUER` | Required for `okta` (domain), `auth0` (domain), `keycloak` (base URL), and `custom` (issuer URL) |
| `RWS_OIDC_TENANT_ID` | Required for `microsoft` (tenant) and `keycloak` (realm name) |
| `RWS_OIDC_SCOPES` | Space-separated; default `openid email profile` |
| `RWS_OIDC_POST_LOGIN_REDIRECT` | Default `/` |

```rust
// Load everything from env
let config = OidcConfig::from_env()?;
let app = App::new().wrap(OidcAuth::new(config, sessions)); // sessions: Arc<SessionStore>
```

---

### Phase 6 — OAuth 2.0 Authorization Server (Token Issuer)

Enables `rws` to be the IdP for downstream services or single-page apps — it
issues its own short-lived JWTs rather than delegating to an external provider.

```
POST /oauth/token
  grant_type=client_credentials & client_id=X & client_secret=Y
  → { access_token, token_type, expires_in }

POST /oauth/token
  grant_type=authorization_code & code=Z & redirect_uri=... & code_verifier=...
  → { access_token, id_token, refresh_token, expires_in }

POST /oauth/token
  grant_type=refresh_token & refresh_token=R
  → { access_token, expires_in }

GET  /oauth/authorize
  response_type=code & client_id=X & redirect_uri=... & scope=openid email
  → redirect to login page or IdP, then back with code

GET  /.well-known/openid-configuration   → discovery document
GET  /.well-known/jwks.json             → public keys for token verification
```

**Configuration:**

```rust
use rust_web_server::sso::server::{AuthServer, AuthServerConfig, ClientStore};

let auth_server = AuthServer::new(AuthServerConfig {
    issuer:              "https://myapp.com".into(),
    signing_key_pem:     env::var("RWS_AUTH_SIGNING_KEY")?,    // RSA or EC private key
    access_token_ttl:    Duration::from_secs(3600),
    refresh_token_ttl:   Duration::from_secs(86400 * 30),
    clients:             ClientStore::from_env()?,              // or ClientStore::from_db(pool)
});

let app = App::new().wrap(auth_server);
// Registers: /oauth/authorize, /oauth/token, /.well-known/openid-configuration,
//            /.well-known/jwks.json automatically
```

**`ClientStore`** — registered OAuth clients:

```rust
ClientStore::new()
    .add(OAuthClient {
        client_id:     "spa-frontend".into(),
        client_secret: None,                 // public client, uses PKCE
        redirect_uris: vec!["https://spa.example.com/callback".into()],
        grants:        vec![GrantType::AuthorizationCode],
        scopes:        vec!["openid", "email", "profile"],
    })
    .add(OAuthClient {
        client_id:     "backend-service".into(),
        client_secret: Some("secret".into()),
        redirect_uris: vec![],
        grants:        vec![GrantType::ClientCredentials],
        scopes:        vec!["api:read", "api:write"],
    })
```

**New module:** `src/sso/server.rs`, `src/sso/client_store.rs`

---

### Phase 7 — SAML 2.0 Service Provider

Enterprise and B2B SSO (Active Directory Federation Services, Okta SAML,
Google Workspace SAML). More complex than OIDC: XML-based, signature over XML
canonicalised form, metadata exchange.

```rust
use rust_web_server::sso::saml::{SamlSp, SamlConfig};

let app = App::new()
    .wrap(SamlSp::new(SamlConfig {
        sp_entity_id:   "https://myapp.com/saml/metadata".into(),
        sp_acs_url:     "https://myapp.com/saml/acs".into(),        // Assertion Consumer Service
        idp_metadata:   SamlIdpMetadata::from_url("https://idp.corp.com/metadata")?,
        // or:
        idp_metadata:   SamlIdpMetadata::from_file("idp-metadata.xml")?,
        sign_requests:  false,                // sign AuthnRequests with SP private key
        sp_private_key: None,
    }));

// Automatically registers:
// GET  /saml/metadata → SP metadata XML (give this URL to the IdP)
// GET  /saml/login    → redirect to IdP with AuthnRequest
// POST /saml/acs      → receive and validate SAML Response, establish session
// GET  /saml/logout   → SP-initiated single logout
```

**What SAML assertion validation checks:**
- XML signature over the `Response` or `Assertion` element
- `Conditions/NotBefore` and `NotOnOrAfter` time windows
- `AudienceRestriction` matches SP entity ID
- `InResponseTo` matches the AuthnRequest ID (prevents replay)
- `SubjectConfirmation` method is `urn:oasis:names:tc:SAML:2.0:cm:bearer`

**Attribute mapping** (IdP-specific names → `OidcClaims` shape):

```rust
SamlConfig {
    // …
    attribute_map: AttributeMap::new()
        .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "email")
        .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name", "name")
        .map("http://schemas.microsoft.com/ws/2008/06/identity/claims/groups", "groups"),
}
```

**Cargo feature:** `sso-saml` (adds `quick-xml` dep)

---

## Architecture Overview

```
src/sso/
  mod.rs           OidcAuth middleware + claims extractor
  oidc_auth.rs     login / callback / logout handlers; session integration
  pkce.rs          code_verifier generation, code_challenge computation
  discovery.rs     OidcProvider; fetch + parse openid-configuration
  jwks.rs          JwksCache; RS256 / ES256 public-key verification; OidcClaims
  config.rs        OidcConfig + provider presets + from_env()
  server.rs        OAuth 2.0 Authorization Server (Phase 6)
  client_store.rs  OAuthClient registry (Phase 6)
  saml/
    mod.rs         SamlSp middleware; ACS handler; metadata handler
    assertion.rs   XML parse + validate SAML Response and Assertion
    metadata.rs    parse IdP metadata XML; fetch from URL

src/http_client/
  mod.rs           HttpClient; plain + TLS (rustls); .get() / .post() / .form()
```

**Cargo features:**

| Feature | Enables |
|---|---|
| `sso` | Phases 1–5 (OIDC client, JWKS, discovery, provider presets). Implies `http2` for TLS. |
| `sso-server` | Phase 6 (Authorization Server). Implies `sso`. |
| `sso-saml` | Phase 7 (SAML 2.0 SP). Adds `quick-xml`. |

---

## Session Integration

After a successful OIDC callback or SAML assertion, `OidcAuth` writes claims
into the existing `SessionStore` under a reserved key:

```rust
// Written by OidcAuth after token verification:
session.set("_oidc_sub",   &claims.sub);
session.set("_oidc_email", &claims.email.unwrap_or_default());
session.set("_oidc_name",  &claims.name.unwrap_or_default());
// full claims JSON serialised under "_oidc_claims"

// Read by handlers:
let claims = OidcAuth::claims(request)?;   // deserialises from session
let sub    = OidcAuth::sub(request)?;      // shortcut
let email  = OidcAuth::email(request)?;
```

No new session mechanism is needed — the existing `SessionStore` + cookie
helpers handle persistence.

---

## Security Checklist

| Requirement | How it is met |
|---|---|
| PKCE (RFC 7636) | S256 challenge generated in Phase 4 `pkce.rs` |
| State parameter (CSRF) | Random state stored in pre-auth session; verified in callback |
| Nonce replay prevention | Nonce stored in session; verified in `id_token` claim |
| Token signature verification | RS256 / ES256 via JWKS (Phase 2) |
| Token expiry | `exp` checked with configurable leeway |
| Audience validation | `aud` must include `client_id` |
| Issuer validation | `iss` must match provider issuer URL |
| Secure session cookie | `SessionStore` cookie is `HttpOnly; Secure; SameSite=Lax` |
| Client secret not in URL | Sent in POST body to token endpoint only |
| JWKS key rotation | `JwksCache` re-fetches on `kid` miss + scheduled refresh |
| SAML XML signature | Verified over canonicalised form before trusting any claim |

---

## Differences from Spring Security / Keycloak Adapter

| Spring / Keycloak | rws SSO |
|---|---|
| `@EnableOAuth2Sso` + `application.yml` | `OidcAuth::new(OidcConfig::from_env()?)` |
| `SecurityContextHolder.getContext().getAuthentication()` | `OidcAuth::claims(request)` |
| `@PreAuthorize("hasRole('ADMIN')")` | Manual check on `claims.groups` in handler |
| Auto-configure from `spring.security.oauth2.*` | Auto-configure from `RWS_OIDC_*` env vars |
| Spring Session (Redis/JDBC) | Built-in in-memory `SessionStore` (swap in DB-backed store via trait) |
| Keycloak adapter XML / `keycloak.json` | `OidcConfig::keycloak(base_url, realm, …)` |
| SAML SP via `spring-security-saml2-service-provider` | `SamlSp::new(SamlConfig { … })` |

---

## Implementation Summary

| Phase | Feature | Status |
|-------|---------|--------|
| 1 | `HttpClient` — outbound TLS HTTP for token / JWKS calls | ✅ Done (v17.91.0) |
| 2 | JWKS fetch + cache; RS256 / ES256 JWT verification; `OidcClaims` | ✅ Done (v17.92.0) |
| 3 | OIDC discovery; `OidcProvider` struct; named presets | ✅ Done (v17.93.0) |
| 4 | OAuth 2.0 Authorization Code + PKCE flow; `OidcAuth` middleware | ✅ Done (v17.94.0) |
| 5 | Provider presets (Google, Microsoft, GitHub, Okta, Auth0, Keycloak); `from_env()` | ✅ Done (v17.95.0) |
| 6 | OAuth 2.0 Authorization Server; `/oauth/token`; `/.well-known/*` | Pending |
| 7 | SAML 2.0 SP; ACS handler; XML signature verification; attribute mapping | Pending |
