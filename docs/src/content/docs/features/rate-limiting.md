---
title: Rate Limiting
description: Per-IP sliding-window rate limiting with a global singleton and per-route custom limiters.
---

`rust-web-server` ships a built-in sliding-window rate limiter in `src/rate_limit/mod.rs`. It is keyed by a string (typically the client IP) and is safe to share across threads.

## Quick start

Add `RateLimitLayer` to the middleware stack to enforce the global limit on every request:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::middleware::RateLimitLayer;

let app = App::new().wrap(RateLimitLayer);
```

`RateLimitLayer` calls `rate_limit::global().check(&connection.client.ip)` on every request. When the budget is exceeded it returns `429 Too Many Requests` without forwarding to the inner application.

## Global singleton

`rate_limit::global()` returns a `&'static RateLimiter` initialized once from environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `1000` | Requests allowed per window |
| `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `60` | Sliding window length in seconds |

```bash
RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS=500
RWS_CONFIG_RATE_LIMIT_WINDOW_SECS=30
```

## `RateLimiter` API

### `check(key) -> bool`

Records a timestamp for `key` and returns `true` when the call is within the budget. Returns `false` — without recording — when the budget is exhausted.

```rust
use rust_web_server::rate_limit::RateLimiter;

let limiter = RateLimiter::new(100, 60); // 100 req / 60 s

if limiter.check("192.168.1.1") {
    // request is within budget — proceed
} else {
    // return 429 Too Many Requests
}
```

### `remaining(key) -> u32`

Returns the number of requests `key` may still make in the current window without being rate-limited. Useful for `X-RateLimit-Remaining` response headers.

```rust
let left = limiter.remaining("192.168.1.1");
// add to response headers:
// X-RateLimit-Remaining: {left}
```

### `set_limits(max_requests, window_secs)`

Updates the limits on a live limiter without a restart. Called automatically by `config_reload::reload()` on `SIGHUP`.

```rust
limiter.set_limits(200, 60); // new limit takes effect immediately
```

### `reset(key)`

Clears all recorded timestamps for `key`. Useful in tests.

```rust
limiter.reset("192.168.1.1");
```

## Hot reload

Send `SIGHUP` or `POST /admin/config/reload` to update the global limiter without restarting. The new `RWS_CONFIG_RATE_LIMIT_*` values are read from the environment and applied atomically via `set_limits`.

## Custom per-route limiter

Create a `RateLimiter` with custom parameters and call `check` in your handler directly:

```rust
use std::sync::Arc;
use rust_web_server::rate_limit::RateLimiter;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::error::{AppError, IntoResponse};

struct State {
    login_limiter: RateLimiter,
}

fn login_handler(
    state: &Arc<State>,
    req: &Request,
    conn: &ConnectionInfo,
) -> Response {
    if !state.login_limiter.check(&conn.client.ip) {
        return AppError::TooManyRequests.into_response();
    }
    // proceed with login logic ...
    Response::new()
}

// Wire up:
let state = Arc::new(State {
    login_limiter: RateLimiter::new(5, 60), // 5 attempts per minute
});
```

Wrap the limiter in `Arc` to share it across handler invocations.

:::note[Per-route vs global]
`RateLimitLayer` uses the global singleton. For endpoints that need tighter limits (login, password reset, OTP verification), create a separate `RateLimiter` and call `check` inside the handler as shown above.
:::
