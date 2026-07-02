---
title: Authentication
description: HTTP Basic Auth and HS256 JWT middleware with helper functions for issuing and verifying tokens.
---

The `auth` Cargo feature adds `BasicAuthLayer` and `JwtLayer` middleware, plus standalone helpers for building and verifying HS256 JWTs.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["auth"] }
```

The feature pulls in `hmac` and `sha2` (RustCrypto) as additional dependencies. No other HTTP or JWT crates are required.

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
`build_jwt` and `verify_jwt` only support HS256. For RS256/ES256 (public-key JWTs from an identity provider), use the `sso` feature which provides `JwksCache`.
:::
