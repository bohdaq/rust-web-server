---
title: Rate Limiting
description: Per-IP sliding-window rate limiting with a global singleton, per-route custom limiters, and a Redis-backed distributed limiter for multi-instance deployments.
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

## Distributed rate limiting with `RedisRateLimiter`

`RateLimiter` tracks state in process memory. Run two or more `rws` instances behind a load balancer and each one enforces its own budget independently — the effective limit for a given client multiplies by the number of replicas instead of staying constant. `RedisRateLimiter` fixes this by keying the counter on a shared Redis server, so every instance increments and reads the same value.

```rust
use rust_web_server::rate_limit::RedisRateLimiter;

// 100 requests / 60s, shared across every rws instance pointed at this Redis server
let limiter = RedisRateLimiter::new("127.0.0.1:6379", None, 100, 60);

match limiter.check("192.168.1.1") {
    Ok(true) => { /* within budget — proceed */ }
    Ok(false) => { /* budget exhausted — return 429 */ }
    Err(e) => { /* Redis unreachable — decide fail-open vs fail-closed yourself */ }
}
```

`RedisRateLimiter::from_env()` builds one from environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `RWS_REDIS_HOST` | `127.0.0.1` | Redis server host |
| `RWS_REDIS_PORT` | `6379` | Redis server port |
| `RWS_REDIS_PASSWORD` | *(none)* | Passed to Redis `AUTH` if set |
| `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` | `1000` | Requests allowed per window |
| `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` | `60` | Window length in seconds |

```rust
use rust_web_server::rate_limit::RedisRateLimiter;

let limiter = RedisRateLimiter::from_env();
```

### How it differs from the in-process `RateLimiter`

`RateLimiter` records a sliding-window log of timestamps per key. `RedisRateLimiter` instead uses a **fixed-window counter**: each key lives under one Redis string with a TTL equal to the window length, and every call issues an atomic `INCR`. This is the standard distributed rate-limiting pattern — Redis guarantees `INCR` is atomic across every concurrent caller, so the count stays correct no matter how many `rws` processes are incrementing it at once.

The tradeoff versus a true sliding window: a client can burst up to `2x max_requests` across a window boundary (once near the end of one window, once right after the next one starts). For most deployments this is an acceptable price for atomic, coordination-free enforcement across a cluster.

`check`, unlike `RateLimiter::check`, returns `Result<bool, std::io::Error>` — the network call to Redis can fail. Callers decide whether an unreachable Redis server should fail open (allow the request) or fail closed (reject it); `RedisRateLimiter` does not pick one for you.

### API

- `RedisRateLimiter::new(addr, password, max_requests, window_secs)` — connect to `addr` (e.g. `"127.0.0.1:6379"`).
- `RedisRateLimiter::from_env()` — build from the environment variables above.
- `check(key) -> Result<bool, io::Error>` — atomically increments the shared counter for `key` and returns whether the call is within budget.
- `remaining(key) -> Result<u32, io::Error>` — requests `key` may still make in the current window.
- `set_limits(max_requests, window_secs)` — update limits on a live limiter; takes effect on the next `check`/`remaining`.
- `reset(key) -> Result<(), io::Error>` — deletes the Redis key for `key`. Useful in tests.

Cloning a `RedisRateLimiter` is cheap — all clones share the same underlying TCP connection, mirroring `RedisSessionStore`'s connection-sharing model (see [Sessions](/features/sessions/)).
