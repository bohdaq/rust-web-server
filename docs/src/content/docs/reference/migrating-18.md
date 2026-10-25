---
title: Migrating to 18.0
description: What changes when upgrading rws from 17.x to 18.0 — Header field types, default response headers, and RSA public keys.
---

18.0 has three breaking changes. Most applications only meet the first.

## `Header` fields are `Cow<'static, str>`

`Header { name, value }` used to be two `String`s; now both are
`Cow<'static, str>`, so the standard headers on every response are borrowed
instead of allocated.

```rust
use rust_web_server::header::Header;

// 17.x
let h = Header { name: Header::_LOCATION.to_string(), value: url };

// 18.0 — a &'static str is borrowed, a String is owned
let h = Header::new(Header::_LOCATION, url);
let h = Header::new("X-Static", "fixed");
let h = Header { name: "X-Static".into(), value: "fixed".into() }; // fields are still public
```

Reading:

| 17.x | 18.0 |
|------|------|
| `h.value.as_str()` | `&*h.value` or `h.value.as_ref()` (`as_str` isn't stable on the 1.88 MSRV) |
| `h.value.clone()` (a `String`) | `h.value.to_string()` |
| `h.value.push_str(..)` | `h.value.to_mut().push_str(..)` |
| `h.name == "Content-Type"` | unchanged (`Cow<str>` compares with `&str`) |

To find a request header, prefer `request.header("content-type")` — a
case-insensitive lookup that returns `Option<&Header>` without allocating.

## Default response headers

| Header | 17.x | 18.0 |
|--------|------|------|
| `Accept-CH`, `Critical-CH`, client hints in `Vary` | always | only with `RWS_CONFIG_CLIENT_HINTS=true` |
| `Vary` | `Origin` + the client-hint headers | `Origin` |
| `Cache-Control` on static files | `no-store, …` | `no-cache` (`RWS_CONFIG_STATIC_CACHE_CONTROL`) |
| `Last-Modified` on static files | non-standard | standard HTTP date |
| `Date` | — | on every response |

What it means:

- **Browsers now keep static files** and revalidate them (`If-None-Match` /
  `If-Modified-Since` → `304`). To get the old behaviour set
  `RWS_CONFIG_STATIC_CACHE_CONTROL=` (empty). For fingerprinted asset names,
  set something like `public, max-age=31536000, immutable`. Responses from
  your handlers still default to `no-store, …`.
- **Client hints**: if your app reads `Sec-CH-*` request headers, set
  `RWS_CONFIG_CLIENT_HINTS=true` (or `client_hints = true` in
  `rws.config.toml`).
- **`CacheLayer`** no longer stores `no-cache` responses, responses to `Range`
  requests, or responses whose `Vary` names a request header outside its key.
  Static files are therefore not cached by it (they were never meant to be).

## RSA public keys

`rws` verifies RS256 JWTs and SAML signatures itself now (the `rsa` crate had
an unfixed advisory, RUSTSEC-2023-0071). Two signatures change:

- `auth::verify_jwt_rs256(token, &key)` takes a `&rsa_key::RsaPublicKey`.
- `SamlIdpMetadata.signing_key` is an `rsa_key::RsaPublicKey`.

`JwtLayer::rs256(pem)` and the SSO/JWKS code are unchanged for callers.

```rust
use rust_web_server::rsa_key::RsaPublicKey;

let key = RsaPublicKey::from_public_key_pem(&pem)?; // BEGIN PUBLIC KEY or BEGIN RSA PUBLIC KEY
let key = RsaPublicKey::from_public_key_der(&der)?; // SPKI
let key = RsaPublicKey::from_components(&n_bytes, &e_bytes)?;
```

Keys under 2048 bits are rejected; a JWKS key that can't be used is skipped.

## Also new in 18.0 (not breaking)

- A request head that doesn't complete within 30 s of its first byte gets
  `408 Request Timeout`.
- `[[upstream]] tls_ca_file` trusts a private CA for `https://` upstreams.
- Inline tables in settings: `rate_limit = { max_requests = 100 }`.

The full list is in [`CHANGELOG.md`](https://github.com/bohdaq/rust-web-server/blob/main/CHANGELOG.md).
