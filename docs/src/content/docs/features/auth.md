---
title: Authentication
description: HTTP Basic Auth, HS256/RS256/ES256 JWT, and forward-auth middleware with helper functions for issuing and verifying tokens.
---

The `auth` Cargo feature adds `BasicAuthLayer`, `JwtLayer`, and `ForwardAuthLayer` middleware, plus standalone helpers for building and verifying HS256 JWTs. The `auth-asymmetric` feature (on top of `auth`) adds `JwtLayer::rs256`/`::es256` for verifying RS256/ES256 tokens against a static public key.

```toml
[dependencies]
rust-web-server = { version = "18", features = ["auth"] }
# or, for RS256/ES256 support too:
rust-web-server = { version = "18", features = ["auth-asymmetric"] }
```

The `auth` feature pulls in `hmac` and `sha2` (RustCrypto) as additional dependencies. `auth-asymmetric` additionally pulls in `p256` (also RustCrypto) for ES256; RS256 is verified in-crate (`rust_web_server::rsa_key::RsaPublicKey`), with no `rsa` crate.

## HTTP Basic Auth

`BasicAuthLayer` validates `Authorization: Basic <base64>` credentials against a caller-supplied closure. It issues `401 Unauthorized` with a `WWW-Authenticate: Basic realm="Protected"` challenge when the header is absent or malformed.

```rust
use rust_web_server::app::App;
use rust_web_server::auth::BasicAuthLayer;
use rust_web_server::core::New;

let app = App::new()
    .wrap(BasicAuthLayer::new(|user, pass| {
        user == "admin" && pass == "s3cret"
    }));
```

The closure signature is `Fn(&str, &str) -> bool + Send + Sync + 'static`. Passwords containing `:` are handled correctly — only the first `:` splits username from password, per RFC 7617.

Credential check outcomes:

| Condition | Response |
|---|---|
| `Authorization` header absent or malformed | `401` + `WWW-Authenticate` challenge |
| Header present but closure returns `false` | `401` (no challenge) |
| Closure returns `true` | Request forwarded to next layer |

### Validating against an htpasswd-style file

`BasicAuthLayer::from_htpasswd_file(path)` loads credentials from a file instead of a closure — the file is read once, at construction time:

```rust
use rust_web_server::app::App;
use rust_web_server::auth::BasicAuthLayer;
use rust_web_server::core::New;

let app = App::new()
    .wrap(BasicAuthLayer::from_htpasswd_file(".htpasswd").expect("failed to read htpasswd file"));
```

Each non-empty, non-comment (`#`-prefixed) line is `username:credential`, where `credential` is one of:

- a plain-text password (Apache's `htpasswd -p` format), or
- `{SHA256}` followed by the base64-encoded SHA-256 digest of the password — **rws's own scheme**, not Apache's.

```
# .htpasswd
alice:s3cret
bob:{SHA256}9S+9MrKzuG/4jvbEkGKChfSCrxXdyylUH5S89Saj9sc=
```

Generate a `{SHA256}` entry with `openssl` (no need to write Rust code):

```bash
printf '%s' 'hunter2' | openssl dgst -sha256 -binary | openssl base64
# -> 9S+9MrKzuG/4jvbEkGKChfSCrxXdyylUH5S89Saj9sc=
```

:::caution[Not compatible with real Apache htpasswd files]
Apache's actual `{SHA}` scheme is SHA-1 (not SHA-256), and modern `htpasswd` tool versions default to bcrypt or `$apr1$` (iterated MD5) when no flag is given. None of `{SHA}`, `$apr1$`, or bcrypt are supported here — this crate has no third-party crypto dependencies beyond the audited RustCrypto hash crates it already uses (`hmac`, `sha2`), and hand-rolling SHA-1, MD5, or bcrypt from scratch isn't a risk worth taking for an authentication check. A real Apache-generated htpasswd file will **not** verify against `from_htpasswd_file`.

If you need genuine Apache-hash compatibility, use [`BasicAuthLayer::new`](#http-basic-auth) with your own closure backed by the `bcrypt`/`sha1` crate of your choice, or regenerate the file with `htpasswd -p` (plain text) or the `{SHA256}` scheme above.
:::

## JWT — HS256

`JwtLayer` verifies `Authorization: Bearer <token>` JWTs signed with HMAC-SHA256.

```rust
use rust_web_server::app::App;
use rust_web_server::auth::JwtLayer;
use rust_web_server::core::New;

let app = App::new()
    .wrap(JwtLayer::new(b"my-signing-secret"));
```

Tokens with a past `exp` claim are rejected. Any token that fails format validation, algorithm check, or signature verification returns `401 Unauthorized`.

### Rotating the secret

Several secrets can verify at once, so a secret can be replaced without
logging everyone out (since 18.20.0):

```rust,ignore
// Sign new tokens with the new secret; the old one verifies until its tokens expire.
let layer = JwtLayer::secrets([new_secret, old_secret]);
// Or name them: a token whose header says "kid": "2026-11" is checked against that secret only.
let layer = JwtLayer::new(old_secret).secret_with_kid("2026-11", new_secret);
```

In the config file, `previous_secret_env` adds the outgoing secret:

```toml
[route.middleware.auth]
type = "jwt"
secret_env = "JWT_SECRET"               # the new secret
previous_secret_env = "JWT_SECRET_OLD"  # still accepted; remove after the longest token lifetime
```

### Building tokens

Use `build_jwt` in a login handler to issue a signed token:

```rust
use rust_web_server::auth::build_jwt;

fn login_handler(/* ... */) -> String {
    let claims = r#"{"sub":"42","role":"admin","exp":9999999999}"#;
    let token = build_jwt(claims, b"my-signing-secret");
    // return token to client
    token
}
```

`build_jwt(claims_json, secret)` always produces an HS256 JWT with the `alg` and `typ` header set. The `claims_json` argument is any valid JSON object string.

### The caller as an identity

`JwtLayer` runs the rest of the request as the token's `sub`
(`Identity::current()` — see [Authorization](/features/authorization/));
`.identity_claim("email")` picks another claim. `BasicAuthLayer` does the
same with the user name. Since 18.14.0.

### Verifying tokens and reading claims

Call `verify_jwt` directly inside a handler when you also need the decoded claims. The verification is cheap (~1 µs) so double-calling is fine.

```rust
use rust_web_server::auth::{verify_jwt, extract_bearer_token};
use rust_web_server::request::Request;
use rust_web_server::response::Response;

fn protected_handler(req: &Request) -> Response {
    let token = extract_bearer_token(req).expect("JwtLayer already validated this");
    let claims = verify_jwt(&token, b"my-signing-secret").unwrap();

    println!("subject: {:?}", claims.sub);
    println!("expires: {:?}", claims.exp);
    // parse custom claims from claims.raw (raw JSON string)

    Response::new()
}
```

### `Claims` struct

| Field | Type | Description |
|---|---|---|
| `sub` | `Option<String>` | Subject (user ID), if present |
| `exp` | `Option<u64>` | Expiration as Unix seconds, if present |
| `raw` | `String` | Raw UTF-8 JSON payload — inspect for custom claims |

`claims.is_valid_at(now_secs)` returns `true` when the token is not yet expired. Returns `true` when `exp` is absent (no expiry set).

### `extract_bearer_token`

Extracts the raw token string from `Authorization: Bearer <token>`. Returns `None` when the header is absent or does not start with `Bearer `.

```rust
use rust_web_server::auth::extract_bearer_token;

if let Some(token) = extract_bearer_token(&request) {
    // token is the raw JWT string
}
```

## Login handler pattern

A typical login flow: verify credentials, issue a JWT, return it to the client.

```rust
use rust_web_server::auth::build_jwt;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::body::form_urlencoded::FormUrlEncoded;
use std::time::{SystemTime, UNIX_EPOCH};

fn login(req: &Request) -> Response {
    let form = FormUrlEncoded::parse(req.body.to_vec()).unwrap_or_default();
    let username = form.get("username").map(String::as_str).unwrap_or("");
    let password = form.get("password").map(String::as_str).unwrap_or("");

    if username != "admin" || password != "s3cret" {
        let mut r = Response::new();
        r.status_code = 401;
        return r;
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() + 3600; // 1 hour

    let claims = format!(r#"{{"sub":"{username}","exp":{exp}}}"#);
    let token = build_jwt(&claims, b"my-signing-secret");

    let mut r = Response::new();
    // set body to JSON with token
    r
}
```

:::note[Algorithm support]
`build_jwt` and `verify_jwt` only support HS256. For RS256/ES256 against a static public key, see the next section. For public-key JWTs from an identity provider that rotates keys behind a live JWKS endpoint, use the `sso` feature's `JwksCache` instead.
:::

## JWT — RS256 / ES256 (`auth-asymmetric` feature)

A common pattern: a separate auth server issues JWTs signed with its RSA or P-256 *private* key, and your service only ever needs the corresponding *public* key to verify them — no OAuth login flow, no JWKS endpoint, no key rotation to track. `JwtLayer::rs256` and `JwtLayer::es256` cover exactly this, without pulling in the full `sso` feature (OIDC login/callback handlers, PKCE, JWKS fetching):

```toml
[dependencies]
rust-web-server = { version = "18", features = ["auth-asymmetric"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::auth::JwtLayer;
use rust_web_server::core::New;

// public_key_pem is SubjectPublicKeyInfo PEM (RS256 also accepts PKCS#1 `BEGIN RSA PUBLIC KEY`), e.g.:
//   openssl rsa -in key.pem -pubout   (RS256)
//   openssl ec  -in key.pem -pubout   (ES256)
let app = App::new().wrap(JwtLayer::rs256(&rsa_public_key_pem).expect("invalid RSA public key"));
// or:
let app = App::new().wrap(JwtLayer::es256(&ec_public_key_pem).expect("invalid P-256 public key"));
```

Both reject a token whose header `alg` doesn't actually match what was verified — an RS256 key can't wave through a token whose header claims `"alg":"HS256"` just because the bytes happen to parse as something. ES256 signatures are validated in the raw 64-byte `r || s` form JWTs specify, not the ASN.1 DER encoding OpenSSL produces by default when signing outside of a JWT library.

If a handler also needs the decoded claims, call the standalone verify functions directly — same code `JwtLayer` uses internally:

```rust
use rust_web_server::auth::{verify_jwt_rs256, verify_jwt_es256};
use rust_web_server::rsa_key::RsaPublicKey;

let rsa_public_key = RsaPublicKey::from_public_key_pem(PEM)?;   // or from_components(n, e)
let claims = verify_jwt_rs256(&token, &rsa_public_key).unwrap();
let claims = verify_jwt_es256(&token, &ec_public_key).unwrap();
```

RSA keys are 2048–8192 bits. Since 18.0.0 RSA verification is implemented
in-crate (`rust_web_server::rsa_key`) rather than with the `rsa` crate, whose
RUSTSEC-2023-0071 advisory has no fixed release; `verify_jwt_rs256` takes an
`rsa_key::RsaPublicKey`.

`RsaPublicKey` loads a key with `from_public_key_pem` (`BEGIN PUBLIC KEY` or
`BEGIN RSA PUBLIC KEY`), `from_public_key_der` (SPKI), `from_pkcs1_der`, or
`from_components(n, e)` (a JWK's `n`/`e`). It also verifies any
RSASSA-PKCS1-v1_5 / SHA-256 signature directly, not just a JWT:

```rust
use rust_web_server::rsa_key::RsaPublicKey;

let key = RsaPublicKey::from_public_key_pem(&std::fs::read_to_string("issuer.pub.pem")?)?;
let ok: bool = key.verify_pkcs1v15_sha256(b"message", &signature);
```

:::note[Upgrading from 17.x]
`verify_jwt_rs256` and `SamlIdpMetadata.signing_key` used `rsa::RsaPublicKey`.
Build an `rsa_key::RsaPublicKey` instead (`from_public_key_pem`,
`from_public_key_der`, or `from_components(n, e)`).
:::

:::note[When to use `sso::JwksCache` instead]
`JwtLayer::rs256`/`::es256` pin a single static public key at construction time. Use `sso::JwksCache` when the signing key can rotate and you need to fetch current keys from a live JWKS URL (`.well-known/jwks.json`), keyed by `kid` — the right fit for a full OIDC identity provider, not a service-to-service integration with one known signer.
:::

## Forward-auth (delegate to an external service)

`ForwardAuthLayer` delegates the allow/deny decision to an external HTTP service — the same pattern as Traefik's `forwardAuth` or nginx's `auth_request`. Use it to gate requests behind a centralized policy engine (OPA, Casbin) or a shared SSO/session service, without embedding that logic in your app.

```rust
use rust_web_server::app::App;
use rust_web_server::auth::forward::ForwardAuthLayer;
use rust_web_server::core::New;

let app = App::new()
    .wrap(ForwardAuthLayer::new("http://auth.internal/verify")
        .copy_header("X-User-Id")
        .copy_header("X-Roles")
        .timeout_ms(2000));
```

On every request, the layer sends a `GET` to the configured URL with every incoming request header copied onto it (so the auth service can inspect a session cookie or an existing `Authorization` header):

1. **`2xx`** — the request proceeds to the next layer/handler. Any header named via `.copy_header(name)` that's present on the *auth service's response* **replaces** the same-named header on the forwarded request. This is deliberate: if it merely appended, a client could send its own `X-User-Id` and have it coexist with the trusted value.
2. **Any other status** — the auth service's response is returned to the client **verbatim**: status code, headers (minus hop-by-hop and body-framing ones), and body. This preserves a `WWW-Authenticate` challenge or a `Location` redirect into an OAuth login flow without `rws` needing to understand either.
3. **Auth service unreachable** (connection refused, timeout, DNS failure) — `502 Bad Gateway`. This fails *closed*: an unreachable auth service is never treated as "access granted."

Redirects from the auth service itself are not followed (`max_redirects(0)` internally) so a `3xx` response reaches step 2 intact instead of being silently resolved to whatever it points to.

No new Cargo dependency — `ForwardAuthLayer` reuses the existing `crate::http_client::Client`, the same synchronous outbound HTTP client used elsewhere in the framework.

:::note[Builder options]
| Method | Default | Purpose |
|---|---|---|
| `ForwardAuthLayer::new(url)` | — | Auth service URL, e.g. `"http://auth.internal/verify"` |
| `.copy_header(name)` | none | Copy `name` from the auth response onto the forwarded request on `2xx`; call multiple times for multiple headers |
| `.timeout_ms(ms)` | `5000` | Auth service call timeout |
:::

`.identity_header("X-User-Id")` also makes the request run as that user for
[authorization](/features/authorization/) — `Identity::current()` — when the
auth service's 2xx answer carries the header (since 18.14.0).

## No-code auth in the config-driven proxy

`rws.config.toml`'s `[route.middleware.auth]` wires directly into `JwtLayer` and `BasicAuthLayer` — no Rust code required. Both need the `auth` feature enabled at build time (`cargo build --features auth`; the default build doesn't include it).

```toml
[[route]]
name = "api"

[route.match]
path = "/api/*"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "backend"

[route.middleware.auth]
type = "jwt"
secret_env = "JWT_SECRET"
```

```toml
[route.middleware.auth]
type = "basic"
htpasswd_file = ".htpasswd"   # or several: "admins.htpasswd, team.htpasswd"
```

The htpasswd files are re-read when they change (checked at most once a
second, since 18.20.0): adding or removing a user takes effect without a
restart, and a file that can't be read keeps the users read before.

### Sign in with Google, Okta, Keycloak… (`type = "oidc"`)

Since 18.21.0 a route can require an OpenID Connect sign-in — an internal
tool, a staging site, a dashboard without its own login:

```toml
[[route]]
name = "grafana"
[route.match]
host = "grafana.example.com"
path = "/*"
[route.action]
type = "proxy"
upstream = "grafana"
[route.middleware.auth]
type              = "oidc"
issuer            = "https://id.example.com/realms/main"  # discovery: <issuer>/.well-known/openid-configuration
# provider        = "google"         # or google / microsoft (tenant = "…") / github instead of issuer
client_id         = "grafana-proxy"
client_secret_env = "OIDC_CLIENT_SECRET"
cookie_secret_env = "OIDC_COOKIE_SECRET"  # ≥ 32 characters; signs the session cookie
allowed_domains   = ["example.com"]  # verified email in these domains…
# allowed_emails  = ["ann@partner.test"]  # …or these addresses…
allowed_groups    = ["sre"]          # …or one of these groups (from the ID token)
# groups_claim    = "groups"         # the claim holding groups (Keycloak, Okta, Authentik, Azure AD)
# redirect_uri    = "https://grafana.example.com/auth/callback"  # default: from the request's Host
# scopes          = ["openid", "email", "profile"]
# session_ttl_secs = 86400
# exclude         = ["/api/health"]  # paths served without signing in
# forward_headers = true             # X-Auth-Request-User / -Email / -Groups upstream
```

Register `https://<host>/auth/callback` as the redirect URI with the
provider. An unauthenticated request goes to `/auth/login`, then to the
provider, and back to the page it asked for (only local paths are accepted
as `return_to`); `/auth/logout` signs out. With no allow-list everyone who
can sign in at the provider is admitted; with lists, a user needs a verified
email on `allowed_emails`, its domain on `allowed_domains`, or a group on
`allowed_groups` — anyone else gets `403` after signing in.

The session is an HMAC-signed, `HttpOnly`, `SameSite=Lax` cookie (`Secure`
over HTTPS), so nothing is stored in rws: sign-ins survive restarts and work
on every replica that shares `cookie_secret_env`. Without it rws makes up a
key at startup (sign-ins end with the process; `rws --check` warns).
Upstream gets `X-Auth-Request-User` (the subject), `X-Auth-Request-Email`
and `X-Auth-Request-Groups` — the names oauth2-proxy uses, so Grafana's
auth proxy and similar integrations work — and copies a client sends itself
are removed. The provider's discovery document is fetched on the first
request; while it can't be reached, requests get `503` with `Retry-After`.
Needs the `sso` feature (in the release binaries since 18.21.0).

### An auth service decides (`type = "forward"`)

Authelia, Authentik, oauth2-proxy or your own service answer each request
(the nginx `auth_request` / Traefik ForwardAuth pattern):

```toml
[route.middleware.auth]
type         = "forward"
url          = "http://authelia:9091/api/verify"
copy_headers = ["Remote-User", "Remote-Groups", "Remote-Email"]
# timeout_ms = 5000
```

rws sends a `GET` with the request's headers plus `X-Forwarded-Method`,
`-Proto`, `-Host`, `-Uri` and `-For` (set by rws, never taken from the
client). A `2xx` lets the request through with the `copy_headers` from the
auth service's answer — a client's own copies of those headers are always
dropped (since 18.21.0); anything else (a `302` to a login page, a `401`) is
returned to the client as is. An unreachable service is a `502`. Same as
[`ForwardAuthLayer`](#forward-auth-delegate-to-an-external-service).

See [Config-Driven Proxy](/proxy/config-driven/) for the full `[route.middleware]` reference. Auth config fails closed: an unset/empty `token_env`/`secret_env`, a missing `htpasswd_file`, an `auth` table with an unknown `type`, or a build without the feature a type needs makes that route answer `503 Service Unavailable` (with an error in the startup log and from `rws --check`) instead of serving it unprotected. Other routes keep working. Bearer tokens are compared in constant time.

## Password hashing (`crypto` feature)

The `crypto` feature (independent of `auth`) adds Argon2id password hashing and secure random tokens:

```toml
[dependencies]
rust-web-server = { version = "18", features = ["crypto"] }
```

```rust
use rust_web_server::crypto::{hash_password, verify_password, generate_token};

// At registration: store the returned PHC string ($argon2id$v=19$...) as-is.
// The random salt is embedded in it, so no separate salt column is needed.
let hash = hash_password("hunter2")?;

// At login: constant-time comparison; Err only when `hash` isn't a valid PHC string.
if !verify_password(&submitted_password, &hash)? {
    // return 401
}

// Password-reset tokens, email verification codes, API keys:
let reset_token = generate_token(32); // 32 random bytes -> 64-char lowercase hex
```

All three live in `rust_web_server::crypto`; errors are `CryptoError`. Use at least 16 bytes for tokens and 32 bytes for high-value secrets such as API keys.
