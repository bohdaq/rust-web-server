---
title: IP Filtering
description: Allow or deny requests by client IPv4 or IPv6 address or CIDR range with the IpFilter middleware.
---

## Quick start

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::ip_filter::IpFilter;

// Allowlist — only internal networks pass
let app = App::new()
    .wrap(IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16", "127.0.0.1"]));

// Denylist — block a known-bad range
let app = App::new()
    .wrap(IpFilter::deny(["1.2.3.4", "5.6.7.0/24"]));
```

## Modes

### `IpFilter::allow(entries)`

**Allowlist mode.** Only requests from IPs that match one of the entries pass through. All others receive `403 Forbidden`.

```rust
IpFilter::allow([
    "127.0.0.1",          // exact loopback
    "10.0.0.0/8",         // entire 10.x.x.x range
    "192.168.1.0/24",     // specific subnet
])
```

### `IpFilter::deny(entries)`

**Denylist mode.** Requests from IPs that match one of the entries receive `403 Forbidden`. All others pass through.

```rust
IpFilter::deny([
    "1.2.3.4",            // single bad actor
    "192.0.2.0/24",       // TEST-NET-1 documentation range
])
```

## Entry formats

Each entry in the list is an exact IPv4 or IPv6 address, or a CIDR range:

| Format | Example | Matches |
|---|---|---|
| Exact address | `"10.0.0.1"` | Only `10.0.0.1` |
| CIDR /8 | `"10.0.0.0/8"` | `10.0.0.0` – `10.255.255.255` |
| CIDR /16 | `"192.168.0.0/16"` | `192.168.0.0` – `192.168.255.255` |
| CIDR /24 | `"192.168.1.0/24"` | `192.168.1.0` – `192.168.1.255` |
| CIDR /32 | `"10.0.0.1/32"` | Only `10.0.0.1` |
| IPv6 address | `"2001:db8::1"` | Only `2001:db8::1` |
| IPv6 CIDR | `"2001:db8::/32"` | `2001:db8::` – `2001:db8:ffff:…` |

Malformed entries (invalid syntax, a prefix longer than 32 for IPv4 or 128
for IPv6) are skipped at construction time. `ip_filter::is_valid_entry(entry)`
tells you whether an entry parses, and `rws --check` reports invalid entries in
`rws.config.toml`.

## IPv4 and IPv6 clients

`rws` listens on IPv4 and IPv6. An IPv4 client reaching a dual-stack socket is
reported as an IPv4-mapped address (`::ffff:10.1.2.3`); IPv4 rules match it
exactly as they match `10.1.2.3`. IPv4 rules never match native IPv6 clients
and vice versa, so a deny list meant for everyone needs both families — for
example `["0.0.0.0/0", "::/0"]`.

## Use case: restrict admin routes to internal network

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::ip_filter::IpFilter;
use rust_web_server::state::AppWithState;
use std::sync::Arc;

struct State;

let admin_only = IpFilter::allow(["10.0.0.0/8", "127.0.0.1"]);

let app = App::with_state(Arc::new(State))
    .get("/admin/metrics", |_state, _req, _params, _conn| {
        // only reachable from internal IPs
        Response::ok("metrics data")
    })
    .wrap(admin_only);
```

For more granular control — applying the filter only to `/admin/*` routes rather than the whole app — wrap an inner router instead of the top-level app:

```rust
use rust_web_server::router::Router;
use rust_web_server::middleware::WithMiddleware;

let admin_router = Router::new()
    .get("/admin/config", admin_config_handler)
    .get("/admin/metrics", admin_metrics_handler);

// Wrap only the admin router with IP filtering
let protected = WithMiddleware::new(admin_router)
    .wrap(IpFilter::allow(["10.0.0.0/8", "127.0.0.1"]));
```

## Composing with other middleware

`IpFilter` implements `Middleware` and stacks cleanly with rate limiting, authentication, and rewrite layers:

```rust
use rust_web_server::rate_limit::RateLimitLayer;
use rust_web_server::auth::BasicAuthLayer;

let app = App::new()
    .wrap(IpFilter::allow(["10.0.0.0/8"]))  // checked first
    .wrap(BasicAuthLayer::new("user", "pass"))
    .wrap(RateLimitLayer::global());
```

:::note[Client IP source]
The client IP is read from `ConnectionInfo::client.ip`, which is the remote address of the TCP connection. If the server sits behind a load balancer or reverse proxy, configure the proxy to forward the real client IP and adjust your handler logic accordingly — `IpFilter` does not inspect `X-Forwarded-For` headers.
:::
