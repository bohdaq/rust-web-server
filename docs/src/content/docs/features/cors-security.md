---
title: CORS & Security Headers
description: Configure cross-origin resource sharing and automatic security headers in rust-web-server.
---

Every response includes a baseline set of security headers automatically. CORS is controlled by a set of `RWS_CONFIG_CORS_*` environment variables. Client Hints are opt-in.

## CORS

### Allow-all mode (default)

When `RWS_CONFIG_CORS_ALLOW_ALL=true` (the default), the server echoes back whatever `Origin` the browser sends and sets `Access-Control-Allow-Credentials: true`. On `OPTIONS` preflight requests it also mirrors back the `Access-Control-Request-Method` and `Access-Control-Request-Headers` values and adds `Access-Control-Max-Age: 86400`.

```bash
RWS_CONFIG_CORS_ALLOW_ALL=true
```

### Explicit origins mode

Set `RWS_CONFIG_CORS_ALLOW_ALL=false` and list the allowed origins as a comma-separated string:

```bash
RWS_CONFIG_CORS_ALLOW_ALL=false
RWS_CONFIG_CORS_ALLOW_ORIGINS=https://app.example.com,https://admin.example.com
RWS_CONFIG_CORS_ALLOW_CREDENTIALS=true
RWS_CONFIG_CORS_ALLOW_METHODS=GET,POST,PUT,DELETE,OPTIONS
RWS_CONFIG_CORS_ALLOW_HEADERS=Content-Type,Authorization,X-Request-ID
RWS_CONFIG_CORS_EXPOSE_HEADERS=X-Total-Count,X-Request-ID
RWS_CONFIG_CORS_MAX_AGE=3600
```

When the incoming `Origin` is not one of the listed origins no CORS headers are added to the response. Origins are compared whole (ASCII case ignored): with `https://app.example.com` listed, `https://app.example.co` and `https://app.example.com.evil` don't match (`Cors::origin_is_allowed`; before 17.113.2 a substring of the list matched). `Access-Control-Expose-Headers` is sent on the actual response, where the browser reads it, not only on the preflight.

### Environment variable reference

| Variable | Default | Description |
|---|---|---|
| `RWS_CONFIG_CORS_ALLOW_ALL` | `true` | Echo any origin back |
| `RWS_CONFIG_CORS_ALLOW_ORIGINS` | `""` | Comma-separated allowlist (used when allow-all is `false`) |
| `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` | `""` | Set `Access-Control-Allow-Credentials: true` |
| `RWS_CONFIG_CORS_ALLOW_METHODS` | `""` | Value for `Access-Control-Allow-Methods` on preflight |
| `RWS_CONFIG_CORS_ALLOW_HEADERS` | `""` | Value for `Access-Control-Allow-Headers` on preflight |
| `RWS_CONFIG_CORS_EXPOSE_HEADERS` | `""` | Value for `Access-Control-Expose-Headers`, on every response to an allowed origin |
| `RWS_CONFIG_CORS_MAX_AGE` | `86400` | Preflight cache TTL in seconds |

### Programmatic CORS (`Cors` struct)

For handlers that need fine-grained control, use `Cors::_process` directly:

```rust
use rust_web_server::cors::Cors;

let cors = Cors {
    allow_all: false,
    allow_origins: vec!["https://app.example.com".into()],
    allow_methods: vec!["GET".into(), "POST".into()],
    allow_headers: vec!["Authorization".into()],
    allow_credentials: true,
    expose_headers: vec![],
    max_age: "3600".into(),
};

let headers = Cors::_process(&request, &cors)?;
```

### Hot reload

CORS configuration is re-read on `SIGHUP` (or `POST /admin/config/reload`) without a restart.

## Security headers

Every response carries `X-Content-Type-Options: nosniff`. On TLS connections
(HTTPS and HTTP/3) the server also sends:

```
Strict-Transport-Security: max-age=31536000; includeSubDomains
```

The other browser-policy headers are opt-in (since 18.0.0 — before that they
were always sent): set `RWS_CONFIG_SECURITY_HEADERS=true`
(`security_headers = true` in `rws.config.toml`) and every response also
carries

| Header | Value |
|---|---|
| `X-Frame-Options` | `SAMEORIGIN` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `Permissions-Policy` | `geolocation=(), microphone=(), camera=()` |
| `Content-Security-Policy` | `default-src 'self'` (unless `RWS_CONFIG_CSP` is set) |

They are off by default because most deployments set them per site (or in a
proxy in front), and each one is work on every response — over HTTP/2 every
header is HPACK-encoded.

### Content-Security-Policy

Set a policy with `RWS_CONFIG_CSP`; it is sent whether or not
`RWS_CONFIG_SECURITY_HEADERS` is on:

```bash
RWS_CONFIG_CSP="default-src 'self'; script-src 'self' cdn.example.com; img-src *"
```

With `RWS_CONFIG_SECURITY_HEADERS=true`, an empty value suppresses the
default policy:

```bash
RWS_CONFIG_CSP=
```

### Nanosecond timestamps

`RWS_CONFIG_NANOS_HEADERS=true` (`nanos_headers = true`) adds
`Date-Unix-Epoch-Nanos` (the response time in nanoseconds since the Unix
epoch, per response) and, for static files, `Last-Modified-Unix-Epoch-Nanos`.
Off by default since 18.0.0: `Date` and `Last-Modified` carry the same
information to the second, and values that change on every response can't be
indexed by HTTP/2's header compression.

## Client Hints

Client hints are opt-in (since 18.0.0): set `RWS_CONFIG_CLIENT_HINTS=true`
(`client_hints = true` in `rws.config.toml`) and every response advertises
the following hints via `Accept-CH` and `Critical-CH`:

```
Accept-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List,
           Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT,
           Save-Data, Device-Memory, Sec-CH-Prefers-Reduced-Motion,
           Sec-CH-Prefers-Color-Scheme
```

The hints also appear in the `Vary` header so caches key responses on them.
They're off by default because they add about 700 bytes to every response,
and `Critical-CH` makes Chromium retry a request that arrived without them.
A browser that already sends a hint (`Sec-CH-UA-*` low-entropy hints,
`Save-Data`) still sends it; reading an incoming hint in a handler works
either way:

```rust
use rust_web_server::client_hint::ClientHint;
use rust_web_server::request::Request;

fn handler(req: &Request) {
    if let Some(h) = req.get_header(ClientHint::PREFERS_COLOR_SCHEME.to_string()) {
        let scheme = &h.value; // "light" or "dark"
    }
    if let Some(h) = req.get_header(ClientHint::NETWORK_SAVE_DATA.to_string()) {
        // serve lower-resolution images when Save-Data: on
    }
}
```

Available hint constants in `ClientHint`:

| Constant | Header |
|---|---|
| `USER_AGENT_CPU_ARCHITECTURE` | `Sec-CH-UA-Arch` |
| `USER_AGENT_CPU_BITNESS` | `Sec-CH-UA-Bitness` |
| `USER_AGENT_FULL_BRAND_INFORMATION` | `Sec-CH-UA-Full-Version-List` |
| `USER_AGENT_DEVICE_MODEL` | `Sec-CH-UA-Model` |
| `USER_AGENT_OPERATING_SYSTEM_VERSION` | `Sec-CH-UA-Platform-Version` |
| `NETWORK_DOWNLOAD_SPEED` | `Downlink` |
| `NETWORK_EFFECTIVE_CONNECTION_TYPE` | `ECT` |
| `NETWORK_ROUND_TRIP_TIME` | `RTT` |
| `NETWORK_SAVE_DATA` | `Save-Data` |
| `DEVICE_MEMORY` | `Device-Memory` |
| `PREFERS_REDUCED_MOTION` | `Sec-CH-Prefers-Reduced-Motion` |
| `PREFERS_COLOR_SCHEME` | `Sec-CH-Prefers-Color-Scheme` |

:::note[Vary header]
With client hints on, the `Vary` response header includes the full list of hints so HTTP caches serve the correct variant per client capability; otherwise it is just `Vary: Origin`.
:::
