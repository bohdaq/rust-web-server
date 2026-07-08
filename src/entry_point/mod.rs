#[cfg(test)]
mod tests;
pub mod command_line_args;
pub mod config_file;
pub mod environment_variables;


use std::{env};
use crate::virtual_host::VirtualHostConfig;

use crate::entry_point::command_line_args::{override_environment_variables_from_command_line_args};
use crate::entry_point::config_file::override_environment_variables_from_config;
use crate::entry_point::environment_variables::read_system_environment_variables;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Config {}

impl Config {
    pub const RWS_CONFIG_IP: &'static str = "RWS_CONFIG_IP";
    /// Default is `0.0.0.0` so the server is reachable inside containers and K8s pods.
    /// For local development you can override to `127.0.0.1` via env var or config file.
    pub const RWS_CONFIG_IP_DEFAULT_VALUE: &'static str = "0.0.0.0";

    /// Log format: `"combined"` (default, Combined Log Format) or `"json"` (structured JSON).
    pub const RWS_CONFIG_LOG_FORMAT: &'static str = "RWS_CONFIG_LOG_FORMAT";
    pub const RWS_CONFIG_LOG_FORMAT_DEFAULT_VALUE: &'static str = "json";

    pub const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub const RWS_CONFIG_PORT_DEFAULT_VALUE: &'static str = "7878";

    pub const RWS_CONFIG_THREAD_COUNT: &'static str = "RWS_CONFIG_THREAD_COUNT";
    pub const RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE: &'static str = "200";

    pub const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub const RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE: &'static str = "true";

    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub const RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub const RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";
    pub const RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE: &'static str = "86400";

    pub const RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES: &'static str = "RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES";
    pub const RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES_DEFAULT_VALUE: &'static str = "10000";

    /// Maximum accepted request body size in bytes, checked against `Content-Length`
    /// before any body bytes are read off the socket. `0` (the default) means
    /// unlimited — this is opt-in, not a default-on behavior change. Requests whose
    /// declared body exceeds this get `413 Payload Too Large` and the connection is
    /// closed (not kept alive), since bytes the server chose not to read are still
    /// queued on the client's side of the socket.
    pub const RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES: &'static str = "RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES";
    pub const RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES_DEFAULT_VALUE: &'static str = "0";

    /// Serve this file (e.g. `"index.html"`) for a `GET`/`HEAD` request that doesn't
    /// match any real file/directory, instead of `404` — the standard client-side-router
    /// ("SPA") fallback pattern needed for deep links in a React Router / Vue Router /
    /// etc. app. Empty (default) disables it entirely. See
    /// `crate::app::controller::static_resource` for the matching heuristic (skips
    /// paths that look like a missed static asset, i.e. the last segment has a `.`).
    pub const RWS_CONFIG_SPA_FALLBACK: &'static str = "RWS_CONFIG_SPA_FALLBACK";
    pub const RWS_CONFIG_SPA_FALLBACK_DEFAULT_VALUE: &'static str = "";

    /// Comma-separated path prefixes (e.g. `"/api,/healthz"`) that never receive the
    /// SPA fallback even when `RWS_CONFIG_SPA_FALLBACK` is set — so a real API `404`
    /// isn't silently rewritten into a `200`. Empty (default) excludes nothing.
    pub const RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES: &'static str = "RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES";
    pub const RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_TLS_CERT_FILE: &'static str = "RWS_CONFIG_TLS_CERT_FILE";
    pub const RWS_CONFIG_TLS_CERT_FILE_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_TLS_KEY_FILE: &'static str = "RWS_CONFIG_TLS_KEY_FILE";
    pub const RWS_CONFIG_TLS_KEY_FILE_DEFAULT_VALUE: &'static str = "";

    /// Path to a PEM-encoded CA certificate used to verify client certificates (mTLS).
    /// When set, the TLS handshake requires a valid client certificate signed by this CA.
    /// Connections without a valid cert are rejected at the TLS layer (before any HTTP processing).
    pub const RWS_CONFIG_TLS_CLIENT_CA_FILE: &'static str = "RWS_CONFIG_TLS_CLIENT_CA_FILE";
    pub const RWS_CONFIG_TLS_CLIENT_CA_FILE_DEFAULT_VALUE: &'static str = "";

    /// Directory containing Tera HTML templates (default: `"templates"`). Requires `tera` feature.
    pub const RWS_CONFIG_TEMPLATE_DIR: &'static str = "RWS_CONFIG_TEMPLATE_DIR";
    pub const RWS_CONFIG_TEMPLATE_DIR_DEFAULT_VALUE: &'static str = "templates";

    /// When non-empty, a plain-HTTP listener on this port redirects all requests to HTTPS.
    /// Set to e.g. `"80"` when running on standard ports. Requires TLS to be configured.
    pub const RWS_CONFIG_HTTP_REDIRECT_PORT: &'static str = "RWS_CONFIG_HTTP_REDIRECT_PORT";
    pub const RWS_CONFIG_HTTP_REDIRECT_PORT_DEFAULT_VALUE: &'static str = "";

    // ── ACME (Automatic Certificate Management Environment) ───────────────────

    /// Comma-separated list of domain names to obtain a certificate for.
    /// Setting this activates ACME at startup. Example: `"example.com,www.example.com"`
    pub const RWS_CONFIG_ACME_DOMAINS: &'static str = "RWS_CONFIG_ACME_DOMAINS";
    pub const RWS_CONFIG_ACME_DOMAINS_DEFAULT_VALUE: &'static str = "";

    /// Contact email sent to the CA. Recommended but not required.
    pub const RWS_CONFIG_ACME_EMAIL: &'static str = "RWS_CONFIG_ACME_EMAIL";
    pub const RWS_CONFIG_ACME_EMAIL_DEFAULT_VALUE: &'static str = "";

    /// Set to `"true"` to use the Let's Encrypt staging environment (for testing).
    pub const RWS_CONFIG_ACME_STAGING: &'static str = "RWS_CONFIG_ACME_STAGING";
    pub const RWS_CONFIG_ACME_STAGING_DEFAULT_VALUE: &'static str = "false";

    /// Custom ACME directory URL. Defaults to Let's Encrypt production.
    pub const RWS_CONFIG_ACME_DIRECTORY: &'static str = "RWS_CONFIG_ACME_DIRECTORY";
    pub const RWS_CONFIG_ACME_DIRECTORY_DEFAULT_VALUE: &'static str = "";

    /// Where to write the provisioned certificate chain (PEM). Defaults to `RWS_CONFIG_TLS_CERT_FILE`.
    pub const RWS_CONFIG_ACME_CERT_PATH: &'static str = "RWS_CONFIG_ACME_CERT_PATH";
    pub const RWS_CONFIG_ACME_CERT_PATH_DEFAULT_VALUE: &'static str = "";

    /// Where to write the certificate's private key (PEM). Defaults to `RWS_CONFIG_TLS_KEY_FILE`.
    pub const RWS_CONFIG_ACME_KEY_PATH: &'static str = "RWS_CONFIG_ACME_KEY_PATH";
    pub const RWS_CONFIG_ACME_KEY_PATH_DEFAULT_VALUE: &'static str = "";

    /// Port for the temporary HTTP-01 challenge server (default 80).
    /// Must be reachable from the internet on port 80. Not used with DNS-01.
    pub const RWS_CONFIG_ACME_CHALLENGE_PORT: &'static str = "RWS_CONFIG_ACME_CHALLENGE_PORT";
    pub const RWS_CONFIG_ACME_CHALLENGE_PORT_DEFAULT_VALUE: &'static str = "80";

    /// Renew when fewer than this many days remain on the certificate (default 30).
    pub const RWS_CONFIG_ACME_RENEW_BEFORE_DAYS: &'static str = "RWS_CONFIG_ACME_RENEW_BEFORE_DAYS";
    pub const RWS_CONFIG_ACME_RENEW_BEFORE_DAYS_DEFAULT_VALUE: &'static str = "30";

    /// Path to persist the ACME account key between restarts (default `acme_account.key`).
    pub const RWS_CONFIG_ACME_ACCOUNT_KEY_PATH: &'static str = "RWS_CONFIG_ACME_ACCOUNT_KEY_PATH";
    pub const RWS_CONFIG_ACME_ACCOUNT_KEY_PATH_DEFAULT_VALUE: &'static str = "acme_account.key";


    pub const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub const RWS_DEFAULT_PORT: &'static i32 = &7878;
    pub const RWS_DEFAULT_THREAD_COUNT: &'static i32 = &200;
    pub const RWS_DEFAULT_REQUEST_ALLOCATION_SIZE_IN_BYTES: &'static i64 = &10000;


}

pub fn bootstrap() {
    read_system_environment_variables();
    override_environment_variables_from_config(None);
    override_environment_variables_from_command_line_args();
}

pub fn set_default_values() {
    println!("  Initializing default values");

    let is_var_set = env::var(Config::RWS_CONFIG_IP).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_IP, Config::RWS_CONFIG_IP_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_IP, Config::RWS_CONFIG_IP_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_IP);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_PORT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_PORT, Config::RWS_CONFIG_PORT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_PORT, Config::RWS_CONFIG_PORT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_PORT);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES, Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES, Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_THREAD_COUNT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_THREAD_COUNT, Config::RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_THREAD_COUNT, Config::RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_THREAD_COUNT);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, Config::RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_ALL, Config::RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_ALL);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, Config::RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, Config::RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, Config::RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, Config::RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, Config::RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_METHODS, Config::RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_METHODS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, Config::RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, Config::RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_MAX_AGE).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, Config::RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_MAX_AGE, Config::RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_MAX_AGE);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_TLS_CERT_FILE).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_TLS_CERT_FILE, Config::RWS_CONFIG_TLS_CERT_FILE_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_TLS_CERT_FILE, Config::RWS_CONFIG_TLS_CERT_FILE_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_TLS_CERT_FILE);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_TLS_KEY_FILE).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_TLS_KEY_FILE, Config::RWS_CONFIG_TLS_KEY_FILE_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_TLS_KEY_FILE, Config::RWS_CONFIG_TLS_KEY_FILE_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_TLS_KEY_FILE);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_HTTP_REDIRECT_PORT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_HTTP_REDIRECT_PORT, Config::RWS_CONFIG_HTTP_REDIRECT_PORT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_HTTP_REDIRECT_PORT, Config::RWS_CONFIG_HTTP_REDIRECT_PORT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_HTTP_REDIRECT_PORT);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_LOG_FORMAT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_LOG_FORMAT, Config::RWS_CONFIG_LOG_FORMAT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_LOG_FORMAT, Config::RWS_CONFIG_LOG_FORMAT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_LOG_FORMAT);
    }

    println!("  End of initializing default values\n");
}


pub fn get_ip_port_thread_count() -> (String, i32, i32) {
    let mut ip : String = Config::RWS_CONFIG_IP_DEFAULT_VALUE.to_string();
    let mut port: i32 = *Config::RWS_DEFAULT_PORT;
    let mut thread_count: i32 = *Config::RWS_DEFAULT_THREAD_COUNT;

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        ip = boxed_ip.unwrap()
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        let _port = boxed_port.unwrap();
        let boxed_parse = _port.parse::<i32>();
        if boxed_parse.is_ok() {
            port = boxed_parse.unwrap();
        } else {
            eprintln!("unable to parse port value, expected number, got {}, variable: {}",
                      _port, Config::RWS_CONFIG_PORT);
        }
    } else {
        eprintln!("unable to parse port value, variable: {}", Config::RWS_CONFIG_PORT);
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        let _thread_count = boxed_thread_count.unwrap();
        let boxed_parse = _thread_count.parse();
        if boxed_parse.is_ok() {
            thread_count = boxed_parse.unwrap()
        } else {
            eprintln!("unable to parse thread count value, expected number, got {}, variable: {}",
                      thread_count, Config::RWS_CONFIG_THREAD_COUNT);
        }

    } else {
        eprintln!("unable to parse thread count value, variable: {}", Config::RWS_CONFIG_THREAD_COUNT);
    }

    (ip, port, thread_count)
}

/// Read all `[[virtual_host]]` entries from config / env vars.
///
/// Each entry must have `RWS_CONFIG_VIRTUAL_HOST_{N}_DOMAIN` set; reading
/// stops at the first missing index.  `cert_file` and `key_file` default to
/// empty string if omitted.
pub fn get_virtual_hosts() -> Vec<VirtualHostConfig> {
    let mut hosts = Vec::new();
    let mut i = 0usize;
    loop {
        match env::var(format!("RWS_CONFIG_VIRTUAL_HOST_{}_DOMAIN", i)) {
            Err(_) => break,
            Ok(domain) => {
                let cert_file = env::var(format!("RWS_CONFIG_VIRTUAL_HOST_{}_CERT_FILE", i))
                    .unwrap_or_default();
                let key_file = env::var(format!("RWS_CONFIG_VIRTUAL_HOST_{}_KEY_FILE", i))
                    .unwrap_or_default();
                hosts.push(VirtualHostConfig { domain, cert_file, key_file });
                i += 1;
            }
        }
    }
    hosts
}

pub fn get_request_allocation_size() -> i64 {
    let mut request_allocation_size: i64 = *Config::RWS_DEFAULT_REQUEST_ALLOCATION_SIZE_IN_BYTES;

    let boxed_port = env::var(Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES);
    if boxed_port.is_ok() {
        let _request_allocation_size = boxed_port.unwrap();
        let boxed_parse = _request_allocation_size.parse::<i64>();
        if boxed_parse.is_ok() {
            request_allocation_size = boxed_parse.unwrap();
        } else {
            eprintln!("unable to parse port value, expected number, got {}, variable: {}",
                      _request_allocation_size, Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES);
        }
    } else {
        eprintln!("unable to parse request allocation size value, variable: {}", Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES);
    }


    request_allocation_size
}

/// Maximum accepted request body size in bytes. `0` means unlimited (the default —
/// enforcing a cap is opt-in). Read fresh on every call, so `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`
/// takes effect immediately, the same as [`get_request_allocation_size`].
pub fn get_max_body_size() -> u64 {
    env::var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}

/// The configured SPA-fallback file (e.g. `"index.html"`), or `None` if
/// `RWS_CONFIG_SPA_FALLBACK` is unset/empty (the default — disabled). Read
/// fresh on every call, so it can be toggled via `rws.config.toml` + `SIGHUP`
/// without a restart, the same as [`get_max_body_size`].
pub fn get_spa_fallback() -> Option<String> {
    env::var(Config::RWS_CONFIG_SPA_FALLBACK).ok().filter(|v| !v.is_empty())
}

/// Path prefixes that never receive the SPA fallback (`RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`,
/// comma-separated), even when the fallback is configured. Empty (default) excludes nothing.
pub fn get_spa_fallback_exclude_prefixes() -> Vec<String> {
    env::var(Config::RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES)
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

