---
title: Typed Config Binding
description: Derive a load() method that reads environment variables into a strongly-typed config struct at startup.
---

## Setup

Enable the `macros` feature in `Cargo.toml`:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["macros"] }
```

## Quick start

```rust
use rust_web_server::Config;

#[derive(rust_web_server::Config)]
#[config(prefix = "APP_")]
struct AppConfig {
    #[config(env = "PORT", default = "8080")]
    port: u16,

    #[config(env = "DATABASE_URL")]
    database_url: String,

    #[config(env = "DEBUG")]
    debug: Option<bool>,
}

fn main() {
    let cfg = AppConfig::load().expect("invalid config");
    println!("listening on port {}", cfg.port);
}
```

## `#[derive(Config)]`

The derive macro generates a single method on the struct:

```rust
impl AppConfig {
    pub fn load() -> Result<Self, String> { ... }
}
```

`load()` reads every field from environment variables and returns `Err(String)` with a descriptive message on the first parse failure.

## Struct-level attribute

```rust
#[config(prefix = "APP_")]
```

When set, every field's environment variable key is `prefix + key`. With `prefix = "APP_"` and a field named `port`, the default env var is `APP_PORT`.

## Field-level attribute

```rust
#[config(env = "MY_PORT", default = "8080")]
```

| Option | Meaning |
|---|---|
| `env = "KEY"` | Explicit env var name. The struct prefix is still prepended. |
| `default = "v"` | Fallback string when the env var is absent. Parsed with the same `FromEnvStr` impl as a present value. |

If `env` is omitted the field name is uppercased (`pool_size` → `POOL_SIZE`), then the prefix is prepended.

## Field derivation rules

| Field declaration | Env var absent | Env var present |
|---|---|---|
| `field: T` with `default` | parse `default` value | parse env var |
| `field: T` without `default` | `Err` — required | parse env var |
| `field: Option<T>` | `Ok(None)` | parse env var, wrap in `Some` |

## `FromEnvStr` trait

All primitive Rust scalar types implement `FromEnvStr` out of the box:

`String`, `bool`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `f32`, `f64`

`bool` accepts `"true"`, `"1"`, `"yes"` → `true`; `"false"`, `"0"`, `"no"` → `false`.

Implement `FromEnvStr` on your own type to use it as a config field:

```rust
use rust_web_server::config_binding::FromEnvStr;

#[derive(Debug)]
enum LogLevel { Error, Warn, Info, Debug }

impl FromEnvStr for LogLevel {
    fn from_env_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn"  => Ok(LogLevel::Warn),
            "info"  => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            other   => Err(format!("unknown log level: {other}")),
        }
    }
}
```

## Complete example

```rust
use rust_web_server::config_binding::FromEnvStr;
use rust_web_server::Config;

#[derive(Debug)]
enum LogLevel { Error, Warn, Info, Debug }

impl FromEnvStr for LogLevel {
    fn from_env_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn"  => Ok(LogLevel::Warn),
            "info"  => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            other   => Err(format!("unknown log level: {other}")),
        }
    }
}

#[derive(Debug, rust_web_server::Config)]
#[config(prefix = "APP_")]
struct AppConfig {
    /// APP_HOST — defaults to "0.0.0.0"
    #[config(env = "HOST", default = "0.0.0.0")]
    host: String,

    /// APP_PORT — defaults to 8080
    #[config(env = "PORT", default = "8080")]
    port: u16,

    /// APP_DB_HOST — required, no default
    #[config(env = "DB_HOST")]
    db_host: String,

    /// APP_DB_PORT — defaults to 5432
    #[config(env = "DB_PORT", default = "5432")]
    db_port: u16,

    /// APP_DB_POOL_SIZE — defaults to 10
    #[config(env = "DB_POOL_SIZE", default = "10")]
    db_pool_size: u32,

    /// APP_LOG_LEVEL — optional; None if not set
    #[config(env = "LOG_LEVEL")]
    log_level: Option<LogLevel>,
}

fn main() {
    let cfg = AppConfig::load().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    println!("starting on {}:{}", cfg.host, cfg.port);
    println!("db: {}:{} pool={}", cfg.db_host, cfg.db_port, cfg.db_pool_size);
    if let Some(level) = cfg.log_level {
        println!("log level: {:?}", level);
    }
}
```

Set environment variables before running:

```bash
export APP_DB_HOST=postgres.internal
export APP_LOG_LEVEL=info
cargo run
```

:::note[Error messages]
`load()` returns `Err` on the first failure with a message like `` `APP_DB_HOST`: required env var `APP_DB_HOST` is not set `` or `` `APP_PORT`: invalid digit found in string ``. Check the message to identify which variable caused the problem.
:::

:::note[No config file]
`#[derive(Config)]` reads only from environment variables. To support `.env` files or `rws.config.toml`, set those variables in the environment before calling `load()`, or use a crate like `dotenvy` to load a `.env` file at the top of `main()`.
:::
