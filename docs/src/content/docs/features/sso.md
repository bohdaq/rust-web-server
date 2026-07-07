---
title: OAuth2 / OIDC SSO
description: Authorization code flow with PKCE, session management, RS256/ES256 JWT verification via JWKS, and rws as its own OAuth 2.0 Authorization Server.
---

The `sso` feature adds full OAuth 2.0 / OIDC support: authorization-code + PKCE flow, session management, and asymmetric JWT verification via a live JWKS endpoint. The `sso-server` feature additionally lets `rws` act as its own OAuth 2.0 Authorization Server, issuing tokens instead of delegating to an external IdP.

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

## The outbound HTTP client behind it all

Token exchange, JWKS fetch, discovery, and UserInfo calls are all plain
[`http_client::Client`](/reference/api/) requests — the same synchronous,
dependency-free HTTP/1.1 + TLS client used everywhere else in `rws`. There is
no separate "SSO HTTP client"; `sso` just depends on the `http-client`
feature (implied automatically) for HTTPS support.

The token endpoint expects an `application/x-www-form-urlencoded` body, so
`OidcClient::exchange_code` uses `Client`'s `.form()` builder method:

```rust
use rust_web_server::http_client::Client;

let resp = Client::new()
    .post("https://oauth2.googleapis.com/token")
    .form(&[
        ("grant_type", "authorization_code"),
        ("code", "abc123"),
        ("redirect_uri", "https://example.com/auth/callback"),
        ("client_id", "my-client-id"),
        ("client_secret", "my-client-secret"),
    ])
    .send()?;
```

`.form()` percent-encodes each pair, joins them with `&`, and sets
`Content-Type: application/x-www-form-urlencoded` — available on both the
sync `RequestBuilder` and the async `AsyncRequestBuilder` (`http2` feature).

## `rws` as its own OAuth 2.0 Authorization Server

Everything above covers `rws` as an OAuth 2.0 / OIDC **client**, delegating
login to an external identity provider. The `sso-server` feature (implies
`sso` and `auth`) flips the role: `AuthServer` lets `rws` issue its own
short-lived JWTs to downstream services or single-page apps, instead of
always delegating to Google/Okta/etc.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["sso-server"] }
```

```rust
use std::sync::Arc;
use std::time::Duration;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::session::SessionStore;
use rust_web_server::sso::server::{AuthServer, AuthServerConfig};
use rust_web_server::sso::client_store::{ClientStore, OAuthClient, GrantType};

let auth_server = AuthServer::new(AuthServerConfig {
    issuer:            "https://myapp.com".into(),
    signing_secret:    std::env::var("RWS_AUTH_SIGNING_SECRET").unwrap(),
    access_token_ttl:  Duration::from_secs(3600),
    refresh_token_ttl: Duration::from_secs(86_400 * 30),
    clients: ClientStore::new()
        .add(OAuthClient {
            client_id:     "spa-frontend".into(),
            client_secret: None, // public client, uses PKCE
            redirect_uris: vec!["https://spa.example.com/callback".into()],
            grants:        vec![GrantType::AuthorizationCode],
            scopes:        vec!["openid".into(), "email".into()],
        })
        .add(OAuthClient {
            client_id:     "backend-service".into(),
            client_secret: Some("s3cr3t".into()),
            redirect_uris: vec![],
            grants:        vec![GrantType::ClientCredentials],
            scopes:        vec!["api:read".into(), "api:write".into()],
        }),
    sessions: Arc::new(SessionStore::new(86_400)),
});

let app = App::new().wrap(auth_server);
```

`AuthServer` intercepts four paths and passes everything else through:

| Path | Method | Purpose |
|---|---|---|
| `/oauth/token` | `POST` | Issues tokens for the `client_credentials`, `authorization_code` (+ PKCE), and `refresh_token` grants |
| `/oauth/authorize` | `GET` | Starts the authorization-code flow for an end user |
| `/.well-known/openid-configuration` | `GET` | Discovery document |
| `/.well-known/jwks.json` | `GET` | Always `{"keys":[]}` — see below |

A machine-to-machine client calls the token endpoint directly:

```
POST /oauth/token
grant_type=client_credentials&client_id=backend-service&client_secret=s3cr3t

→ {"access_token":"eyJ...","token_type":"Bearer","expires_in":3600}
```

:::caution[Two deviations from a textbook Authorization Server]
- **Tokens are signed HS256**, not RSA/EC from a PEM private key — this
  crate has no PEM/DER private-key parser, and HS256 is a fully adequate
  choice when the issuer and every resource server share one secret.
  `GET /.well-known/jwks.json` therefore always returns an empty key set —
  there's no public key to publish for a symmetric algorithm. Configure
  resource servers with the same `signing_secret` directly, e.g.
  `auth::JwtLayer::new(secret)` or `auth::verify_jwt(token, secret)`.
- **`/oauth/authorize` has no built-in login page.** It trusts an existing
  application session: if the browser's session (default cookie, default
  key `"user_id"`, override with `.subject_session_key()`) already
  identifies a user, a code is minted immediately and the browser is
  redirected straight to `redirect_uri?code=...`; otherwise it's redirected
  to a configurable `login_url` (default `/login`) with
  `?return_to=<original authorize URL>`. Building that login page — and
  populating the session after a successful login — is the embedding
  application's own responsibility, the same boundary `OidcAuth` draws
  around needing an *external* IdP to already exist.
:::

`ClientStore` has no `::from_env()` — a list of clients with per-client
secrets has no natural flat-env-var encoding (unlike `OidcConfig`'s
one-client-per-process shape above). `ClientStore::new().add(...)` is the
only registration path; load clients from wherever your application already
keeps them (config file, database, secret manager) and build the store once
at startup.
