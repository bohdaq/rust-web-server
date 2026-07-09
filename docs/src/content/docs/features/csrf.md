---
title: CSRF Protection
description: Double-submit cookie pattern with constant-time comparison and automatic SameSite=Strict enforcement.
---

The `csrf` feature adds `CsrfLayer` middleware that protects state-mutating requests against cross-site request forgery using the double-submit cookie pattern.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["csrf"] }
```

## Quick start

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::csrf::CsrfLayer;

let app = App::new().wrap(CsrfLayer::new());
```

## How it works

1. **Safe methods (`GET`, `HEAD`, `OPTIONS`, `TRACE`):** `CsrfLayer` reads the existing `_csrf` cookie or generates a new 32-byte random token (hex-encoded, 64 characters). The token is injected into the request as a private `X-Rws-Csrf-Token` header so `CsrfToken::from_request` can return it to the handler. The `_csrf` cookie is set (or refreshed) on the response with `SameSite=Strict; Path=/`.

2. **Mutating methods (`POST`, `PUT`, `PATCH`, `DELETE`):** The layer reads the cookie value and the submitted value (from the `X-CSRF-Token` request header or the `_csrf` form field in `application/x-www-form-urlencoded` bodies). The two values are compared in **constant time** to prevent timing attacks. If they do not match, `403 Forbidden` is returned immediately.

## Embedding the token in HTML forms

In a `GET` handler, use `CsrfToken::from_request` to retrieve the current token:

```rust
use rust_web_server::csrf::CsrfToken;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;

fn show_form(req: &Request, _conn: &ConnectionInfo) -> Response {
    let token = CsrfToken::from_request(req)
        .map(|t| t.value().to_string())
        .unwrap_or_default();

    let html = format!(
        r#"<form method="POST" action="/submit">
  <input type="hidden" name="_csrf" value="{token}">
  <button type="submit">Submit</button>
</form>"#
    );
    // build Response with `html` body ...
    Response::new()
}
```

`CsrfToken::from_request` returns `None` when `CsrfLayer` is not in the middleware stack.

## AJAX / fetch requests

For JavaScript clients, read the token from the `_csrf` cookie (the cookie is not `HttpOnly` by default) and send it as the `X-CSRF-Token` header:

```javascript
// Read the cookie
function getCookie(name) {
    const value = `; ${document.cookie}`;
    const parts = value.split(`; ${name}=`);
    if (parts.length === 2) return parts.pop().split(';').shift();
}

// Attach to every mutating request
fetch('/api/resource', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'X-CSRF-Token': getCookie('_csrf'),
    },
    body: JSON.stringify({ key: 'value' }),
});
```

## React (or any fetch/axios-based SPA)

The pattern above applies directly to a React app — the wrinkle is *when* the `_csrf` cookie gets set, since a single-page app has no traditional server-rendered `GET` page load for every navigation the way a template-rendered site does.

**In production**, `rws` itself serves the built app (see [Static Files & SPA Fallback](/features/static-files/)) — so the very first request the browser makes (loading `index.html`) is already a `GET` through `CsrfLayer`, which sets the `_csrf` cookie on that response before any React code has even run. By the time your app mounts, the cookie already exists.

**In local dev**, if you're using the [recommended frontend dev-server proxy setup](/getting-started/frontend-dev-proxy/), the HTML shell comes from Vite/webpack-dev-server, not `rws` — so nothing sets the `_csrf` cookie until the SPA makes its first request through the proxy to `rws`. If that first request happens to be a mutating one (e.g. a login `POST`), there's no cookie yet and it gets `403`. Prime it explicitly with a safe `GET` call on app startup:

```jsx
// api.js
import axios from 'axios';

function getCookie(name) {
  const value = `; ${document.cookie}`;
  const parts = value.split(`; ${name}=`);
  if (parts.length === 2) return parts.pop().split(';').shift();
}

export const api = axios.create({ baseURL: '/api' });

// Attach the CSRF token to every mutating request. GET/HEAD requests are
// never checked, so they're left alone.
api.interceptors.request.use((config) => {
  if (['post', 'put', 'patch', 'delete'].includes(config.method)) {
    config.headers['X-CSRF-Token'] = getCookie('_csrf');
  }
  return config;
});
```

```jsx
// App.jsx
import { useEffect } from 'react';
import { api } from './api';

function App() {
  useEffect(() => {
    // Prime the _csrf cookie once on app startup — any safe (GET) endpoint
    // works, since CsrfLayer refreshes the cookie on every GET regardless
    // of which route handles it.
    api.get('/session');
  }, []);

  // ... rest of the app
}
```

After this runs once, `getCookie('_csrf')` inside the interceptor has a real value for every subsequent mutating request — login forms, form submissions, `DELETE` buttons, all get the header attached automatically with no per-call boilerplate.

Whichever approach you use, the `_csrf` cookie must stay readable from JavaScript — don't call `.http_only(true)` on the `CsrfLayer` (see [Defaults and customization](#defaults-and-customization) below), since that's the opposite, HTML-form-only workflow where JS never needs to see the token at all.

## Defaults and customization

| Setting | Default | Builder method |
|---|---|---|
| Cookie name | `_csrf` | `.cookie_name("csrf_token")` |
| Form field name | `_csrf` | `.field_name("csrf_token")` |
| Header name | `X-CSRF-Token` | `.header_name("X-My-CSRF")` |
| `SameSite` | `Strict` | (not configurable) |
| `HttpOnly` | `false` | `.http_only(true)` |
| `Secure` | `false` | `.secure(true)` |

```rust
use rust_web_server::csrf::CsrfLayer;

let layer = CsrfLayer::new()
    .cookie_name("csrf_token")
    .field_name("csrf_token")
    .http_only(true)   // HTML-form-only workflow; JS cannot read the cookie
    .secure(true);     // Required in production (HTTPS only)
```

:::note[Production checklist]
In production, always set `.secure(true)` so the cookie is only transmitted over HTTPS. This prevents the CSRF token from leaking over plain HTTP.
:::

## What is and is not checked

| Method | Checked? |
|---|---|
| `GET` | No — token is generated/refreshed instead |
| `HEAD` | No |
| `OPTIONS` | No |
| `TRACE` | No |
| `POST` | Yes |
| `PUT` | Yes |
| `PATCH` | Yes |
| `DELETE` | Yes |

## Constant-time comparison

The middleware uses a constant-time byte comparison to prevent timing attacks:

- The comparison always runs in time proportional to the token length, regardless of where the mismatch occurs.
- Length mismatch returns `false` immediately (lengths are not secret).
- The token is 64 hex characters (256 bits of entropy from the OS CSPRNG via `OsRng`).

## Token source precedence

On mutating requests, the submitted token is extracted in this order:

1. `X-CSRF-Token` request header (any content type).
2. `_csrf` field in an `application/x-www-form-urlencoded` request body.

If neither is present, `403 Forbidden` is returned.
