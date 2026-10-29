//! `[route.action] type = "mcp"`: the [MCP gateway](crate::mcp::gateway)
//! from the config file (18.9.0).
//!
//! ```toml
//! [[route]]
//!   [route.match]
//!   path = "/mcp"
//!
//!   [route.action]
//!   type = "mcp"
//!
//!   [route.action.mcp]
//!   name        = "gateway"     # announced to clients
//!   refresh_secs = 300          # re-read upstream tools; 0 = only at start
//!   audit_log   = true
//!   allowed_origins = ["https://app.example.com"]
//!
//!   [[route.action.mcp.upstream]]
//!   prefix      = "github"
//!   url         = "https://mcp.github.example/mcp"
//!   timeout_ms  = 30000
//!   headers_env = { Authorization = "GITHUB_MCP_AUTHORIZATION" }  # header = env var holding its value
//!
//!   [[route.action.mcp.caller]]
//!   identity  = "alice"
//!   token_env = "ALICE_MCP_TOKEN"       # the bearer token alice sends
//!
//!   [[route.action.mcp.allow]]
//!   identity = "alice"
//!   tools    = ["github__*"]
//!
//!   [[route.action.mcp.limit]]
//!   tools       = "github__create_issue"
//!   max_calls   = 5
//!   window_secs = 3600
//! ```
//!
//! Secrets never sit in the file: caller tokens and upstream header values
//! are read from the environment variables named. The gateway is built in
//! the background (it lists every upstream's tools); until then, and while
//! an upstream can't be reached at startup, requests answer 503 and the
//! build is retried every 10 s.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::application::Application;
use crate::config_check::ConfigReport;
use crate::mcp::gateway::{McpGateway, Upstream};
use crate::mcp::McpServer;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

use super::parser::{get_array, get_str, get_u32, get_u64, section_exists, SectionMap};

/// How long a failed build waits before the next attempt.
const RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpGatewayConfig {
    pub name: String,
    pub refresh_secs: u64,
    pub audit_log: bool,
    pub allowed_origins: Vec<String>,
    pub upstreams: Vec<UpstreamConfig>,
    pub callers: Vec<CallerConfig>,
    pub allow: Vec<(String, Vec<String>)>,
    pub limits: Vec<LimitConfig>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpstreamConfig {
    pub prefix: String,
    pub url: String,
    pub timeout_ms: u64,
    /// `(header, environment variable holding its value)`.
    pub headers_env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallerConfig {
    pub identity: String,
    pub token_env: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LimitConfig {
    pub tools: String,
    pub max_calls: u32,
    pub window_secs: u64,
}

/// The keys `check_unknown_keys` accepts in each part.
pub(crate) const GATEWAY_KEYS: &[&str] = &["name", "refresh_secs", "audit_log", "allowed_origins"];
pub(crate) const UPSTREAM_KEYS: &[&str] = &["prefix", "url", "timeout_ms"];
pub(crate) const CALLER_KEYS: &[&str] = &["identity", "token_env"];
pub(crate) const ALLOW_KEYS: &[&str] = &["identity", "tools"];
pub(crate) const LIMIT_KEYS: &[&str] = &["tools", "max_calls", "window_secs"];

/// `section` is the action's, e.g. `route[0].action`.
pub(crate) fn parse(map: &SectionMap, section: &str) -> McpGatewayConfig {
    let base = format!("{section}.mcp");
    let indexed = |kind: &str| -> Vec<String> {
        (0..).map(|i| format!("{base}.{kind}[{i}]")).take_while(|s| section_exists(map, s)).collect()
    };
    let name = get_str(map, &base, "name");
    McpGatewayConfig {
        name: if name.is_empty() { "rws-mcp-gateway".to_string() } else { name },
        refresh_secs: get_u64(map, &base, "refresh_secs", crate::mcp::gateway::DEFAULT_REFRESH.as_secs()),
        audit_log: get_str(map, &base, "audit_log").eq_ignore_ascii_case("true"),
        allowed_origins: get_array(map, &base, "allowed_origins"),
        upstreams: indexed("upstream")
            .into_iter()
            .map(|s| UpstreamConfig {
                prefix: get_str(map, &s, "prefix"),
                url: get_str(map, &s, "url"),
                timeout_ms: get_u64(map, &s, "timeout_ms", 30_000),
                headers_env: map.get(&format!("{s}.headers_env")).cloned().unwrap_or_default(),
            })
            .collect(),
        callers: indexed("caller")
            .into_iter()
            .map(|s| CallerConfig { identity: get_str(map, &s, "identity"), token_env: get_str(map, &s, "token_env") })
            .collect(),
        allow: indexed("allow").into_iter().map(|s| (get_str(map, &s, "identity"), get_array(map, &s, "tools"))).collect(),
        limits: indexed("limit")
            .into_iter()
            .map(|s| LimitConfig { tools: get_str(map, &s, "tools"), max_calls: get_u32(map, &s, "max_calls", 0), window_secs: get_u64(map, &s, "window_secs", 0) })
            .collect(),
    }
}

/// Semantic checks for `rws --check` and startup.
pub(crate) fn validate(config: &McpGatewayConfig, route_path: Option<&str>, label: &str, report: &mut ConfigReport) {
    let offline = crate::config_check::offline();
    match route_path {
        Some(path) if path.ends_with('*') => report.error(format!("{label}: an mcp route needs an exact [route.match] path, e.g. \"/mcp\" (not {path:?})")),
        None => report.error(format!("{label}: an mcp route needs [route.match] path, e.g. \"/mcp\"")),
        _ => {}
    }
    if config.upstreams.is_empty() {
        report.error(format!("{label}: [route.action.mcp] has no [[route.action.mcp.upstream]]"));
    }
    let mut prefixes = Vec::new();
    for (i, u) in config.upstreams.iter().enumerate() {
        let what = format!("{label}: upstream #{}", i + 1);
        if u.prefix.is_empty() || u.prefix.contains("__") || u.prefix.ends_with('_') || !u.prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            report.error(format!("{what}: prefix {:?} must be letters, digits, '-' and single '_'", u.prefix));
        } else if prefixes.contains(&u.prefix) {
            report.error(format!("{what}: prefix {:?} is used twice", u.prefix));
        } else {
            prefixes.push(u.prefix.clone());
        }
        if !(u.url.starts_with("http://") || u.url.starts_with("https://")) {
            report.error(format!("{what}: url {:?} must start with http:// or https://", u.url));
        }
        for (header, env) in &u.headers_env {
            if !offline && std::env::var(env).unwrap_or_default().is_empty() {
                report.error(format!("{what}: header {header} reads {env}, which is unset or empty"));
            }
        }
    }
    if config.callers.is_empty() {
        report.error(format!("{label}: no [[route.action.mcp.caller]]: nobody can call the gateway"));
    }
    for c in &config.callers {
        if c.identity.is_empty() || c.token_env.is_empty() {
            report.error(format!("{label}: a caller needs identity and token_env"));
        } else if !offline && std::env::var(&c.token_env).unwrap_or_default().trim().is_empty() {
            report.error(format!("{label}: caller {} reads its token from {}, which is unset or empty", c.identity, c.token_env));
        }
    }
    if config.allow.is_empty() {
        report.warn(format!("{label}: no [[route.action.mcp.allow]] rule: callers see no tools"));
    }
    for (identity, tools) in &config.allow {
        if identity.is_empty() || tools.is_empty() {
            report.error(format!("{label}: an allow rule needs identity and tools"));
        }
    }
    for l in &config.limits {
        if l.tools.is_empty() || l.max_calls == 0 || l.window_secs == 0 {
            report.error(format!("{label}: a limit needs tools, max_calls and window_secs above 0"));
        }
    }
}

/// The gateway from `config` (lists every upstream's tools: network).
fn build(config: &McpGatewayConfig, path: &str) -> Result<McpServer, String> {
    let mut gateway = McpGateway::new(config.name.clone(), env!("CARGO_PKG_VERSION"))
        .refresh_every((config.refresh_secs > 0).then(|| Duration::from_secs(config.refresh_secs)))
        .audit_log(config.audit_log);
    for u in &config.upstreams {
        let mut upstream = Upstream::new(u.prefix.clone(), u.url.clone()).timeout_ms(u.timeout_ms);
        for (header, env) in &u.headers_env {
            if let Ok(value) = std::env::var(env) {
                upstream = upstream.header(header.clone(), value);
            }
        }
        gateway = gateway.upstream(upstream);
    }
    for (identity, tools) in &config.allow {
        gateway = gateway.allow(identity.clone(), tools.clone());
    }
    for l in &config.limits {
        gateway = gateway.limit(l.tools.clone(), l.max_calls, l.window_secs);
    }
    let tokens: Vec<(String, String)> = config
        .callers
        .iter()
        .filter_map(|c| std::env::var(&c.token_env).ok().filter(|t| !t.trim().is_empty()).map(|t| (t, c.identity.clone())))
        .collect();
    let mut server = gateway.build()?.at(path).require_bearer_tokens(tokens);
    if !config.allowed_origins.is_empty() {
        server = server.allowed_origins(config.allowed_origins.clone());
    }
    Ok(server)
}

/// The route's [`Application`]: the gateway once built, 503 until then.
pub(crate) struct GatewayAdapter {
    slot: Arc<Mutex<Option<McpServer>>>,
}

impl GatewayAdapter {
    /// Starts building in the background; retries every [`RETRY`] until it
    /// works or the adapter is dropped (a reload replaced the route).
    pub(crate) fn start(config: McpGatewayConfig, path: String) -> GatewayAdapter {
        let slot: Arc<Mutex<Option<McpServer>>> = Arc::new(Mutex::new(None));
        let weak: Weak<Mutex<Option<McpServer>>> = Arc::downgrade(&slot);
        let spawned = std::thread::Builder::new().name("rws-mcp-gateway-build".to_string()).spawn(move || loop {
            match build(&config, &path) {
                Ok(server) => {
                    if let Some(slot) = weak.upgrade() {
                        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(server);
                        println!("mcp gateway at {path}: {} upstream(s) federated", config.upstreams.len());
                    }
                    return;
                }
                Err(e) => {
                    eprintln!("mcp gateway at {path}: {e}; retrying in {} s", RETRY.as_secs());
                    std::thread::sleep(RETRY);
                    if weak.strong_count() == 0 {
                        return;
                    }
                }
            }
        });
        if let Err(e) = spawned {
            eprintln!("mcp gateway: not started: {e}");
        }
        GatewayAdapter { slot }
    }
}

impl Application for GatewayAdapter {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let server = self.slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match server {
            Some(server) => server.execute(request, connection),
            None => Ok(unavailable()),
        }
    }
}

fn unavailable() -> Response {
    let mut response = super::service_unavailable();
    response.headers.retain(|h| !h.name.eq_ignore_ascii_case("retry-after"));
    response.headers.push(crate::header::Header::new("Retry-After", RETRY.as_secs().to_string()));
    response
}

#[cfg(test)]
mod tests;
