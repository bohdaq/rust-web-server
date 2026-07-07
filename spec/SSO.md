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

### Phase 2 — RS256 / ES256 JWT Verification and JWKS

The existing `JwtLayer` only verifies HS256 (symmetric secret). IdPs
(Google, Microsoft, Okta) sign tokens with RSA or EC private keys and publish
the matching public keys at a JWKS endpoint.

```rust
use rust_web_server::sso::jwks::{JwksCache, JwtVerifier};

// Fetch and cache keys from the IdP JWKS endpoint
let cache = JwksCache::new("https://www.googleapis.com/oauth2/v3/certs")
    .refresh_interval_secs(3600);        // background refresh

// Verify a token from a request
let verifier = JwtVerifier::from_cache(&cache);
let claims: OidcClaims = verifier.verify(
    &token,
    VerifyOptions {
        audience:  "my-client-id.apps.googleusercontent.com",
        issuer:    "https://accounts.google.com",
        leeway_secs: 30,
    },
)?;
println!("{} {}", claims.sub, claims.email.unwrap_or_default());
```

**What JWKS verification does:**
1. Download `jwks_uri` → parse array of JWK objects (n, e for RSA; x, y for EC)
2. Match `kid` header from the incoming JWT to the cached key
3. Reconstruct the public key and verify the JWT signature (RS256 or ES256)
4. Validate `exp`, `iat`, `aud`, `iss`, `nonce`

**`OidcClaims` struct** (standard claims from OpenID Connect Core §5.1):

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
    pub groups:             Option<Vec<String>>,  // non-standard; Okta / Entra
    pub extra:              HashMap<String, serde_json::Value>,
}
```

**New module:** `src/sso/jwks.rs`

---

### Phase 3 — OIDC Discovery

Every compliant OIDC provider publishes a metadata document at
`{issuer}/.well-known/openid-configuration`. Fetching it once gives all
endpoint URLs so they do not need to be hard-coded.

```rust
use rust_web_server::sso::discovery::OidcProvider;

// Discover all endpoints from the issuer URL
let provider = OidcProvider::discover("https://accounts.google.com")?;

println!("{}", provider.authorization_endpoint);  // https://accounts.google.com/o/oauth2/v2/auth
println!("{}", provider.token_endpoint);          // https://oauth2.googleapis.com/token
println!("{}", provider.jwks_uri);               // https://www.googleapis.com/oauth2/v3/certs
println!("{}", provider.userinfo_endpoint.unwrap());

// Or use a named preset — no HTTP call needed for discovery:
let provider = OidcProvider::google();
let provider = OidcProvider::microsoft("my-tenant-id");
let provider = OidcProvider::github();    // OAuth 2.0 only, no OIDC discovery
let provider = OidcProvider::okta("mycompany.okta.com");
let provider = OidcProvider::auth0("mycompany.auth0.com");
let provider = OidcProvider::keycloak("https://keycloak.example.com", "myrealm");
```

**`OidcProvider` fields:**

```rust
pub struct OidcProvider {
    pub issuer:                     String,
    pub authorization_endpoint:     String,
    pub token_endpoint:             String,
    pub jwks_uri:                   String,
    pub userinfo_endpoint:          Option<String>,
    pub end_session_endpoint:       Option<String>,   // logout URL
    pub scopes_supported:           Vec<String>,
    pub response_types_supported:   Vec<String>,
}
```

**New module:** `src/sso/discovery.rs`

---

### Phase 4 — OAuth 2.0 Authorization Code + PKCE Flow

The core SSO client flow. Handles the full round-trip: redirect → callback →
token exchange → session establishment.

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

**Usage:**

```rust
use rust_web_server::sso::{OidcAuth, OidcConfig};

let app = App::with_state(my_state)
    .wrap(
        OidcAuth::new(OidcConfig {
            provider:      OidcProvider::google(),
            client_id:     env::var("GOOGLE_CLIENT_ID")?,
            client_secret: env::var("GOOGLE_CLIENT_SECRET")?,
            redirect_uri:  "https://myapp.com/auth/callback".into(),
            scopes:        vec!["openid", "email", "profile"],
            post_login_redirect: "/dashboard".into(),
        })
        .exclude("/healthz")    // paths that bypass auth
        .exclude("/auth/"),
    )
    .get("/dashboard", dashboard_handler)
    .get("/auth/login",    OidcAuth::login_handler)    // built-in
    .get("/auth/callback", OidcAuth::callback_handler) // built-in
    .get("/auth/logout",   OidcAuth::logout_handler);  // built-in

// Access claims inside any handler:
fn dashboard_handler(req: &Request, _: &PathParams, _: &ConnectionInfo, _: &MyState) -> Response {
    let claims: &OidcClaims = OidcAuth::claims(req).unwrap();
    // claims.email, claims.name, claims.sub, etc.
}
```

**What `OidcAuth` does per request:**
1. Is the path excluded? → pass through
2. Is there a valid session with `oidc_claims` key? → pass through with claims injected into request extensions
3. No session → store original URL in temporary session → redirect to `/auth/login`

**What `OidcAuth::callback_handler` does:**
1. Validate `state` parameter (matches what was stored to prevent CSRF)
2. POST to `token_endpoint` with `code`, `redirect_uri`, `code_verifier` (PKCE)
3. Receive `id_token` + `access_token`
4. Verify `id_token` via JWKS (Phase 2)
5. Verify `nonce` matches stored nonce
6. Store `OidcClaims` in session
7. Redirect to original URL (or `post_login_redirect`)

**PKCE implementation:**
```
code_verifier  = random 43–128 char base64url string
code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
```

Both stored in the pre-auth session; `code_verifier` sent in token exchange.

**New module:** `src/sso/mod.rs`, `src/sso/oidc_auth.rs`, `src/sso/pkce.rs`

---

### Phase 5 — Provider Presets and `OidcConfig` Builder

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

**Environment variable convention** (all providers):

| Variable | Description |
|---|---|
| `RWS_OIDC_PROVIDER` | One of: `google`, `microsoft`, `github`, `okta`, `auth0`, `keycloak`, `custom` |
| `RWS_OIDC_CLIENT_ID` | OAuth 2.0 client ID |
| `RWS_OIDC_CLIENT_SECRET` | OAuth 2.0 client secret |
| `RWS_OIDC_REDIRECT_URI` | Callback URL registered at the IdP |
| `RWS_OIDC_ISSUER` | Required for `custom` provider |
| `RWS_OIDC_TENANT_ID` | Required for `microsoft` provider |
| `RWS_OIDC_SCOPES` | Space-separated; default `openid email profile` |
| `RWS_OIDC_POST_LOGIN_REDIRECT` | Default `/` |

```rust
// Load everything from env
let config = OidcConfig::from_env()?;
let app = App::new().wrap(OidcAuth::new(config));
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
| 2 | JWKS fetch + cache; RS256 / ES256 JWT verification; `OidcClaims` | Pending |
| 3 | OIDC discovery; `OidcProvider` struct; named presets | Pending |
| 4 | OAuth 2.0 Authorization Code + PKCE flow; `OidcAuth` middleware | Pending |
| 5 | Provider presets (Google, Microsoft, GitHub, Okta, Auth0, Keycloak); `from_env()` | Pending |
| 6 | OAuth 2.0 Authorization Server; `/oauth/token`; `/.well-known/*` | Pending |
| 7 | SAML 2.0 SP; ACS handler; XML signature verification; attribute mapping | Pending |
