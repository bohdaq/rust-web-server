---
title: Configuration Overview
description: How rust-web-server loads, layers, and hot-reloads its configuration at runtime.
---

`rust-web-server` uses a four-layer configuration system. Each layer can override
the one below it. All resolved values are stored as process environment variables
under the `RWS_CONFIG_*` namespace and read at request time — there is no config
struct passed through the call stack.

## Priority order (lowest to highest)

| Priority | Source | Notes |
|----------|--------|-------|
| 1 | Built-in defaults | Hardcoded in `src/entry_point/mod.rs` |
| 2 | System environment variables | Set before the process starts |
| 3 | `rws.config.toml` | Optional file in the working directory |
| 4 | CLI arguments | Flags passed on the command line |

A value set at a higher priority layer always wins. For example, `--port=9000` on
the command line overrides `port = 8080` in `rws.config.toml`, which in turn
overrides `RWS_CONFIG_PORT=8888` in the environment.

## How startup loading works

`bootstrap()` in `src/entry_point/mod.rs` runs three steps in order:

```rust
read_system_environment_variables();       // step 1: read existing env vars
override_environment_variables_from_config(None); // step 2: apply rws.config.toml
override_environment_variables_from_command_line_args(); // step 3: apply CLI flags
```

After all three steps, any `RWS_CONFIG_*` variable that was never set by any
source gets its compiled-in default value. From that point on, the server reads
configuration exclusively through `std::env::var("RWS_CONFIG_*")`.

The config file path can be overridden at the OS level:

```bash
RWS_CONFIG_FILE=/etc/rws/production.toml rws
```

## Hot reload

Some settings can be changed while the server is running without a restart.
Trigger a reload with either:

```bash
# Unix signal
kill -HUP $(pidof rws)

# HTTP endpoint (no body required)
curl -X POST http://localhost:7878/admin/config/reload
```

### What reloads

| Setting | Environment variable |
|---------|---------------------|
| CORS — all fields | `RWS_CONFIG_CORS_*` |
| Rate-limit thresholds | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS`, `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` |
| Log format | `RWS_CONFIG_LOG_FORMAT` |
| Request allocation size | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |

On TLS builds, `SIGHUP` also rebuilds the `TlsAcceptor` from the updated
certificate files for all virtual hosts — no restart needed to rotate certs.

### What requires a restart

| Setting | Reason |
|---------|--------|
| IP / Port | The bound socket cannot be moved |
| Thread count | The thread pool is fixed at startup |
| TLS cert / key paths (plain HTTP build) | The acceptor is built once |

:::note[Rate-limit changes are instant]
When a reload fires, the live global `RateLimiter` is updated in place via
`set_limits()`. Existing per-IP counters are preserved — only the limit values
change. The new limits apply to the very next request.
:::

## Reading config in handler code

`config_reload::current()` returns a typed `ConfigSnapshot` containing all
hot-reloadable values. It takes a brief read lock and clones a handful of
strings — safe to call on every request.

```rust
use rust_web_server::config_reload;

let cfg = config_reload::current();
if cfg.cors_allow_all {
    // allow any origin
}
println!("log format: {}", cfg.log_format);
```

The `ConfigSnapshot` fields map directly to environment variables:

| Field | Environment variable |
|-------|---------------------|
| `cors_allow_all` | `RWS_CONFIG_CORS_ALLOW_ALL` |
| `cors_allow_origins` | `RWS_CONFIG_CORS_ALLOW_ORIGINS` |
| `cors_allow_credentials` | `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` |
| `cors_allow_methods` | `RWS_CONFIG_CORS_ALLOW_METHODS` |
| `cors_allow_headers` | `RWS_CONFIG_CORS_ALLOW_HEADERS` |
| `cors_expose_headers` | `RWS_CONFIG_CORS_EXPOSE_HEADERS` |
| `cors_max_age` | `RWS_CONFIG_CORS_MAX_AGE` |
| `rate_limit_max_requests` | `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` |
| `rate_limit_window_secs` | `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` |
| `log_format` | `RWS_CONFIG_LOG_FORMAT` |
| `request_allocation_size` | `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` |

For settings that are not in `ConfigSnapshot` (IP, port, thread count, TLS
paths) read them directly:

```rust
let port = std::env::var("RWS_CONFIG_PORT").unwrap_or_else(|_| "7878".to_string());
```
