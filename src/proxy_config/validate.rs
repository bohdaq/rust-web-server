//! Static checks for the proxy sections of `rws.config.toml`, used by
//! `rws --check` and printed at startup. The parser itself is lenient — a
//! typo'd key just leaves a default in place — so this is where a route that
//! points at a missing upstream, an auth secret that isn't set, or a key the
//! parser never reads gets reported.

use std::collections::HashSet;

use crate::config_check::ConfigReport;
use crate::proxy_config::{parser, ActionConfig, AuthConfig, MiddlewareConfig, ProxyConfig};

const STRATEGIES: &[&str] = &["round_robin", "random", "ip_hash", "least_connections"];

const HEALTH_CHECK_KEYS: &[&str] = &["path", "interval_secs", "timeout_ms", "healthy_threshold", "unhealthy_threshold"];
const PROXY_ACTION_KEYS: &[&str] = &["upstream", "connect_timeout_ms", "read_timeout_ms", "strip_path_prefix", "add_path_prefix"];
const GRPC_ACTION_KEYS: &[&str] = &["upstream", "connect_timeout_ms", "read_timeout_ms"];
const STATIC_ACTION_KEYS: &[&str] = &["root", "index"];
const REDIRECT_ACTION_KEYS: &[&str] = &["location", "status"];
const RESPOND_ACTION_KEYS: &[&str] = &["status", "body", "content_type"];
const REWRITE_RULE_KEYS: &[&str] = &["type", "name", "value", "prefix", "from", "to", "code", "reason"];

/// The keys the parser reads in a section, by the section's path with array
/// indices removed (`route[2].middleware.auth` → `route.middleware.auth`).
/// `None` for a section the parser never reads at all.
fn known_keys(section: &str) -> Option<Vec<&'static str>> {
    // The global `[middleware]` table has the same shape as `[route.middleware]`.
    let route_form;
    let section = if section == "middleware" || section.starts_with("middleware.") {
        route_form = format!("route.{section}");
        route_form.as_str()
    } else {
        section
    };
    let keys: Vec<&'static str> = match section {
        "upstream" => vec!["name", "backends", "strategy", "tls_ca_file", "tls_client_cert_file", "tls_client_key_file"],
        "upstream.health_check" | "ws_proxy.health_check" => HEALTH_CHECK_KEYS.to_vec(),
        "route" => vec!["name"],
        "route.match" => vec!["host", "path", "method", "content_type"],
        // `[route.action]` takes `type` plus the short form of every action's keys.
        "route.action" => {
            let mut keys = vec!["type"];
            for group in [PROXY_ACTION_KEYS, GRPC_ACTION_KEYS, STATIC_ACTION_KEYS, REDIRECT_ACTION_KEYS, RESPOND_ACTION_KEYS] {
                for key in group {
                    if !keys.contains(key) {
                        keys.push(key);
                    }
                }
            }
            keys
        }
        "route.action.proxy" => PROXY_ACTION_KEYS.to_vec(),
        "route.action.grpc" => GRPC_ACTION_KEYS.to_vec(),
        "route.action.static" => STATIC_ACTION_KEYS.to_vec(),
        "route.action.redirect" => REDIRECT_ACTION_KEYS.to_vec(),
        "route.action.respond" => RESPOND_ACTION_KEYS.to_vec(),
        "route.middleware" => vec!["timeout_ms", "max_body_size"],
        "route.middleware.rate_limit" => vec!["max_requests", "window_secs"],
        "route.middleware.cache" => vec!["ttl_secs", "vary_by"],
        "route.middleware.auth" => vec!["type", "token_env", "secret_env", "htpasswd_file"],
        "route.middleware.ip_filter" => vec!["allow", "deny"],
        "route.middleware.rewrite.request" | "route.middleware.rewrite.response" => REWRITE_RULE_KEYS.to_vec(),
        "tcp_proxy" => vec!["name", "listen", "backends", "connect_timeout_ms"],
        "udp_proxy" => vec!["name", "listen", "backends", "reply_timeout_ms", "buffer_size"],
        "ws_proxy" => vec!["name", "listen", "backends", "connect_timeout_ms", "read_timeout_ms"],
        _ => return None,
    };
    Some(keys)
}

/// `route[2].middleware.rewrite.request[0]` → `route.middleware.rewrite.request`.
fn strip_indices(section: &str) -> String {
    let mut out = String::with_capacity(section.len());
    let mut in_index = false;
    for c in section.chars() {
        match c {
            '[' => in_index = true,
            ']' => in_index = false,
            _ if !in_index => out.push(c),
            _ => {}
        }
    }
    out
}

/// Report keys and tables in the proxy sections that the parser ignores.
pub fn check_unknown_keys(toml: &str, report: &mut ConfigReport) {
    let map = parser::parse(toml);
    let mut sections: Vec<&String> = map.keys().collect();
    sections.sort();
    for section in sections {
        let root = section.split(['.', '[']).next().unwrap_or("");
        if !["route", "upstream", "tcp_proxy", "udp_proxy", "ws_proxy", "middleware"].contains(&root) {
            continue;
        }
        let shape = strip_indices(section);
        match known_keys(&shape) {
            None => report.error(format!("[{}]: unknown table — ignored", shape)),
            Some(keys) => {
                for (key, _) in &map[section] {
                    if !keys.contains(&key.as_str()) {
                        report.error(format!("[{}] {}: unknown key — ignored", shape, key));
                    }
                }
            }
        }
    }
}

/// `host:port`, or a URL with an `http://`, `https://`, `h2://`, `ws://` or
/// `wss://` scheme, whose port may be left to the scheme's default.
fn is_host_port(backend: &str) -> bool {
    let (rest, has_scheme) = match ["http://", "https://", "h2://", "h2s://", "grpc://", "grpcs://", "ws://", "wss://"]
        .iter()
        .find_map(|scheme| backend.strip_prefix(scheme))
    {
        Some(rest) => (rest, true),
        None => (backend, false),
    };
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() || rest.contains('/') || rest.contains(char::is_whitespace) {
        return false;
    }
    match rest.rsplit_once(':') {
        Some((host, port)) => !host.is_empty() && port.parse::<u16>().is_ok_and(|p| p > 0),
        None => has_scheme,
    }
}

impl ProxyConfig {
    /// Check the configuration for mistakes the parser lets through. Reads
    /// the auth secrets' environment variables and htpasswd files, so run it
    /// in the environment the server will run in.
    pub fn validate(&self, report: &mut ConfigReport) {
        let mut names = HashSet::new();
        for (i, upstream) in self.upstreams.iter().enumerate() {
            let label = if upstream.name.is_empty() { format!("[[upstream]] #{}", i + 1) } else { format!("upstream '{}'", upstream.name) };
            if upstream.name.is_empty() {
                report.error(format!("{label}: missing `name` — no route can refer to it"));
            } else if !names.insert(upstream.name.as_str()) {
                report.error(format!("{label}: declared more than once"));
            }
            if upstream.backends.is_empty() {
                report.error(format!("{label}: `backends` is empty"));
            }
            for backend in &upstream.backends {
                if !is_host_port(backend) {
                    report.error(format!("{label}: backend '{backend}' is not host:port"));
                }
            }
            let client = match (&upstream.tls_client_cert_file, &upstream.tls_client_key_file) {
                (Some(cert), Some(key)) => Some((cert.as_str(), key.as_str())),
                (None, None) => None,
                _ => {
                    report.error(format!("{label}: tls_client_cert_file and tls_client_key_file go together"));
                    None
                }
            };
            if upstream.tls_ca_file.is_some() || client.is_some() {
                if let Err(e) = crate::proxy::UpstreamTls::new(upstream.tls_ca_file.as_deref(), client) {
                    report.error(format!("{label}: {e}; its TLS backends would answer 502"));
                }
                if !upstream.tls {
                    report.warn(format!("{label}: TLS settings are set but no backend is https://, h2s:// or grpcs://, so they aren't used"));
                }
            }
            if !STRATEGIES.contains(&upstream.strategy.as_str()) {
                report.error(format!(
                    "{label}: unknown strategy '{}' (expected one of {})",
                    upstream.strategy,
                    STRATEGIES.join(", ")
                ));
            }
            #[cfg(not(any(feature = "http-client", feature = "http2")))]
            if upstream.tls {
                report.error(format!("{label}: https:// backends need the `http2` or `http-client` feature, which this build doesn't include"));
            }
        }

        for (i, route) in self.routes.iter().enumerate() {
            let label = if route.name.is_empty() { format!("route #{}", i + 1) } else { format!("route '{}'", route.name) };
            match &route.action {
                ActionConfig::Proxy { upstream, .. } | ActionConfig::Grpc { upstream, .. } => {
                    if upstream.is_empty() {
                        report.error(format!("{label}: proxy action has no `upstream`"));
                    } else if !names.contains(upstream.as_str()) {
                        if is_host_port(upstream) {
                            report.warn(format!("{label}: upstream '{upstream}' is not a declared [[upstream]]; using it as a backend address (no health checks)"));
                        } else {
                            report.error(format!("{label}: upstream '{upstream}' is not a declared [[upstream]]"));
                        }
                    }
                }
                ActionConfig::Static { root, .. } => {
                    if root.is_empty() {
                        report.error(format!("{label}: static action has no `root`"));
                    } else if !std::path::Path::new(root).is_dir() {
                        report.warn(format!("{label}: static root '{root}' is not a directory (yet)"));
                    }
                }
                ActionConfig::Redirect { location, status } => {
                    if location.is_empty() {
                        report.error(format!("{label}: redirect action has no `location`"));
                    }
                    if !(300..=399).contains(status) {
                        report.error(format!("{label}: redirect status {status} is not a 3xx code"));
                    }
                }
                ActionConfig::Respond { status, .. } => {
                    if !(100..=599).contains(status) {
                        report.error(format!("{label}: respond status {status} is not an HTTP status code"));
                    }
                }
                ActionConfig::Mcp => {
                    report.warn(format!("{label}: `mcp` actions are not served from the config file; requests fall through to the static files"));
                }
                ActionConfig::Unknown(kind) if kind.is_empty() => {
                    report.error(format!("{label}: [route.action] has no `type`"));
                }
                ActionConfig::Unknown(kind) => {
                    report.error(format!("{label}: unknown action type '{kind}' (expected proxy, grpc, static, redirect or respond)"));
                }
            }
            validate_middleware(&label, &route.middleware, report);

            let m = &route.match_;
            if m.host.is_none() && m.path.is_none() && m.method.is_none() && m.content_type.is_none() && i + 1 < self.routes.len() {
                report.warn(format!("{label}: matches every request, so the routes after it are never reached"));
            }
        }

        validate_middleware("[middleware]", &self.global_middleware, report);

        let listeners = self.tcp_proxies.iter().map(|p| ("tcp_proxy", &p.name, &p.listen, &p.backends))
            .chain(self.udp_proxies.iter().map(|p| ("udp_proxy", &p.name, &p.listen, &p.backends)))
            .chain(self.ws_proxies.iter().map(|p| ("ws_proxy", &p.name, &p.listen, &p.backends)));
        for (kind, name, listen, backends) in listeners {
            let label = format!("[[{kind}]] '{name}'");
            if listen.parse::<std::net::SocketAddr>().is_err() {
                report.error(format!("{label}: listen '{listen}' is not an address like 0.0.0.0:5432"));
            }
            if backends.is_empty() {
                report.error(format!("{label}: `backends` is empty"));
            }
            for backend in backends {
                if !is_host_port(backend) {
                    report.error(format!("{label}: backend '{backend}' is not host:port"));
                }
            }
        }
    }
}

fn validate_middleware(label: &str, mw: &MiddlewareConfig, report: &mut ConfigReport) {
    match &mw.auth {
        Some(AuthConfig::Bearer { token_env }) | Some(AuthConfig::Jwt { secret_env: token_env }) => {
            if token_env.is_empty() {
                report.error(format!("{label}: auth has no `token_env` / `secret_env`; the route answers 503"));
            } else if std::env::var(token_env).unwrap_or_default().is_empty() {
                report.error(format!("{label}: auth secret env var '{token_env}' is unset or empty; the route answers 503 until it is set"));
            }
        }
        Some(AuthConfig::Basic { htpasswd_file }) => {
            #[cfg(feature = "auth")]
            if let Err(e) = crate::auth::BasicAuthLayer::from_htpasswd_file(htpasswd_file) {
                report.error(format!("{label}: basic auth: {e}; the route answers 503"));
            }
            #[cfg(not(feature = "auth"))]
            let _ = htpasswd_file;
        }
        None => {}
    }
    #[cfg(not(feature = "auth"))]
    if matches!(mw.auth, Some(AuthConfig::Jwt { .. }) | Some(AuthConfig::Basic { .. })) {
        report.error(format!("{label}: jwt and basic auth need the `auth` feature, which this build doesn't include; the route answers 503"));
    }

    for entry in mw.ip_allow.iter().chain(&mw.ip_deny) {
        if !crate::ip_filter::is_valid_entry(entry) {
            report.error(format!("{label}: ip_filter entry '{entry}' is not an IP address or CIDR range — ignored"));
        }
    }
    if !mw.ip_allow.is_empty() && !mw.ip_deny.is_empty() {
        report.warn(format!("{label}: ip_filter has both `allow` and `deny`; only `allow` is used"));
    }
    if let Some(rl) = &mw.rate_limit {
        if rl.max_requests == 0 || rl.window_secs == 0 {
            report.error(format!("{label}: rate_limit needs max_requests and window_secs above 0"));
        }
    }
    for rule in mw.rewrite_request.iter().chain(&mw.rewrite_response) {
        let known = ["header_set", "header_remove", "uri_set", "uri_strip_prefix", "strip_prefix",
                     "uri_add_prefix", "add_prefix", "status", "body_replace"];
        if !known.contains(&rule.type_.as_str()) {
            report.error(format!("{label}: unknown rewrite rule type '{}' — ignored", rule.type_));
        }
    }
}
