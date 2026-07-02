---
title: CORS & Security Headers
description: Configure cross-origin resource sharing and automatic security headers in rust-web-server.
---

Every response includes a baseline set of security headers automatically. CORS is controlled by a set of `RWS_CONFIG_CORS_*` environment variables. Client Hints are advertised on every response.

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

When the incoming `Origin` header does not appear in `RWS_CONFIG_CORS_ALLOW_ORIGINS` no CORS headers are added to the response.

### Environment variable reference

| Variable | Default | Description |
|---|---|---|
| `RWS_CONFIG_CORS_ALLOW_ALL` | `true` | Echo any origin back |
| `RWS_CONFIG_CORS_ALLOW_ORIGINS` | `""` | Comma-separated allowlist (used when allow-all is `false`) |
| `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` | `""` | Set `Access-Control-Allow-Credentials: true` |
| `RWS_CONFIG_CORS_ALLOW_METHODS` | `""` | Value for `Access-Control-Allow-Methods` on preflight |
| `RWS_CONFIG_CORS_ALLOW_HEADERS` | `""` | Value for `Access-Control-Allow-Headers` on preflight |
| `RWS_CONFIG_CORS_EXPOSE_HEADERS` | `""` | Value for `Access-Control-Expose-Headers` |
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

## Automatic security headers

The following headers are added to every response regardless of TLS state:

| Header | Default value |
|---|---|
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `SAMEORIGIN` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `Permissions-Policy` | `geolocation=(), microphone=(), camera=()` |
| `Content-Security-Policy` | `default-src 'self'` (overridable) |

On TLS connections (HTTPS and HTTP/3) the server additionally sends:

```
Strict-Transport-Security: max-age=31536000; includeSubDomains
```

### Content-Security-Policy

Override the default CSP via `RWS_CONFIG_CSP`:

```bash
RWS_CONFIG_CSP="default-src 'self'; script-src 'self' cdn.example.com; img-src *"
```

Set the variable to an empty string to suppress the header entirely:

```bash
RWS_CONFIG_CSP=
```

## Client Hints

On every response the server advertises the following Client Hints via `Accept-CH` and `Critical-CH`:

```
Accept-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List,
           Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT,
           Save-Data, Device-Memory, Sec-CH-Prefers-Reduced-Motion,
           Sec-CH-Prefers-Color-Scheme
```

The hints also appear in the `Vary` header so caches key responses on them. Reading an incoming hint in a handler:

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
The `Vary` response header always includes the full list of Client Hints so HTTP caches serve the correct variant per client capability.
:::
