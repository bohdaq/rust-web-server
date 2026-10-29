---
title: Migrating to 18.0
description: What changes when upgrading rws from 17.x to 18.0 — Header field types, default response headers, and RSA public keys.
---

18.0 has a handful of breaking changes; most applications only meet the first and the `Request.body` type. If you are coming from 17.113.0 or earlier, also read [the patch-release changes](#coming-from-171130-or-earlier).

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
| `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy` | always | only with `RWS_CONFIG_SECURITY_HEADERS=true` |
| `Content-Security-Policy` | `default-src 'self'` unless `RWS_CONFIG_CSP` | `RWS_CONFIG_CSP`, or the default with `RWS_CONFIG_SECURITY_HEADERS=true` |
| `Date-Unix-Epoch-Nanos`, `Last-Modified-Unix-Epoch-Nanos` | always | only with `RWS_CONFIG_NANOS_HEADERS=true` |
| `Access-Control-Allow-Origin` (+ `-Credentials: true`) for any `Origin` | always (`cors_allow_all` defaulted to `true`) | only with `RWS_CONFIG_CORS_ALLOW_ALL=true` or a listed origin |

What it means:

- **Browsers now keep static files** and revalidate them (`If-None-Match` /
  `If-Modified-Since` → `304`). To get the old behaviour set
  `RWS_CONFIG_STATIC_CACHE_CONTROL=` (empty). For fingerprinted asset names,
  set something like `public, max-age=31536000, immutable`. Responses from
  your handlers still default to `no-store, …`.
- **Security headers**: to keep sending `X-Frame-Options`,
  `Referrer-Policy`, `Permissions-Policy` and the default CSP, set
  `RWS_CONFIG_SECURITY_HEADERS=true` (`security_headers = true`). A CSP set
  with `RWS_CONFIG_CSP` is sent either way.
- **Nanosecond timestamps**: clients that read `Date-Unix-Epoch-Nanos` or
  `Last-Modified-Unix-Epoch-Nanos` need `RWS_CONFIG_NANOS_HEADERS=true`
  (`nanos_headers = true`).
- **Client hints**: if your app reads `Sec-CH-*` request headers, set
  `RWS_CONFIG_CLIENT_HINTS=true` (or `client_hints = true` in
  `rws.config.toml`).
- **CORS**: a browser app on another origin that calls `rws` needs its origin
  in `RWS_CONFIG_CORS_ALLOW_ORIGINS` (`[cors] allow_origins`), plus
  `RWS_CONFIG_CORS_ALLOW_CREDENTIALS=true` if it sends cookies. Setting
  `RWS_CONFIG_CORS_ALLOW_ALL=true` restores the old echo-any-origin
  behaviour; avoid it for anything with logins.
- **ACME renewal**: certificates renew at two-thirds of their lifetime (or
  when the CA's renewal information says), not 30 days before expiry; set
  `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS=30` to keep the old margin. Code that
  builds an `AcmeConfig` by hand sets `renew_before_days: None` (or
  `Some(days)`) and `profile: None`.
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

## `Request.body` is a `Body`

`Request.body` was a `Vec<u8>`; it is a `request::Body`, which can also be a
stream of a body still arriving. It dereferences to `&[u8]`, so reading
code mostly compiles unchanged. What to change:

| 17.x | 18.0 |
|---|---|
| `body: vec![]` | `body: Default::default()` |
| `body: bytes` (a `Vec<u8>`) | `body: bytes.into()` |
| `request.body.clone()` where a `Vec<u8>` is wanted | `request.body.to_vec()` |
| `String::from_utf8(request.body)` | `String::from_utf8(request.body.to_vec())` |
| `request.body = vec` | `request.body = vec.into()` |
| `assert_eq!(b"x".to_vec(), request.body)` | unchanged (`Vec<u8> == Body` works) |

## The `rws` binary runs one runtime per core

The `rws` binary now starts one single-threaded runtime per core, each
accepting on its own `SO_REUSEPORT` socket, like nginx's workers
(`RWS_CONFIG_RUNTIME=per-core`, Unix). On fast CPUs this serves more
requests per second; on slower ones it can cost more CPU per small request.
To keep 17.x behaviour, set `runtime = "shared"` in `rws.config.toml` or
`RWS_CONFIG_RUNTIME=shared`. Applications built on the library are
unchanged: they stay `shared` unless they set it.

## 18.9: `ConnectionInfo.tls`

`ConnectionInfo` has a new public field, `tls: bool` (the client connected
over TLS), which proxies use for `X-Forwarded-Proto`. A struct literal that
builds a `ConnectionInfo` must now set it. To stop field additions breaking
your code, use the constructor or `Default`:

```rust,ignore
// 18.8
let connection = ConnectionInfo { client, server, request_size: 10_000, sni_hostname: None };
// 18.9
let connection = ConnectionInfo { tls: false, ..ConnectionInfo::new(client, server) };
let connection = ConnectionInfo::default(); // in tests
```

## Coming from 17.113.0 or earlier

18.0 carries the 17.113.1 security fixes and the 17.113.2 conformance fixes.
If you skipped those releases, these refuse or change what you may rely on:

- **Static files:** paths with `..`, dot-files (`.git/`, `.env`; set
  `RWS_CONFIG_SERVE_DOTFILES=true` to serve them), `*.pem`, `*.key` and
  `rws.config.toml` are not served. Request paths are percent-decoded, so a
  file whose name contains a literal `%` is requested as `%25`.
- **ACME** keeps its keys in `.rws/acme/` by default (files at the old
  default paths are moved there on start).
- **The `rws` binary** mounts `/mcp` only when `MCP_TOKEN` is set.
- **HTTP/1.1 requests** without `Host` or with two, with a lower-case method,
  a folded header line or whitespace before a header's colon get `400`; an
  HTTP/1.0 connection closes after each response unless the client sends
  `Connection: keep-alive`.
- **CORS** origins match whole: list each origin, a part of one no longer
  matches.

## Also new in 18.0 (not breaking)

- A request head that doesn't complete within 30 s of its first byte gets
  `408 Request Timeout`.
- `[[upstream]] tls_ca_file` trusts a private CA for `https://` upstreams.
- Inline tables in settings: `rate_limit = { max_requests = 100 }`.

The full list is in [`CHANGELOG.md`](https://github.com/bohdaq/rust-web-server/blob/main/CHANGELOG.md).
