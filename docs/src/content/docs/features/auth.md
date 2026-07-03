---
title: Authentication
description: HTTP Basic Auth, HS256/RS256/ES256 JWT, and forward-auth middleware with helper functions for issuing and verifying tokens.
---

The `auth` Cargo feature adds `BasicAuthLayer`, `JwtLayer`, and `ForwardAuthLayer` middleware, plus standalone helpers for building and verifying HS256 JWTs. The `auth-asymmetric` feature (on top of `auth`) adds `JwtLayer::rs256`/`::es256` for verifying RS256/ES256 tokens against a static public key.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["auth"] }
# or, for RS256/ES256 support too:
rust-web-server = { version = "17", features = ["auth-asymmetric"] }
```

The `auth` feature pulls in `hmac` and `sha2` (RustCrypto) as additional dependencies. `auth-asymmetric` additionally pulls in `rsa` and `p256` (also RustCrypto) — the same crates the `sso` feature uses, without the rest of `sso`'s OAuth/OIDC/JWKS machinery.

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
    let form = FormUrlEncoded::parse(&req.body);
    let username = form.get("username").unwrap_or("");
    let password = form.get("password").unwrap_or("");

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
rust-web-server = { version = "17", features = ["auth-asymmetric"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::auth::JwtLayer;
use rust_web_server::core::New;

// public_key_pem is SubjectPublicKeyInfo PEM, e.g.:
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

let claims = verify_jwt_rs256(&token, &rsa_public_key).unwrap();
let claims = verify_jwt_es256(&token, &ec_public_key).unwrap();
```

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

## No-code auth in the config-driven proxy

`rws.config.toml`'s `[route.middleware.auth]` wires directly into `JwtLayer` and `BasicAuthLayer` — no Rust code required. Both need the `auth` feature enabled at build time (`cargo build --features auth`, or the full-featured default build).

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
htpasswd_file = ".htpasswd"
```

See [Config-Driven Proxy](/proxy/config-driven/) for the full `[route.middleware]` reference. Fail-open-with-a-warning behavior applies to both: an unset/empty `secret_env`, a missing `htpasswd_file`, or building without the `auth` feature all skip that route's auth check (logged to stderr at startup) rather than aborting the whole config — the route falls through to its normal action, unprotected. Check your server's startup logs after changing auth config.
