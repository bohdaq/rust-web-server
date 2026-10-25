---
title: Hot Config Reload
description: Update proxy routes, CORS rules, rate limits, log format, TLS certificates, and Tera templates without restarting the server.
---

## Triggering a reload

Two mechanisms trigger a hot reload:

```bash
# Unix signal — sends SIGHUP to the running server
kill -HUP $(pgrep rws)
# or by PID
kill -HUP $(pidof rws)
```

```bash
# HTTP endpoint — no request body required
curl -X POST http://localhost:8080/admin/config/reload
```

Both re-parse `rws.config.toml`, update process environment variables, apply the new rate-limit thresholds to the live `RateLimiter`, and publish a new `ConfigSnapshot` atomically. On `http2`/`http3` builds, `SIGHUP` also rebuilds the `TlsAcceptor` with fresh certificates for every virtual host. If the `tera` feature is enabled and `template::init()` was called, both also re-glob Tera templates from disk — see [Templates: hot reload](/building-apps/templates/#hot-reload).

Request handling reads settings from a cached snapshot (`ServerConfig::current()`), not from the environment on every request; a reload rebuilds it, so changes apply from the next request. A library user who changes an `RWS_CONFIG_*` variable with `std::env::set_var` calls `rust_web_server::server_config::invalidate()` afterwards for the same effect.

## What reloads without restart

| Setting | Env var |
|---|---|
| CORS — all fields | `RWS_CONFIG_CORS_*` |
| Rate-limit max requests | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` |
| Rate-limit window (seconds) | `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` |
| Log format (`json` / `combined` / `off`) | `RWS_CONFIG_LOG_FORMAT` |
| Request allocation size | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |
| TLS certificates (http2+ builds) | `RWS_CONFIG_TLS_CERT_FILE`, `RWS_CONFIG_TLS_KEY_FILE` |
| Tera templates (`tera` feature) | re-read from the directory passed to `template::init()` — see [Templates: hot reload](/building-apps/templates/#hot-reload) |
| Proxy routes, upstreams, health checks, `[middleware]` | `[[route]]`, `[[upstream]]`, `[middleware]` in `rws.config.toml` — see below |

## Reloading proxy routes

In proxy mode (a config file with `[[route]]` or `[[upstream]]` sections), a
reload also rebuilds the routes. Edit the file, check it, then signal:

```bash
rws --check && kill -HUP $(pgrep rws)
```

The new file is validated first — the same checks as
[`rws --check`](/configuration/config-file/#check-the-file-before-you-deploy).
If it has errors, they are logged and **the running routes keep serving**; a
typo never takes the proxy down:

```text
[proxy_config] reload ERROR: route 'api': unknown action type 'proxxy' (expected proxy, grpc, static, redirect or respond)
[proxy_config] rws.config.toml has errors — keeping the running routes
```

Otherwise the new routes take effect for the next request, requests already
in progress finish on the old ones, health checkers for the old upstreams stop
and new ones start. A reload of an unchanged file does nothing.

## What requires a restart

| Setting | Why |
|---|---|
| IP address | Bound socket cannot be moved |
| Port | Bound socket cannot be moved |
| Thread count | Thread pool is fixed at startup |
| `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]` | Their listener sockets are bound at startup; a reload that changes them logs a warning |

## `ConfigSnapshot`

Call `config_reload::current()` anywhere in the handler stack to get a typed snapshot of all hot-reloadable values at that instant. The call takes a brief read lock and clones a handful of strings — safe to call on every request.

```rust
use rust_web_server::config_reload;

fn my_handler(req: &Request, conn: &ConnectionInfo) -> Response {
    let cfg = config_reload::current();

    if cfg.cors_allow_all {
        // serve with open CORS
    }

    println!(
        "rate limit: {}/{} log: {}",
        cfg.rate_limit_max_requests,
        cfg.rate_limit_window_secs,
        cfg.log_format,
    );

    Response::new()
}
```

### `ConfigSnapshot` fields

| Field | Type | Source env var |
|---|---|---|
| `cors_allow_all` | `bool` | `RWS_CONFIG_CORS_ALLOW_ALL` |
| `cors_allow_origins` | `String` | `RWS_CONFIG_CORS_ALLOW_ORIGINS` |
| `cors_allow_methods` | `String` | `RWS_CONFIG_CORS_ALLOW_METHODS` |
| `cors_allow_headers` | `String` | `RWS_CONFIG_CORS_ALLOW_HEADERS` |
| `cors_allow_credentials` | `String` | `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` |
| `cors_expose_headers` | `String` | `RWS_CONFIG_CORS_EXPOSE_HEADERS` |
| `cors_max_age` | `String` | `RWS_CONFIG_CORS_MAX_AGE` |
| `rate_limit_max_requests` | `u32` | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` |
| `rate_limit_window_secs` | `u64` | `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` |
| `log_format` | `String` | `RWS_CONFIG_LOG_FORMAT` |
| `request_allocation_size` | `i64` | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |
| `max_body_size` | `u64` | `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` |

## Triggering reload from code

`RELOAD_REQUESTED` is an `AtomicBool` in the `config_reload` module. Set it to `true` to trigger a reload on the next connection cycle, or call `config_reload::reload()` directly from any thread:

```rust
use rust_web_server::config_reload;

// Option A: set the flag (picked up between connections)
config_reload::RELOAD_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);

// Option B: call reload() immediately from a handler
fn reload_handler(_req: &Request, _conn: &ConnectionInfo) -> Response {
    config_reload::reload();
    Response::new() // 200 OK
}
```

## Pattern: dynamic behaviour from live config

```rust
fn rate_limit_info(_req: &Request, _conn: &ConnectionInfo) -> Response {
    let cfg = config_reload::current();
    let body = format!(
        r#"{{"max_requests":{},"window_secs":{}}}"#,
        cfg.rate_limit_max_requests, cfg.rate_limit_window_secs
    );
    Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, body.into_bytes())
}
```

:::note[Signal safety]
The `SIGHUP` handler does only one thing: it stores `true` into the `AtomicBool`. The actual `reload()` call happens on the main thread between connection accepts — no allocation, no I/O, no locks inside the signal handler.
:::
