---
title: OAuth2 / OIDC SSO
description: Authorization code flow with PKCE, session management, and RS256/ES256 JWT verification via JWKS.
---

The `sso` feature adds full OAuth 2.0 / OIDC support: authorization-code + PKCE flow, session management, and asymmetric JWT verification via a live JWKS endpoint.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["sso"] }
```

## Quick start

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::session::SessionStore;
use rust_web_server::sso::{OidcAuth, OidcConfig};

let sessions = Arc::new(SessionStore::new(86_400)); // 24 h TTL
let config   = OidcConfig::google(
    "my-client-id",
    "my-client-secret",
    "https://example.com/auth/callback",
);
let app = App::new().wrap(OidcAuth::new(config, sessions));
```

## Loading config from environment variables

`OidcConfig::from_env()` reads environment variables so credentials stay out of source code:

```bash
RWS_OIDC_PROVIDER=google          # google | github | microsoft | okta | auth0 | keycloak | custom
RWS_OIDC_CLIENT_ID=my-client-id
RWS_OIDC_CLIENT_SECRET=my-secret
RWS_OIDC_REDIRECT_URI=https://example.com/auth/callback
RWS_OIDC_SCOPES=openid email profile   # space-separated; default: openid email profile
RWS_OIDC_POST_LOGIN_REDIRECT=/dashboard
```

Provider-specific extras:

| Provider | Extra variable | Meaning |
|---|---|---|
| `microsoft` | `RWS_OIDC_TENANT_ID` | Tenant GUID, `common`, `organizations`, or `consumers` |
| `okta` | `RWS_OIDC_ISSUER` | Okta org domain, e.g. `dev-12345.okta.com` |
| `auth0` | `RWS_OIDC_ISSUER` | Auth0 domain, e.g. `myapp.us.auth0.com` |
| `keycloak` | `RWS_OIDC_ISSUER` + `RWS_OIDC_TENANT_ID` | Base URL + realm name |
| `custom` | `RWS_OIDC_ISSUER` | OIDC issuer URL for auto-discovery |

```rust
let config = OidcConfig::from_env().expect("OIDC config missing");
```

## Provider presets

All presets accept `(client_id, client_secret, redirect_uri)` unless noted.

```rust
// Google
let cfg = OidcConfig::google("id", "secret", "https://example.com/auth/callback");

// Microsoft Entra ID / Azure AD
let cfg = OidcConfig::microsoft("common", "id", "secret", "https://example.com/auth/callback");

// GitHub OAuth 2.0 (no id_token — uses UserInfo endpoint instead)
let cfg = OidcConfig::github("id", "secret", "https://example.com/auth/callback");

// Okta
let cfg = OidcConfig::okta("dev-12345.okta.com", "id", "secret", "https://example.com/auth/callback");

// Auth0
let cfg = OidcConfig::auth0("myapp.us.auth0.com", "id", "secret", "https://example.com/auth/callback");

// Keycloak
let cfg = OidcConfig::keycloak(
    "https://keycloak.example.com",
    "my-realm",
    "id", "secret",
    "https://example.com/auth/callback",
);

// Auto-discovery from any OIDC-compliant provider
let cfg = OidcConfig::discover("https://provider.example.com", "id", "secret", "https://example.com/auth/callback")?;
```

## Authorization-code + PKCE flow

```
Browser                App                         Identity Provider
  |                     |                                   |
  |-- GET /any-page --> |                                   |
  |                     | (no session)                      |
  |<-- 302 /auth/login--|                                   |
  |                     |                                   |
  |-- GET /auth/login ->|                                   |
  |                     | generate PKCE verifier+challenge, |
  |                     | state, nonce → save in session    |
  |<-- 302 idp/authorize|                                   |
  |                                                         |
  |------- GET idp/authorize?code_challenge=... ----------->|
  |<--------------------- 302 /auth/callback?code=... ------|
  |                                                         |
  |-- GET /auth/callback?code=... -->|                      |
  |                     | exchange code (with PKCE verifier)|
  |                     |<-------- POST idp/token ----------|
  |                     |----------- id_token+access_token->|
  |                     | verify id_token via JWKS          |
  |                     | store OidcClaims in session       |
  |<-- 302 /dashboard --|                                   |
```

`OidcAuth` intercepts three paths automatically:

| Path | Action |
|---|---|
| `GET /auth/login` | Generates PKCE + state + nonce, redirects to provider |
| `GET /auth/callback` | Validates state, exchanges code, verifies id_token, stores claims |
| `GET /auth/logout` | Destroys session, redirects to `/` |

All other paths check the session. Authenticated requests have claims injected into the `X-Rws-Oidc-Claims` header (JSON) and are forwarded to the next layer. Unauthenticated requests redirect to `/auth/login?return_to=<current-path>`.

## Claims extraction in handlers

```rust
use rust_web_server::sso::OidcAuth;
use rust_web_server::request::Request;
use rust_web_server::response::Response;

fn dashboard(req: &Request) -> Response {
    // Read the full claims object
    if let Some(claims) = OidcAuth::claims(req) {
        let user_id = &claims.sub;
        let email   = claims.email.as_deref().unwrap_or("unknown");
        let name    = claims.name.as_deref().unwrap_or("unknown");
        println!("User {user_id} ({email}) / {name}");
    }

    // Shortcuts
    let sub   = OidcAuth::sub(req);
    let email = OidcAuth::email(req);

    Response::new()
}
```

### `OidcClaims` fields

| Field | Type | Description |
|---|---|---|
| `sub` | `String` | Subject (unique user ID at the provider) |
| `iss` | `String` | Issuer URL |
| `aud` | `Vec<String>` | Intended audience (your client ID) |
| `exp` | `u64` | Expiration as Unix seconds |
| `iat` | `u64` | Issued-at as Unix seconds |
| `nonce` | `Option<String>` | Nonce for replay protection |
| `email` | `Option<String>` | User's email address |
| `email_verified` | `Option<bool>` | Whether the email is verified |
| `name` | `Option<String>` | Full display name |
| `given_name` | `Option<String>` | First name |
| `family_name` | `Option<String>` | Last name |
| `picture` | `Option<String>` | Profile picture URL |
| `locale` | `Option<String>` | User's locale |

## Public path exclusion

Exclude paths from the authentication check (health endpoints, public assets, etc.):

```rust
let app = App::new()
    .wrap(
        OidcAuth::new(config, sessions)
            .exclude("/healthz")
            .exclude("/public/")
    );
```

## Custom paths

```rust
OidcAuth::new(config, sessions)
    .login_path("/login")
    .callback_path("/oauth/callback")
    .logout_path("/logout")
```

## RS256/ES256 JWT verification via JWKS

`JwksCache` fetches the provider's public keys from the JWKS URI and verifies `id_token` JWTs. Key rotation is handled automatically: on a verification failure the cache is refreshed and the verification is retried once.

```rust
use rust_web_server::sso::{JwksCache, VerifyOptions};

let jwks = JwksCache::new("https://accounts.google.com/.well-known/jwks");
let opts = VerifyOptions {
    audience:    "my-client-id",
    issuer:      "https://accounts.google.com",
    leeway_secs: 60,
};
let claims = jwks.verify_jwt(&id_token, &opts)?;
```

`OidcAuth` calls `JwksCache::verify_jwt` internally; you only need to use it directly when processing tokens outside the middleware (e.g. in a mobile API that receives tokens from a separate auth service).

:::note[GitHub does not issue id_tokens]
GitHub's OAuth 2.0 implementation does not return an `id_token`. `OidcAuth` detects this and falls back to the UserInfo endpoint to fetch claims.
:::
