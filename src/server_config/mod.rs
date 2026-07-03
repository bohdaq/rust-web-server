//! Typed server configuration.
//!
//! [`ServerConfig`] holds all per-instance configuration fields as typed Rust
//! values. It replaces point-of-use `env::var("RWS_CONFIG_*")` calls with a
//! struct that can be:
//!
//! - Built from environment variables once at startup: [`ServerConfig::from_env()`]
//! - Constructed directly in tests without touching the environment:
//!   [`ServerConfig::default()`] / struct update syntax
//! - Passed to [`App::with_config`] to create a fully isolated application
//!   instance — essential for parallel tests and embedded multi-tenant use
//!
//! # Test isolation example
//!
//! ```rust,ignore
//! use rust_web_server::app::App;
//! use rust_web_server::server_config::ServerConfig;
//! use rust_web_server::test_client::TestClient;
//!
//! // No env writes, no lock needed.
//! let app = App::with_config(ServerConfig {
//!     cors_allow_all: false,
//!     cors_allow_origins: "https://example.com".to_string(),
//!     ..ServerConfig::default()
//! });
//! let client = TestClient::new(app);
//! let res = client.get("/").send();
//! ```

#[cfg(test)]
mod tests;

use crate::entry_point::Config;

/// Default `Content-Security-Policy` header value. Mirrors
/// `Header::_CONTENT_SECURITY_POLICY_VALUE_DEFAULT` without creating a
/// circular import (`server_config` ← `header` ← `cors` ← `server_config`).
const CSP_DEFAULT: &str = "default-src 'self'";

/// All runtime-configurable settings for one server instance.
///
/// Fields map 1-to-1 to `RWS_CONFIG_*` environment variable names documented
/// in [`Config`]. Default values match the environment-variable defaults.
///
/// Construct via [`ServerConfig::from_env()`] at startup or
/// [`ServerConfig::default()`] in tests.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    // ── CORS ─────────────────────────────────────────────────────────────────
    /// `RWS_CONFIG_CORS_ALLOW_ALL` — when `true`, all cross-origin requests are
    /// reflected back as allowed (echo the `Origin` header). Overrides all
    /// other CORS fields. Default: `true`.
    pub cors_allow_all: bool,
    /// `RWS_CONFIG_CORS_ALLOW_ORIGINS` — comma-separated list of allowed
    /// origins when `cors_allow_all` is `false`. Default: `""` (none).
    pub cors_allow_origins: String,
    /// `RWS_CONFIG_CORS_ALLOW_CREDENTIALS` — value for the
    /// `Access-Control-Allow-Credentials` response header. Default: `""`.
    pub cors_allow_credentials: String,
    /// `RWS_CONFIG_CORS_ALLOW_METHODS` — value for
    /// `Access-Control-Allow-Methods`. Default: `""`.
    pub cors_allow_methods: String,
    /// `RWS_CONFIG_CORS_ALLOW_HEADERS` — value for
    /// `Access-Control-Allow-Headers`. Default: `""`.
    pub cors_allow_headers: String,
    /// `RWS_CONFIG_CORS_EXPOSE_HEADERS` — value for
    /// `Access-Control-Expose-Headers`. Default: `""`.
    pub cors_expose_headers: String,
    /// `RWS_CONFIG_CORS_MAX_AGE` — value for `Access-Control-Max-Age`.
    /// Default: `"86400"`.
    pub cors_max_age: String,

    // ── Security headers ──────────────────────────────────────────────────────
    /// `RWS_CONFIG_CSP` — `Content-Security-Policy` header value. An empty
    /// string suppresses the header entirely. Default: the framework default CSP.
    pub csp: String,

    // ── Server internals ──────────────────────────────────────────────────────
    /// `RWS_CONFIG_LOG_FORMAT` — `"json"` or `"combined"`. Default: `"json"`.
    pub log_format: String,
    /// `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` — bytes allocated for
    /// incoming request parsing. Default: `10000`.
    pub request_allocation_size: i64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            cors_allow_all: Config::RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE
                .eq_ignore_ascii_case("true"),
            cors_allow_origins: Config::RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE.to_string(),
            cors_allow_credentials: Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE
                .to_string(),
            cors_allow_methods: Config::RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE.to_string(),
            cors_allow_headers: Config::RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE.to_string(),
            cors_expose_headers: Config::RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE.to_string(),
            cors_max_age: Config::RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE.to_string(),
            csp: CSP_DEFAULT.to_string(),
            log_format: Config::RWS_CONFIG_LOG_FORMAT_DEFAULT_VALUE.to_string(),
            request_allocation_size: *Config::RWS_DEFAULT_REQUEST_ALLOCATION_SIZE_IN_BYTES,
        }
    }
}

impl ServerConfig {
    /// Build a `ServerConfig` by reading all `RWS_CONFIG_*` environment
    /// variables. Missing variables fall back to their default values.
    ///
    /// Call this once at startup (inside `App::new()`) rather than on every
    /// request. For hot-reload, use `config_reload::current()` to get a
    /// fresh snapshot after a `SIGHUP`/`POST /admin/config/reload`.
    pub fn from_env() -> Self {
        let read = |key: &str| std::env::var(key).unwrap_or_default();
        Self {
            cors_allow_all: read(Config::RWS_CONFIG_CORS_ALLOW_ALL)
                .eq_ignore_ascii_case("true"),
            cors_allow_origins: read(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS),
            cors_allow_credentials: read(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS),
            cors_allow_methods: read(Config::RWS_CONFIG_CORS_ALLOW_METHODS),
            cors_allow_headers: read(Config::RWS_CONFIG_CORS_ALLOW_HEADERS),
            cors_expose_headers: read(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS),
            cors_max_age: read(Config::RWS_CONFIG_CORS_MAX_AGE),
            csp: std::env::var("RWS_CONFIG_CSP")
                .unwrap_or_else(|_| CSP_DEFAULT.to_string()),
            log_format: read(Config::RWS_CONFIG_LOG_FORMAT),
            request_allocation_size: std::env::var(
                Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES,
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(*Config::RWS_DEFAULT_REQUEST_ALLOCATION_SIZE_IN_BYTES),
        }
    }
}
