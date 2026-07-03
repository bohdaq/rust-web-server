---
title: Cookies
description: Read cookies from incoming requests, set cookies with the SetCookie builder, and sign or encrypt cookie values.
---

`rust-web-server` provides two types in `rust_web_server::cookie` for working with HTTP cookies:

- `CookieJar` — parses the `Cookie` request header into individual name/value pairs
- `SetCookie` — builder for `Set-Cookie` response header values

## Reading cookies from a request

The `Cookie` header value is a semicolon-separated list of `name=value` pairs. Pass the raw header value to `CookieJar::parse`:

```rust
use rust_web_server::cookie::CookieJar;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

fn dashboard(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let jar = req
        .get_header("cookie")
        .map(|h| CookieJar::parse(&h.value))
        .unwrap_or_else(|| CookieJar::parse(""));

    let session_id = jar.get("session").map(|c| c.value.as_str()).unwrap_or("");

    if session_id.is_empty() {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
        return r;
    }

    // session_id is available here
    Response::new()
}
```

### `CookieJar` API

| Method | Signature | Description |
|---|---|---|
| `CookieJar::parse` | `fn parse(header_value: &str) -> CookieJar` | Parses a raw `Cookie` header value |
| `.get(name)` | `fn get(&self, name: &str) -> Option<&Cookie>` | Returns the first matching cookie |

The returned `Cookie` struct has two fields:
- `name: String`
- `value: String`

Names and values are trimmed of leading/trailing whitespace. If the `Cookie` header contains `session=abc123; theme=dark`, you get two cookies.

## Setting cookies in a response

Use `SetCookie` to build the value for the `Set-Cookie` response header. The builder is fluent — every method takes ownership and returns `Self`.

```rust
use rust_web_server::cookie::SetCookie;
use rust_web_server::header::Header;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::core::New;

fn login(
    _req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let session_token = "eyJhbGc...";  // generate your token

    let cookie_value = SetCookie::new("session", session_token)
        .path("/")
        .http_only()
        .secure()
        .same_site("Strict")
        .max_age(3600)  // 1 hour in seconds
        .build();

    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.headers.push(Header {
        name:  "Set-Cookie".to_string(),
        value: cookie_value,
    });
    r
}
```

### `SetCookie` builder methods

| Method | RFC 6265 attribute | Description |
|---|---|---|
| `SetCookie::new(name, value)` | — | Creates the builder with name and value |
| `.path(path)` | `Path` | Restricts cookie to URL subtree |
| `.domain(domain)` | `Domain` | Restricts cookie to domain and sub-domains |
| `.max_age(seconds)` | `Max-Age` | Lifetime in seconds; `0` or negative deletes the cookie |
| `.secure()` | `Secure` | Only sent over HTTPS connections |
| `.http_only()` | `HttpOnly` | Inaccessible to JavaScript (`document.cookie`) |
| `.same_site(policy)` | `SameSite` | One of `"Strict"`, `"Lax"`, or `"None"` |
| `.build()` | — | Returns the formatted `Set-Cookie` header value string |

The built string looks like:

```
session=eyJhbGc...; Path=/; Max-Age=3600; Secure; HttpOnly; SameSite=Strict
```

:::note[Expires attribute]
The `Expires` attribute (an absolute date) is not directly supported by `SetCookie`. Use `Max-Age` instead — it is preferred by RFC 6265 and avoids clock skew issues. If you need `Expires`, append it manually to the string returned by `.build()`.
:::

## Signed and encrypted cookie values

Plain `SetCookie` values are readable and forgeable by the client — fine for a UI preference like a theme, wrong for anything a handler trusts without re-checking server-side (a session identifier, a role, a discount eligibility flag). The `crypto` feature adds two function pairs in `rust_web_server::cookie` for exactly this:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["crypto"] }
```

| Function pair | Guarantee | Use when |
|---|---|---|
| `signed_cookie` / `verify_signed_cookie` | Tamper-evident, but still plain-text and readable by the client | The client only needs to *see* the value, not silently modify it |
| `encrypted_cookie` / `decrypt_cookie` | Confidential — unreadable and unmodifiable by the client | The value shouldn't be visible to the client at all |

### Signing (HMAC-SHA256)

```rust
use rust_web_server::cookie::{signed_cookie, verify_signed_cookie, SetCookie};

let secret = b"my-signing-secret"; // load from an env var in real code

// Issuing:
let cookie_value = signed_cookie("plan=pro", secret);
let header_value = SetCookie::new("prefs", &cookie_value).http_only().build();

// Reading it back later:
match verify_signed_cookie(&cookie_value, secret) {
    Some(value) => { /* value == "plan=pro"; trust it */ }
    None => { /* missing, malformed, or tampered — treat as absent */ }
}
```

`signed_cookie` returns `"<value>.<hex-signature>"`. `verify_signed_cookie` splits on the *last* `.`, so a value that itself contains dots still round-trips correctly — the fixed-length hex HMAC signature never contains one. Any tampering with either half, or verifying with the wrong secret, returns `None`.

### Encryption (AES-256-GCM)

```rust
use rust_web_server::cookie::{encrypted_cookie, decrypt_cookie, SetCookie};

let key = b"my-encryption-key"; // any length — SHA-256-derived into the 256-bit AES key

// Issuing:
let cookie_value = encrypted_cookie("session-token-abc123", key);
let header_value = SetCookie::new("sess", &cookie_value).http_only().secure().build();

// Reading it back later:
match decrypt_cookie(&cookie_value, key) {
    Some(value) => { /* value == "session-token-abc123" */ }
    None => { /* missing, malformed, tampered, or wrong key */ }
}
```

`encrypted_cookie` returns `"<hex-nonce>.<hex-ciphertext-and-tag>"`, generating a fresh random 96-bit nonce on every call — encrypting the same value twice never produces the same cookie value, so an observer can't correlate repeated issuances. `decrypt_cookie` returns `None` on any failure (GCM's authentication tag check fails closed rather than returning garbage on a tampered ciphertext).

:::note[Choosing between the two]
Both fail closed to `None` rather than exposing *why* verification failed — this avoids giving an attacker an oracle to probe. If you don't need confidentiality, prefer `signed_cookie`: it's cheaper and the value stays inspectable in a browser's dev tools during development.
:::

## Deleting a cookie

Set `Max-Age` to `0` to instruct the browser to delete the cookie immediately:

```rust
let cookie_value = SetCookie::new("session", "")
    .path("/")
    .http_only()
    .max_age(0)
    .build();
```

## Setting multiple cookies

Push one `Set-Cookie` header per cookie. HTTP allows (and requires) multiple `Set-Cookie` headers in a single response:

```rust
r.headers.push(Header {
    name:  "Set-Cookie".to_string(),
    value: SetCookie::new("session", token).path("/").http_only().build(),
});
r.headers.push(Header {
    name:  "Set-Cookie".to_string(),
    value: SetCookie::new("theme", "dark").path("/").max_age(31536000).build(),
});
```

## Full request/response cycle example

```rust
use rust_web_server::cookie::{CookieJar, SetCookie};
use rust_web_server::header::Header;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::core::New;

fn refresh_session(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    // Read the existing session cookie
    let jar = req
        .get_header("cookie")
        .map(|h| CookieJar::parse(&h.value))
        .unwrap_or_else(|| CookieJar::parse(""));

    let old_token = jar.get("session").map(|c| c.value.as_str()).unwrap_or("");
    if old_token.is_empty() {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
        return r;
    }

    // Rotate the token
    let new_token = rotate_token(old_token);

    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.headers.push(Header {
        name:  "Set-Cookie".to_string(),
        value: SetCookie::new("session", new_token)
            .path("/")
            .http_only()
            .secure()
            .same_site("Strict")
            .max_age(3600)
            .build(),
    });
    r
}

fn rotate_token(old: &str) -> String {
    // your token rotation logic
    format!("{}_rotated", old)
}
```
