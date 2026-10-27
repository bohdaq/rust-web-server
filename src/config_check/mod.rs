//! `rws --check`: validate `rws.config.toml` without starting the server —
//! the counterpart of `nginx -t` / `caddy validate`.
//!
//! Reports unknown keys (in settings and in proxy sections), values of the
//! wrong type, TLS files that can't be loaded, proxy routes pointing at
//! undeclared upstreams, auth secrets that aren't set, and similar mistakes
//! that would otherwise surface as a warning in the startup log or as a 503
//! at request time. Nothing is bound and the environment is not modified.
//!
//! ```bash
//! rws --check                      # the file rws would load (RWS_CONFIG_FILE or ./rws.config.toml)
//! rws --check=/etc/rws/site.toml   # a specific file
//! ```

#[cfg(test)]
mod tests;

use std::io::Cursor;
use std::path::Path;

use crate::entry_point::config_file::collect_settings;
use crate::entry_point::Config;

/// Errors and warnings found in a configuration file.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ConfigReport {
    /// Mistakes: the configuration does not do what it says.
    pub errors: Vec<String>,
    /// Likely mistakes that don't stop the server working.
    pub warnings: Vec<String>,
}

impl ConfigReport {
    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// `true` when there are no errors (warnings are allowed).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// One JSON object: `{"path":…,"ok":…,"errors":[…],"warnings":[…]}`.
    /// Strings are escaped; exit codes stay with the caller.
    pub fn render_json(&self, path: &str) -> String {
        fn escaped(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + 2);
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                }
            }
            out
        }
        let list = |items: &[String]| {
            let inner: Vec<String> = items.iter().map(|i| format!("\"{}\"", escaped(i))).collect();
            format!("[{}]", inner.join(","))
        };
        format!(
            "{{\"path\":\"{}\",\"ok\":{},\"errors\":{},\"warnings\":{}}}",
            escaped(path),
            self.is_ok(),
            list(&self.errors),
            list(&self.warnings),
        )
    }

    /// Human-readable report, one finding per line, ending in a verdict.
    pub fn render(&self, path: &str) -> String {
        let mut out = String::new();
        for e in &self.errors {
            out.push_str(&format!("error: {e}\n"));
        }
        for w in &self.warnings {
            out.push_str(&format!("warning: {w}\n"));
        }
        if self.is_ok() {
            out.push_str(&format!("{path}: configuration OK"));
            if !self.warnings.is_empty() {
                out.push_str(&format!(" ({} warning{})", self.warnings.len(), plural(self.warnings.len())));
            }
        } else {
            out.push_str(&format!(
                "{path}: {} error{}, {} warning{}",
                self.errors.len(), plural(self.errors.len()),
                self.warnings.len(), plural(self.warnings.len()),
            ));
        }
        out.push('\n');
        out
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The path `--check` targets in `args` (the process arguments), or `None`
/// when `--check` is absent. `--check=<path>` and `--check <path>` name a
/// file; a bare `--check` checks the file the server would load.
pub fn check_target_from_args(args: &[String]) -> Option<String> {
    let mut iter = args.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        if let Some(path) = arg.strip_prefix("--check=") {
            return Some(path.to_string());
        }
        if arg == "--check" {
            return Some(match iter.peek() {
                Some(next) if !next.starts_with('-') => next.to_string(),
                _ => default_config_path(),
            });
        }
    }
    None
}

/// Whether the arguments ask for machine-readable output (`--format=json`).
pub fn wants_json_output(args: &[String]) -> bool {
    args.iter().any(|a| a == "--format=json")
}

/// `RWS_CONFIG_FILE` if set, else `rws.config.toml` in the working directory.
pub fn default_config_path() -> String {
    std::env::var("RWS_CONFIG_FILE")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "rws.config.toml".to_string())
}

/// Check the configuration file at `path`.
pub fn check_file(path: &str) -> ConfigReport {
    match std::fs::read_to_string(path) {
        Ok(content) => check_str(&content),
        Err(e) => {
            let mut report = ConfigReport::default();
            report.error(format!("cannot read {path}: {e}"));
            report
        }
    }
}

/// Check configuration text. Relative file paths in it (certificates,
/// htpasswd files, static roots) are resolved against the working directory,
/// as the server resolves them.
pub fn check_str(content: &str) -> ConfigReport {
    let mut report = ConfigReport::default();
    if content.contains('\0') {
        report.error("the file contains a NUL byte");
        return report;
    }

    match collect_settings(Cursor::new(content.as_bytes()), String::new()) {
        Ok(resolved) => {
            for key in &resolved.unknown {
                report.error(format!("unknown setting '{key}' — ignored"));
            }
            check_settings(&resolved.settings, &mut report);
        }
        Err(e) => report.error(format!("unable to parse: {e}")),
    }

    #[cfg(not(target_arch = "wasm32"))]
    if content.contains("[[route]]") || content.contains("[[upstream]]")
        || content.contains("[[tcp_proxy]]") || content.contains("[[udp_proxy]]") || content.contains("[[ws_proxy]]")
    {
        crate::proxy_config::validate::check_unknown_keys(content, &mut report);
        crate::proxy_config::ProxyConfig::from_str(content).validate(&mut report);
    } else if content.lines().any(|l| l.trim().starts_with("[middleware")) {
        report.warn("[middleware] only applies in proxy mode (a file with [[route]] or [[upstream]] sections)");
    }

    report
}

/// Type and consistency checks for the `RWS_CONFIG_*` settings in the file.
fn check_settings(settings: &[(String, String)], report: &mut ConfigReport) {
    let get = |name: &str| settings.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.trim());

    for (name, value) in settings {
        let value = value.trim();
        let key = setting_key(name);
        match name.as_str() {
            Config::RWS_CONFIG_PORT | Config::RWS_CONFIG_HTTP_REDIRECT_PORT | Config::RWS_CONFIG_ACME_CHALLENGE_PORT => {
                if !(name == Config::RWS_CONFIG_HTTP_REDIRECT_PORT && value.is_empty())
                    && !value.parse::<u16>().is_ok_and(|p| p > 0)
                {
                    report.error(format!("{key} = {value:?}: not a port number (1-65535)"));
                }
            }
            Config::RWS_CONFIG_IP => {
                if value.parse::<std::net::IpAddr>().is_err() {
                    report.error(format!("{key} = {value:?}: not an IP address (use 0.0.0.0 or :: for all interfaces)"));
                }
            }
            Config::RWS_CONFIG_THREAD_COUNT => {
                if !value.parse::<usize>().is_ok_and(|n| n > 0) {
                    report.error(format!("{key} = {value:?}: not a positive number"));
                }
            }
            Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES => {
                if !value.parse::<u64>().is_ok_and(|n| n > 0) {
                    report.error(format!("{key} = {value:?}: not a positive number"));
                }
            }
            "RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES" | "RWS_CONFIG_OPEN_FILE_CACHE_SECS" | "RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS" | "RWS_CONFIG_RATE_LIMIT_WINDOW_SECS"
            | "RWS_CONFIG_ACME_RENEW_BEFORE_DAYS" | "RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS"
            | "RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD" | "RWS_CONFIG_CIRCUIT_BREAKER_MAX_HALF_OPEN_PROBES"
            | "RWS_CONFIG_CIRCUIT_BREAKER_RECOVERY_SECS" | Config::RWS_CONFIG_CORS_MAX_AGE => {
                if value.parse::<u64>().is_err() {
                    report.error(format!("{key} = {value:?}: not a whole number"));
                }
            }
            Config::RWS_CONFIG_CORS_ALLOW_ALL | Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS | Config::RWS_CONFIG_ACME_STAGING
            | "RWS_CONFIG_CLIENT_HINTS" | "RWS_CONFIG_OPEN_FILE_CACHE_STRICT"
            | "RWS_CONFIG_SECURITY_HEADERS" | "RWS_CONFIG_NANOS_HEADERS" | "RWS_CONFIG_SERVE_DOTFILES" | "RWS_CONFIG_WATCH"
            | "RWS_CONFIG_PROXY_PROTOCOL" => {
                if !matches!(value.to_ascii_lowercase().as_str(), "true" | "false") {
                    report.error(format!("{key} = {value:?}: expected true or false"));
                }
            }
            "RWS_CONFIG_ACCESS_LOG_MAX_SIZE" | "RWS_CONFIG_ACCESS_LOG_KEEP" => {
                if crate::log::access_sink::parse_size(value).is_none() {
                    report.error(format!("{key} = {value:?}: expected a number of bytes or files (e.g. 104857600 or 100M)"));
                }
            }
            "RWS_CONFIG_RUNTIME" => {
                if !matches!(value.to_ascii_lowercase().as_str(), "shared" | "per-core") {
                    report.error(format!("{key} = {value:?}: expected \"shared\" or \"per-core\""));
                }
            }
            "RWS_CONFIG_LOG_FORMAT" => {
                if !matches!(value.to_ascii_lowercase().as_str(), "combined" | "json" | "off" | "none") {
                    report.error(format!("{key} = {value:?}: expected \"combined\", \"json\" or \"off\""));
                }
            }
            "RWS_CONFIG_TLS_CLIENT_CA_FILE" | Config::RWS_CONFIG_ACME_CA_FILE => {
                if !value.is_empty() && !Path::new(value).is_file() {
                    report.error(format!("{key} = {value:?}: file not found"));
                }
            }
            "RWS_CONFIG_TEMPLATE_DIR" => {
                if !value.is_empty() && !Path::new(value).is_dir() {
                    report.warn(format!("{key} = {value:?}: directory not found"));
                }
            }
            _ => {}
        }
    }

    // TLS: both files or neither; loadable unless ACME will create them.
    let acme_domains = get(Config::RWS_CONFIG_ACME_DOMAINS).unwrap_or("");
    let acme = !acme_domains.is_empty();
    let cert = get(Config::RWS_CONFIG_TLS_CERT_FILE).unwrap_or("");
    let key = get(Config::RWS_CONFIG_TLS_KEY_FILE).unwrap_or("");
    match (cert.is_empty(), key.is_empty()) {
        (false, true) => report.error("tls_cert_file is set but tls_key_file is not"),
        (true, false) => report.error("tls_key_file is set but tls_cert_file is not"),
        (false, false) if !acme => check_tls_pair("tls_cert_file / tls_key_file", cert, key, report),
        _ => {}
    }
    // spa_fallback names a file relative to the working directory; if it is
    // missing, every client-side route except / 404s.
    if let Some(fallback) = get(Config::RWS_CONFIG_SPA_FALLBACK).filter(|v| !v.is_empty()) {
        if !std::path::Path::new(fallback).is_file() {
            report.warn(format!(
                "spa_fallback = {fallback:?}: the file does not exist in the working directory, so every client-side route would 404"
            ));
        }
    }

    // [cors] allow_origins must match what the browser sends in Origin:
    // scheme://host[:port], no trailing slash, no path.
    if let Some(origins) = get(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS) {
        for origin in origins.split(',').map(str::trim).filter(|o| !o.is_empty()) {
            if origin != "*" && !origin.starts_with("http://") && !origin.starts_with("https://") {
                report.warn(format!(
                    "[cors] allow_origins: '{origin}' does not look like scheme://host[:port]; a browser's Origin header never matches it"
                ));
            } else if origin != "*" && (origin.ends_with('/') || origin.rsplit_once("://").is_some_and(|(_, rest)| rest.contains('/'))) {
                report.warn(format!(
                    "[cors] allow_origins: '{origin}' has a path or trailing slash; Origin headers carry none, so it never matches"
                ));
            }
        }
    }

    let mut seen_domains: Vec<&str> = Vec::new();
    for i in 0.. {
        let domain = get(&format!("RWS_CONFIG_VIRTUAL_HOST_{i}_DOMAIN"));
        let cert = get(&format!("RWS_CONFIG_VIRTUAL_HOST_{i}_CERT_FILE"));
        let key = get(&format!("RWS_CONFIG_VIRTUAL_HOST_{i}_KEY_FILE"));
        if domain.is_none() && cert.is_none() && key.is_none() {
            break;
        }
        let label = format!("[[virtual_host]] {}", domain.unwrap_or("(no domain)"));
        match (domain, cert, key) {
            (Some(d), Some(c), Some(k)) if !d.is_empty() => {
                // Two entries with the same domain: only one certificate can
                // win, so the loser is a silent misconfiguration.
                if seen_domains.contains(&d) {
                    report.error(format!(
                        "{label}: a second [[virtual_host]] with the same domain; only one certificate can win"
                    ));
                } else {
                    seen_domains.push(d);
                }
                check_tls_pair(&label, c, k, report)
            }
            _ => report.error(format!("{label}: needs domain, cert_file and key_file")),
        }
    }

    if acme {
        #[cfg(not(feature = "acme"))]
        report.error("[acme] is configured, but this rws binary was built without the `acme` feature — no certificate would be requested");
        if get(Config::RWS_CONFIG_ACME_EMAIL).unwrap_or("").is_empty() {
            report.warn("[acme] has no email: the CA can't warn you about expiring or revoked certificates");
        }
        let challenge = get(Config::RWS_CONFIG_ACME_CHALLENGE).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("http-01").to_ascii_lowercase();
        if !matches!(challenge.as_str(), "http-01" | "tls-alpn-01" | "dns-01") {
            report.error(format!("[acme] challenge = {challenge:?}: expected http-01, tls-alpn-01 or dns-01"));
        }
        for domain in acme_domains.split(',').map(str::trim) {
            // A wildcard is one leading "*." label (RFC 8555 §7.1.3).
            let (wildcard, name) = match domain.strip_prefix("*.") {
                Some(rest) => (true, rest),
                None => (false, domain),
            };
            if name.is_empty() || name.contains('*') || name.contains('/') || name.contains(':') || !name.contains('.') {
                report.error(format!("[acme] domains: '{domain}' is not a domain name"));
            } else if wildcard && challenge != "dns-01" {
                report.error(format!("[acme] domains: the wildcard '{domain}' needs challenge = \"dns-01\""));
            }
        }
        let port = get(Config::RWS_CONFIG_PORT).unwrap_or("7878");
        let challenge_port = get(Config::RWS_CONFIG_ACME_CHALLENGE_PORT).unwrap_or("80");
        let redirect_port = get(Config::RWS_CONFIG_HTTP_REDIRECT_PORT).unwrap_or("");
        if challenge == "http-01" && redirect_port.is_empty() {
            report.warn(format!(
                "[acme] without http_redirect_port: nothing answers the HTTP-01 challenge on port {challenge_port}; set http_redirect_port = {challenge_port}"
            ));
        }
        if challenge == "dns-01" {
            check_acme_dns(&get, report);
        }
        let on_demand = get(Config::RWS_CONFIG_ACME_ON_DEMAND).unwrap_or("").trim().to_ascii_lowercase();
        if !matches!(on_demand.as_str(), "" | "true" | "false") {
            report.error(format!("[acme] on_demand = {on_demand:?}: expected true or false"));
        } else if on_demand == "true" {
            match get(Config::RWS_CONFIG_ACME_ON_DEMAND_ASK).map(str::trim).filter(|v| !v.is_empty()) {
                None => report.error("[acme] on_demand needs on_demand_ask: without it anyone could make rws order certificates for any name pointed at it"),
                Some(ask) if !(ask.starts_with("http://") || ask.starts_with("https://")) => {
                    report.error(format!("[acme] on_demand_ask = {ask:?}: expected an http:// or https:// URL"))
                }
                Some(_) => {}
            }
            if challenge == "dns-01" {
                report.error("[acme] on_demand works with http-01 and tls-alpn-01, not dns-01 (it would need every customer's DNS zone)");
            }
        }
        if port != "443" {
            report.warn(format!("[acme] with port = {port}: browsers expect HTTPS on 443 (fine if a port mapping forwards 443 here)"));
        }
    }
}

/// `[acme] challenge = "dns-01"`: a provider and what it needs.
fn check_acme_dns<'a>(get: &dyn Fn(&str) -> Option<&'a str>, report: &mut ConfigReport) {
    let set = |name: &str| get(name).map(str::trim).filter(|v| !v.is_empty());
    match set(Config::RWS_CONFIG_ACME_DNS_PROVIDER).map(str::to_ascii_lowercase).as_deref() {
        Some("rfc2136") => {
            match set(Config::RWS_CONFIG_ACME_DNS_SERVER) {
                None => report.error("[acme] dns_provider = \"rfc2136\" needs dns_server (the primary DNS server, host[:port])"),
                Some(server) if server.contains('/') || server.is_empty() => report.error(format!("[acme] dns_server = {server:?}: expected host[:port]")),
                Some(_) => {}
            }
            match (set(Config::RWS_CONFIG_ACME_DNS_TSIG_KEY), set(Config::RWS_CONFIG_ACME_DNS_TSIG_SECRET)) {
                (Some(_), None) | (None, Some(_)) => report.error("[acme] dns_tsig_key and dns_tsig_secret go together"),
                (None, None) => report.warn("[acme] rfc2136 without a TSIG key: the server must accept unsigned updates from this host"),
                (Some(_), Some(secret)) => {
                    #[cfg(feature = "acme")]
                    if crate::acme::base64_secret(secret).is_none() {
                        report.error("[acme] dns_tsig_secret is not base64");
                    }
                    #[cfg(not(feature = "acme"))]
                    let _ = secret;
                }
            }
            if let Some(algorithm) = set(Config::RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM) {
                if !matches!(algorithm.to_ascii_lowercase().as_str(), "hmac-sha256" | "hmac-sha512") {
                    report.error(format!("[acme] dns_tsig_algorithm = {algorithm:?}: expected hmac-sha256 or hmac-sha512"));
                }
            }
        }
        Some("exec") => match set(Config::RWS_CONFIG_ACME_DNS_EXEC) {
            None => report.error("[acme] dns_provider = \"exec\" needs dns_exec (the program)"),
            Some(program) if !Path::new(program).is_file() => report.error(format!("[acme] dns_exec = {program:?}: file not found")),
            Some(_) => {}
        },
        Some(other) => report.error(format!("[acme] dns_provider = {other:?}: expected rfc2136 or exec")),
        None => report.error("[acme] challenge = \"dns-01\" needs dns_provider (rfc2136 or exec)"),
    }
    if let Some(secs) = set(Config::RWS_CONFIG_ACME_DNS_PROPAGATION_SECS) {
        if secs.parse::<u64>().is_err() {
            report.error(format!("[acme] dns_propagation_secs = {secs:?}: not a whole number"));
        }
    }
}

/// `RWS_CONFIG_CORS_ALLOW_ALL` → `cors_allow_all`, for messages.
fn setting_key(env_name: &str) -> String {
    env_name.trim_start_matches("RWS_CONFIG_").to_ascii_lowercase()
}

fn check_tls_pair(label: &str, cert: &str, key: &str, report: &mut ConfigReport) {
    let mut missing = false;
    for (what, path) in [("certificate", cert), ("key", key)] {
        if !Path::new(path).is_file() {
            report.error(format!("{label}: {what} file '{path}' not found"));
            missing = true;
        }
    }
    if missing {
        return;
    }
    #[cfg(all(feature = "http2", not(target_arch = "wasm32")))]
    if let Err(e) = crate::tls::create_tls_acceptor(cert, key) {
        report.error(format!("{label}: {e}"));
    }
    #[cfg(not(feature = "http2"))]
    report.warn(format!("{label}: this build has no TLS support (`http1` only), so the certificate is not used"));
}
