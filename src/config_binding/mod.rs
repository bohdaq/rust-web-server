//! Typed configuration binding — read environment variables into strongly-typed structs.
//!
//! Use `#[derive(Config)]` (requires `macros` feature) to generate a `load()` method
//! that reads env vars and parses them into the annotated field types.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use rust_web_server::Config;
//!
//! #[derive(rust_web_server::Config)]
//! #[config(prefix = "APP_")]
//! struct MyConfig {
//!     #[config(env = "PORT", default = "8080")]
//!     port: u16,
//!
//!     #[config(env = "DATABASE_URL")]
//!     database_url: String,   // required — Err if env var is absent
//!
//!     #[config(env = "FEATURE_FLAG")]
//!     feature_flag: Option<bool>, // None if env var is absent or empty
//! }
//!
//! // At startup:
//! let cfg = MyConfig::load().expect("failed to load config");
//! println!("listening on port {}", cfg.port);
//! ```
//!
//! # Field derivation rules
//!
//! | Field annotation | Env var absent | Env var present |
//! |---|---|---|
//! | `#[config(env = "KEY", default = "v")]` | use `"v"` | parse to field type |
//! | `#[config(env = "KEY")]` (non-Option) | `Err` | parse to field type |
//! | `#[config(env = "KEY")]` (`Option<T>`) | `Ok(None)` | parse, wrap in `Some` |
//! | No `#[config]` — uses `PREFIX + SCREAMING_SNAKE_CASE(field)` | same as non-Option rules |
//!
//! # Supported types
//!
//! All primitive Rust scalar types implement [`FromEnvStr`] and can be used as field types:
//! `String`, `bool`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`,
//! `i128`, `isize`, `f32`, `f64`. Wrap in `Option<T>` for optional fields.

#[cfg(test)]
mod tests;

/// Parse a value from an environment variable string.
///
/// Implement this trait to support custom field types in `#[derive(Config)]` structs.
pub trait FromEnvStr: Sized {
    fn from_env_str(s: &str) -> Result<Self, String>;
}

impl FromEnvStr for String {
    fn from_env_str(s: &str) -> Result<Self, String> {
        Ok(s.to_string())
    }
}

impl FromEnvStr for bool {
    fn from_env_str(s: &str) -> Result<Self, String> {
        match s.trim() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            other => Err(format!("expected true/false/1/0/yes/no, got {:?}", other)),
        }
    }
}

macro_rules! impl_from_env_str_via_parse {
    ($($t:ty),+) => {
        $(
            impl FromEnvStr for $t {
                fn from_env_str(s: &str) -> Result<Self, String> {
                    s.trim().parse::<$t>().map_err(|e| e.to_string())
                }
            }
        )+
    };
}

impl_from_env_str_via_parse!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

// ── Runtime helpers called by generated `load()` code ────────────────────────

/// Read a required env var and parse it into `T`.
///
/// Returns `Err` if the variable is not set or if parsing fails.
pub fn load_required<T: FromEnvStr>(key: &str) -> Result<T, String> {
    let val = std::env::var(key)
        .map_err(|_| format!("required env var `{}` is not set", key))?;
    T::from_env_str(&val).map_err(|e| format!("`{}`: {}", key, e))
}

/// Read an env var and parse it into `T`, falling back to `default` when the variable is absent.
///
/// Returns `Err` if parsing fails on either the env var value or the default.
pub fn load_with_default<T: FromEnvStr>(key: &str, default: &str) -> Result<T, String> {
    let val = std::env::var(key).unwrap_or_else(|_| default.to_string());
    T::from_env_str(&val).map_err(|e| format!("`{}`: {}", key, e))
}

/// Read an optional env var and parse it into `Option<T>`.
///
/// Returns `Ok(None)` when the variable is absent or empty.
/// Returns `Err` if the variable is present but parsing fails.
pub fn load_optional<T: FromEnvStr>(key: &str) -> Result<Option<T>, String> {
    match std::env::var(key) {
        Ok(val) if !val.trim().is_empty() => T::from_env_str(&val)
            .map(Some)
            .map_err(|e| format!("`{}`: {}", key, e)),
        _ => Ok(None),
    }
}
