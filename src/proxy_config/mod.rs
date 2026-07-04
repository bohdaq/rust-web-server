//! Config-driven proxy application.
//!
//! When `rws.config.toml` contains `[[route]]` or `[[upstream]]` sections,
//! `ConfigDrivenApp` is used as the top-level `Application` instead of the
//! hardcoded `build_app()` in `main.rs`.
//!
//! # Quick start
//!
//! ```toml
//! # rws.config.toml
//! [[upstream]]
//! name = "api"
//! backends = ["localhost:3000"]
//!
//! [[route]]
//! name = "api-proxy"
//!
//! [route.match]
//! path = "/api/*"
//!
//! [route.action]
//! type = "proxy"
//!
//! [route.action.proxy]
//! upstream = "api"
//! ```

pub mod parser;
pub mod health;
pub mod builder;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::server_config::ServerConfig;

// ── Public config types ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub upstreams: Vec<UpstreamConfig>,
    pub routes: Vec<RouteConfig>,
    pub tcp_proxies: Vec<TcpProxyConfig>,
    pub udp_proxies: Vec<UdpProxyConfig>,
    pub ws_proxies: Vec<WsProxyConfig>,
    pub global_middleware: MiddlewareConfig,
}

#[derive(Debug, Clone)]
pub struct UpstreamConfig {
    pub name: String,
    pub backends: Vec<String>,
    pub strategy: String, // "round_robin" | "random" | "ip_hash"
    pub health_check: Option<HealthCheckConfig>,
    /// `true` when all backends use `https://` scheme — connections to the
    /// upstream are made over TLS. Requires the `http-client` or `http2`
    /// feature (which bring in `rustls` + `webpki-roots`).
    pub tls: bool,
}

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub path: String,
    pub interval_secs: u64,
    pub timeout_ms: u64,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub match_: MatchConfig,
    pub action: ActionConfig,
    pub middleware: MiddlewareConfig,
}

#[derive(Debug, Clone, Default)]
pub struct MatchConfig {
    pub host: Option<String>,
    pub path: Option<String>,
    pub method: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ActionConfig {
    Proxy {
        upstream: String,
        connect_timeout_ms: u64,
        read_timeout_ms: u64,
        strip_path_prefix: Option<String>,
        add_path_prefix: Option<String>,
    },
    Grpc {
        upstream: String,
        connect_timeout_ms: u64,
        read_timeout_ms: u64,
    },
    Static {
        root: String,
        index: Vec<String>,
    },
    Redirect {
        location: String,
        status: u16,
    },
    Respond {
        status: u16,
        body: String,
        content_type: String,
    },
    Mcp,
    Unknown(String),
}

#[derive(Debug, Clone, Default)]
pub struct MiddlewareConfig {
    pub rate_limit: Option<RateLimitConfig>,
    pub cache: Option<CacheConfig>,
    pub auth: Option<AuthConfig>,
    pub rewrite_request: Vec<RewriteRuleConfig>,
    pub rewrite_response: Vec<RewriteRuleConfig>,
    pub ip_allow: Vec<String>,
    pub ip_deny: Vec<String>,
    /// `timeout_ms` — if the route (including all its other middleware)
    /// doesn't produce a response within this many milliseconds, the client
    /// gets `504 Gateway Timeout` instead of waiting further. See
    /// `crate::timeout` for the underlying mechanism and its limitations.
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_secs: u64,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub ttl_secs: u64,
    pub vary_by: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum AuthConfig {
    /// `auth = { type = "basic", htpasswd_file = ".htpasswd" }`. See
    /// `crate::auth::BasicAuthLayer::from_htpasswd_file` for the supported
    /// file format (plain text and `{SHA256}` only — not Apache's `{SHA}`,
    /// `$apr1$`, or bcrypt).
    Basic { htpasswd_file: String },
    /// `auth = { type = "jwt", secret_env = "JWT_SECRET" }`. Requires the
    /// `auth` feature; verifies HS256 JWTs via `crate::auth::JwtLayer`.
    Jwt { secret_env: String },
    Bearer { token_env: String },
}

#[derive(Debug, Clone, Default)]
pub struct RewriteRuleConfig {
    pub type_: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub prefix: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub code: Option<u16>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TcpProxyConfig {
    pub name: String,
    pub listen: String,
    pub backends: Vec<String>,
    pub connect_timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct UdpProxyConfig {
    pub name: String,
    pub listen: String,
    pub backends: Vec<String>,
    pub reply_timeout_ms: u64,
    pub buffer_size: usize,
}

#[derive(Debug, Clone)]
pub struct WsProxyConfig {
    pub name: String,
    pub listen: String,
    pub backends: Vec<String>,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    /// Optional `[ws_proxy.health_check]` — same shape and semantics as
    /// `[upstream.health_check]`. The probe is a plain HTTP `GET {path}`
    /// against the backend's `host:port` (the `ws://`/`wss://` scheme only
    /// determines whether the probe connects over TLS); most WebSocket
    /// backends serve a regular HTTP health endpoint alongside their upgrade
    /// route, the same way nginx/Traefik health-check WS upstreams.
    pub health_check: Option<HealthCheckConfig>,
}

// ── ProxyConfig loading ────────────────────────────────────────────────────────

impl ProxyConfig {
    /// Returns `true` if `rws.config.toml` (or `RWS_CONFIG_FILE`) contains
    /// `[[route]]` or `[[upstream]]` sections, meaning config-driven mode
    /// should be used.
    pub fn is_proxy_mode() -> bool {
        let path = config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                contents.contains("[[route]]") || contents.contains("[[upstream]]")
            }
            Err(_) => false,
        }
    }

    /// Parse the config file and return a `ProxyConfig`.
    pub fn load() -> Self {
        let path = config_file_path();
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        Self::from_str(&contents)
    }

    /// Parse `toml` text directly into a `ProxyConfig`. Used in tests.
    pub fn from_str(toml: &str) -> Self {
        use parser::{get_array, get_str, get_u32, get_u64, section_exists};

        let map = parser::parse(toml);

        // ── upstreams ──────────────────────────────────────────────────────────
        let mut upstreams = Vec::new();
        let mut i = 0;
        loop {
            let sec = format!("upstream[{}]", i);
            if !section_exists(&map, &sec) {
                break;
            }
            let name = get_str(&map, &sec, "name");
            let backends = get_array(&map, &sec, "backends");
            let strategy = {
                let s = get_str(&map, &sec, "strategy");
                if s.is_empty() { "round_robin".to_string() } else { s }
            };
            let hc_sec = format!("{}.health_check", sec);
            let health_check = if section_exists(&map, &hc_sec) {
                Some(HealthCheckConfig {
                    path: {
                        let p = get_str(&map, &hc_sec, "path");
                        if p.is_empty() { "/health".to_string() } else { p }
                    },
                    interval_secs: get_u64(&map, &hc_sec, "interval_secs", 30),
                    timeout_ms: get_u64(&map, &hc_sec, "timeout_ms", 5000),
                    healthy_threshold: get_u32(&map, &hc_sec, "healthy_threshold", 2),
                    unhealthy_threshold: get_u32(&map, &hc_sec, "unhealthy_threshold", 3),
                })
            } else {
                None
            };
            let tls = backends.iter().any(|b| b.starts_with("https://"));
            upstreams.push(UpstreamConfig { name, backends, strategy, health_check, tls });
            i += 1;
        }

        // ── routes ─────────────────────────────────────────────────────────────
        let mut routes = Vec::new();
        let mut i = 0;
        loop {
            let sec = format!("route[{}]", i);
            if !section_exists(&map, &sec) {
                break;
            }
            let name = get_str(&map, &sec, "name");

            // match
            let m_sec = format!("{}.match", sec);
            let match_ = MatchConfig {
                host: {
                    let h = get_str(&map, &m_sec, "host");
                    if h.is_empty() { None } else { Some(h) }
                },
                path: {
                    let p = get_str(&map, &m_sec, "path");
                    if p.is_empty() { None } else { Some(p) }
                },
                method: {
                    let m = get_str(&map, &m_sec, "method");
                    if m.is_empty() { None } else { Some(m.to_uppercase()) }
                },
                content_type: {
                    let c = get_str(&map, &m_sec, "content_type");
                    if c.is_empty() { None } else { Some(c) }
                },
            };

            // action
            let a_sec = format!("{}.action", sec);
            let action_type = get_str(&map, &a_sec, "type");
            let action = match action_type.as_str() {
                "proxy" => {
                    let p_sec = format!("{}.action.proxy", sec);
                    ActionConfig::Proxy {
                        upstream: get_str(&map, &p_sec, "upstream"),
                        connect_timeout_ms: get_u64(&map, &p_sec, "connect_timeout_ms", 5000),
                        read_timeout_ms: get_u64(&map, &p_sec, "read_timeout_ms", 30000),
                        strip_path_prefix: {
                            let v = get_str(&map, &p_sec, "strip_path_prefix");
                            if v.is_empty() { None } else { Some(v) }
                        },
                        add_path_prefix: {
                            let v = get_str(&map, &p_sec, "add_path_prefix");
                            if v.is_empty() { None } else { Some(v) }
                        },
                    }
                }
                "grpc" => {
                    let p_sec = format!("{}.action.grpc", sec);
                    ActionConfig::Grpc {
                        upstream: get_str(&map, &p_sec, "upstream"),
                        connect_timeout_ms: get_u64(&map, &p_sec, "connect_timeout_ms", 5000),
                        read_timeout_ms: get_u64(&map, &p_sec, "read_timeout_ms", 30000),
                    }
                }
                "static" => {
                    let s_sec = format!("{}.action.static", sec);
                    ActionConfig::Static {
                        root: get_str(&map, &s_sec, "root"),
                        index: get_array(&map, &s_sec, "index"),
                    }
                }
                "redirect" => {
                    let r_sec = format!("{}.action.redirect", sec);
                    ActionConfig::Redirect {
                        location: get_str(&map, &r_sec, "location"),
                        status: get_u64(&map, &r_sec, "status", 301) as u16,
                    }
                }
                "respond" => {
                    let r_sec = format!("{}.action.respond", sec);
                    ActionConfig::Respond {
                        status: get_u64(&map, &r_sec, "status", 200) as u16,
                        body: get_str(&map, &r_sec, "body"),
                        content_type: {
                            let ct = get_str(&map, &r_sec, "content_type");
                            if ct.is_empty() { "text/plain".to_string() } else { ct }
                        },
                    }
                }
                "mcp" => ActionConfig::Mcp,
                other => ActionConfig::Unknown(other.to_string()),
            };

            // middleware
            let mw_sec = format!("{}.middleware", sec);
            let middleware = parse_middleware_config(&map, &mw_sec, i);

            routes.push(RouteConfig { name, match_, action, middleware });
            i += 1;
        }

        // ── tcp_proxy ──────────────────────────────────────────────────────────
        let mut tcp_proxies = Vec::new();
        let mut i = 0;
        loop {
            let sec = format!("tcp_proxy[{}]", i);
            if !section_exists(&map, &sec) {
                break;
            }
            tcp_proxies.push(TcpProxyConfig {
                name: get_str(&map, &sec, "name"),
                listen: get_str(&map, &sec, "listen"),
                backends: get_array(&map, &sec, "backends"),
                connect_timeout_ms: get_u64(&map, &sec, "connect_timeout_ms", 5000),
            });
            i += 1;
        }

        // ── udp_proxy ──────────────────────────────────────────────────────────
        let mut udp_proxies = Vec::new();
        let mut i = 0;
        loop {
            let sec = format!("udp_proxy[{}]", i);
            if !section_exists(&map, &sec) {
                break;
            }
            udp_proxies.push(UdpProxyConfig {
                name: get_str(&map, &sec, "name"),
                listen: get_str(&map, &sec, "listen"),
                backends: get_array(&map, &sec, "backends"),
                reply_timeout_ms: get_u64(&map, &sec, "reply_timeout_ms", 5000),
                buffer_size: get_u64(&map, &sec, "buffer_size", 65536) as usize,
            });
            i += 1;
        }

        // ── ws_proxy ───────────────────────────────────────────────────────────
        let mut ws_proxies = Vec::new();
        let mut i = 0;
        loop {
            let sec = format!("ws_proxy[{}]", i);
            if !section_exists(&map, &sec) {
                break;
            }
            let hc_sec = format!("{}.health_check", sec);
            let health_check = if section_exists(&map, &hc_sec) {
                Some(HealthCheckConfig {
                    path: {
                        let p = get_str(&map, &hc_sec, "path");
                        if p.is_empty() { "/health".to_string() } else { p }
                    },
                    interval_secs: get_u64(&map, &hc_sec, "interval_secs", 30),
                    timeout_ms: get_u64(&map, &hc_sec, "timeout_ms", 5000),
                    healthy_threshold: get_u32(&map, &hc_sec, "healthy_threshold", 2),
                    unhealthy_threshold: get_u32(&map, &hc_sec, "unhealthy_threshold", 3),
                })
            } else {
                None
            };
            ws_proxies.push(WsProxyConfig {
                name: get_str(&map, &sec, "name"),
                listen: get_str(&map, &sec, "listen"),
                backends: get_array(&map, &sec, "backends"),
                connect_timeout_ms: get_u64(&map, &sec, "connect_timeout_ms", 5000),
                read_timeout_ms: get_u64(&map, &sec, "read_timeout_ms", 30000),
                health_check,
            });
            i += 1;
        }

        // ── global middleware ──────────────────────────────────────────────────
        let global_middleware = parse_middleware_config(&map, "middleware", usize::MAX);

        ProxyConfig {
            upstreams,
            routes,
            tcp_proxies,
            udp_proxies,
            ws_proxies,
            global_middleware,
        }
    }
}

/// Parse a `MiddlewareConfig` from the section map at a given base path.
/// `route_idx` is used only to build inner-array section paths for rewrite rules.
fn parse_middleware_config(
    map: &parser::SectionMap,
    mw_sec: &str,
    route_idx: usize,
) -> MiddlewareConfig {
    use parser::{get_array, get_str, get_u32, get_u64, section_exists};

    let rl_sec = format!("{}.rate_limit", mw_sec);
    let rate_limit = if section_exists(map, &rl_sec) {
        Some(RateLimitConfig {
            max_requests: get_u32(map, &rl_sec, "max_requests", 1000),
            window_secs: get_u64(map, &rl_sec, "window_secs", 60),
        })
    } else {
        None
    };

    let c_sec = format!("{}.cache", mw_sec);
    let cache = if section_exists(map, &c_sec) {
        Some(CacheConfig {
            ttl_secs: get_u64(map, &c_sec, "ttl_secs", 60),
            vary_by: get_array(map, &c_sec, "vary_by"),
        })
    } else {
        None
    };

    let a_sec = format!("{}.auth", mw_sec);
    let auth = if section_exists(map, &a_sec) {
        let auth_type = get_str(map, &a_sec, "type");
        match auth_type.as_str() {
            "bearer" => Some(AuthConfig::Bearer {
                token_env: get_str(map, &a_sec, "token_env"),
            }),
            "jwt" => Some(AuthConfig::Jwt {
                secret_env: get_str(map, &a_sec, "secret_env"),
            }),
            "basic" => Some(AuthConfig::Basic {
                htpasswd_file: get_str(map, &a_sec, "htpasswd_file"),
            }),
            _ => None,
        }
    } else {
        None
    };

    // Rewrite rules — the section paths use route_idx for route-scoped rules
    // or a flat path for global middleware. We look for:
    //   route[N].middleware.rewrite.request[0], [1], …
    //   route[N].middleware.rewrite.response[0], [1], …
    // For global: middleware.rewrite.request[0], etc.
    let rewrite_request = collect_rewrite_rules(map, mw_sec, "request");
    let rewrite_response = collect_rewrite_rules(map, mw_sec, "response");

    let ip_sec = format!("{}.ip_filter", mw_sec);
    let ip_allow = if section_exists(map, &ip_sec) {
        get_array(map, &ip_sec, "allow")
    } else {
        vec![]
    };
    let ip_deny = if section_exists(map, &ip_sec) {
        get_array(map, &ip_sec, "deny")
    } else {
        vec![]
    };

    let _ = route_idx; // used implicitly via mw_sec paths

    // Flat scalar directly under [route.middleware] (or the global
    // [middleware] table), not a nested sub-table like rate_limit/cache —
    // 0/absent both mean "no timeout configured".
    let timeout_ms = match get_u64(map, mw_sec, "timeout_ms", 0) {
        0 => None,
        ms => Some(ms),
    };

    MiddlewareConfig { rate_limit, cache, auth, rewrite_request, rewrite_response, ip_allow, ip_deny, timeout_ms }
}

/// Collect `[[{mw_sec}.rewrite.{direction}]]` entries.
fn collect_rewrite_rules(
    map: &parser::SectionMap,
    mw_sec: &str,
    direction: &str,
) -> Vec<RewriteRuleConfig> {
    use parser::{get_str, get_u64};

    let mut rules = Vec::new();
    let mut j = 0;
    loop {
        let rsec = format!("{}.rewrite.{}[{}]", mw_sec, direction, j);
        if !parser::section_exists(map, &rsec) {
            break;
        }
        let code_val = get_u64(map, &rsec, "code", 0);
        rules.push(RewriteRuleConfig {
            type_: get_str(map, &rsec, "type"),
            name: {
                let v = get_str(map, &rsec, "name");
                if v.is_empty() { None } else { Some(v) }
            },
            value: {
                let v = get_str(map, &rsec, "value");
                if v.is_empty() { None } else { Some(v) }
            },
            prefix: {
                let v = get_str(map, &rsec, "prefix");
                if v.is_empty() { None } else { Some(v) }
            },
            from: {
                let v = get_str(map, &rsec, "from");
                if v.is_empty() { None } else { Some(v) }
            },
            to: {
                let v = get_str(map, &rsec, "to");
                if v.is_empty() { None } else { Some(v) }
            },
            code: if code_val == 0 { None } else { Some(code_val as u16) },
            reason: {
                let v = get_str(map, &rsec, "reason");
                if v.is_empty() { None } else { Some(v) }
            },
        });
        j += 1;
    }
    rules
}

fn config_file_path() -> String {
    std::env::var("RWS_CONFIG_FILE").unwrap_or_else(|_| "rws.config.toml".to_string())
}

// ── ConfigDrivenApp ────────────────────────────────────────────────────────────

/// A compiled route: a matcher paired with a handler application.
pub(crate) struct CompiledRoute {
    pub(crate) matcher: RouteMatcher,
    /// Shared, type-erased handler. `Arc` makes `Clone` cheap (pointer copy).
    pub(crate) handler: Arc<dyn Application + Send + Sync>,
}

/// Matching criteria for a single route.
#[derive(Clone, Default)]
pub(crate) struct RouteMatcher {
    /// Optional SNI hostname / `Host` header match.
    pub(crate) host: Option<String>,
    /// Path prefix to match (derived from `path = "/v1/*"`).
    pub(crate) path_prefix: Option<String>,
    /// Exact path to match (derived from `path = "/v1/ping"`).
    pub(crate) path_exact: Option<String>,
    /// Uppercase HTTP method, or `None` for any.
    pub(crate) method: Option<String>,
    /// `Content-Type` prefix (e.g. `"application/grpc"`).
    pub(crate) content_type_prefix: Option<String>,
}

impl RouteMatcher {
    pub(crate) fn from_match_config(cfg: &MatchConfig) -> Self {
        let (path_prefix, path_exact) = match &cfg.path {
            Some(p) if p.ends_with('*') => {
                // "/v1/*" → prefix "/v1/"
                let stripped = p.trim_end_matches('*').to_string();
                (Some(stripped), None)
            }
            Some(p) => (None, Some(p.clone())),
            None => (None, None),
        };
        let content_type_prefix = cfg.content_type.as_ref().map(|ct| {
            if ct.ends_with('*') {
                ct.trim_end_matches('*').to_string()
            } else {
                ct.clone()
            }
        });
        RouteMatcher {
            host: cfg.host.clone(),
            path_prefix,
            path_exact,
            method: cfg.method.clone(),
            content_type_prefix,
        }
    }

    /// Returns `true` if `request` and `conn` match all configured criteria.
    pub(crate) fn matches(&self, request: &Request, conn: &ConnectionInfo) -> bool {
        // Host matching: SNI first, then Host header
        if let Some(ref expected_host) = self.host {
            let actual_host = conn
                .sni_hostname
                .as_deref()
                .or_else(|| {
                    request
                        .headers
                        .iter()
                        .find(|h| h.name.eq_ignore_ascii_case("host"))
                        .map(|h| h.value.as_str())
                })
                .unwrap_or("");
            if actual_host != expected_host.as_str() {
                return false;
            }
        }

        // Method matching
        if let Some(ref m) = self.method {
            if request.method.to_uppercase() != m.as_str() {
                return false;
            }
        }

        // Path matching: strip query string for comparison
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        if let Some(ref prefix) = self.path_prefix {
            if !path.starts_with(prefix.as_str()) {
                return false;
            }
        } else if let Some(ref exact) = self.path_exact {
            if path != exact.as_str() {
                return false;
            }
        }

        // Content-Type prefix matching
        if let Some(ref ct_prefix) = self.content_type_prefix {
            let actual_ct = request
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case("content-type"))
                .map(|h| h.value.as_str())
                .unwrap_or("");
            if !actual_ct.starts_with(ct_prefix.as_str()) {
                return false;
            }
        }

        true
    }
}

/// An `Application` that routes requests based on a parsed `ProxyConfig`.
///
/// `Clone` is cheap: `routes` is an `Arc<Vec<...>>` (pointer copy), and
/// `fallback` is `App`, itself cheap to clone (an `Option<Arc<ServerConfig>>`).
#[derive(Clone)]
pub struct ConfigDrivenApp {
    routes: Arc<Vec<CompiledRoute>>,
    /// Fallback for unmatched requests — handles /healthz, /readyz, /metrics,
    /// static files, and the 404 controller. Reads `RWS_CONFIG_*` env vars
    /// per request (`App::new()`'s default) unless pinned via
    /// [`ConfigDrivenApp::with_config`].
    fallback: App,
}

impl ConfigDrivenApp {
    pub(crate) fn new(routes: Vec<CompiledRoute>) -> Self {
        use crate::core::New;
        ConfigDrivenApp {
            routes: Arc::new(routes),
            fallback: App::new(),
        }
    }

    /// Pin the fallback [`App`] (used for any request none of the
    /// config-driven routes match) to an explicit [`ServerConfig`], instead
    /// of reading `RWS_CONFIG_*` environment variables per request.
    ///
    /// Mirrors [`App::with_config`] / [`crate::state::AppWithState::with_config`]
    /// — same rationale: safe for parallel tests, and lets multiple
    /// differently-configured proxy instances coexist in one process.
    ///
    /// ```rust,no_run
    /// use rust_web_server::proxy_config::build_from_file;
    /// use rust_web_server::server_config::ServerConfig;
    ///
    /// let (app, _handles) = build_from_file();
    /// let app = app.with_config(ServerConfig::default());
    /// ```
    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.fallback = App::with_config(config);
        self
    }
}

impl Application for ConfigDrivenApp {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        for route in self.routes.iter() {
            if route.matcher.matches(request, conn) {
                return route.handler.execute(request, conn);
            }
        }
        self.fallback.execute(request, conn)
    }
}

// ── NullApp ────────────────────────────────────────────────────────────────────

/// A dead-end `Application` that always returns 404.
/// Used as the `next` parameter when calling `Middleware::handle` directly.
#[derive(Clone, Copy)]
pub(crate) struct NullApp;

impl Application for NullApp {
    fn execute(&self, _request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
        Ok(r)
    }
}

// ── DynamicProxy ──────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Backend-selection strategy for `DynamicProxy`, configured via the
/// `strategy` field on `[[upstream]]` in `rws.config.toml`. Unknown or empty
/// values fall back to `RoundRobin`, matching the parser's own default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadBalanceStrategy {
    RoundRobin,
    Random,
    IpHash,
    LeastConnections,
}

impl LoadBalanceStrategy {
    fn parse(s: &str) -> Self {
        match s {
            "random" => LoadBalanceStrategy::Random,
            "ip_hash" => LoadBalanceStrategy::IpHash,
            "least_connections" => LoadBalanceStrategy::LeastConnections,
            _ => LoadBalanceStrategy::RoundRobin,
        }
    }
}

/// A proxy adapter that reads its backend list from a shared, health-checker-
/// maintained live list at request time. Supports dynamic removal/restoration
/// of backends without restarting.
///
/// This type is `Clone + Send + Sync` and implements `Application`.
#[derive(Clone)]
pub(crate) struct DynamicProxy {
    live: Arc<RwLock<Vec<String>>>,
    counter: Arc<AtomicUsize>,
    connect_timeout: Duration,
    read_timeout: Duration,
    strip_prefix: Option<Arc<String>>,
    add_prefix: Option<Arc<String>>,
    tls: bool,
    strategy: LoadBalanceStrategy,
    connections: Arc<RwLock<HashMap<String, Arc<AtomicUsize>>>>,
}

impl DynamicProxy {
    pub(crate) fn new(
        live: Arc<RwLock<Vec<String>>>,
        connect_timeout_ms: u64,
        read_timeout_ms: u64,
        strip_prefix: Option<String>,
        add_prefix: Option<String>,
        tls: bool,
        strategy: String,
    ) -> Self {
        DynamicProxy {
            live,
            counter: Arc::new(AtomicUsize::new(0)),
            connect_timeout: Duration::from_millis(connect_timeout_ms),
            read_timeout: Duration::from_millis(read_timeout_ms),
            strip_prefix: strip_prefix.map(Arc::new),
            add_prefix: add_prefix.map(Arc::new),
            tls,
            strategy: LoadBalanceStrategy::parse(&strategy),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn next_backend(&self, client_ip: &str) -> Option<String> {
        let live = self.live.read().unwrap();
        if live.is_empty() {
            return None;
        }

        let idx = match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                self.counter.fetch_add(1, Ordering::Relaxed) % live.len()
            }
            LoadBalanceStrategy::Random => {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0) as usize;
                let salt = self.counter.fetch_add(1, Ordering::Relaxed);
                nanos.wrapping_add(salt) % live.len()
            }
            LoadBalanceStrategy::IpHash => {
                let mut hasher = DefaultHasher::new();
                client_ip.hash(&mut hasher);
                (hasher.finish() as usize) % live.len()
            }
            LoadBalanceStrategy::LeastConnections => {
                let connections = self.connections.read().unwrap();
                live.iter()
                    .enumerate()
                    .min_by_key(|(_, backend)| {
                        connections
                            .get(*backend)
                            .map(|c| c.load(Ordering::Relaxed))
                            .unwrap_or(0)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        };

        Some(live[idx].clone())
    }

    /// Returns the shared in-flight connection counter for `backend`,
    /// creating it on first use. Only consulted under `LeastConnections`.
    fn connection_counter(&self, backend: &str) -> Arc<AtomicUsize> {
        if let Some(counter) = self.connections.read().unwrap().get(backend) {
            return Arc::clone(counter);
        }
        let mut connections = self.connections.write().unwrap();
        Arc::clone(
            connections
                .entry(backend.to_string())
                .or_insert_with(|| Arc::new(AtomicUsize::new(0))),
        )
    }
}

/// Decrements a backend's in-flight connection count when the request
/// finishes (including early returns), keeping `least_connections` accurate.
struct ConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Application for DynamicProxy {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        let backend = match self.next_backend(&conn.client.ip) {
            Some(b) => b,
            None => {
                return Ok(bad_gateway());
            }
        };

        let _connection_guard = if self.strategy == LoadBalanceStrategy::LeastConnections {
            let counter = self.connection_counter(&backend);
            counter.fetch_add(1, Ordering::Relaxed);
            Some(ConnectionGuard { counter })
        } else {
            None
        };

        let (host, port, _) = match crate::proxy_config::health::parse_backend_url(&backend) {
            Some(t) => t,
            None => return Ok(bad_gateway()),
        };

        // Apply path rewriting if configured
        let mut req_clone;
        let effective_request = if self.strip_prefix.is_some() || self.add_prefix.is_some() {
            req_clone = request.clone();
            if let Some(ref sp) = self.strip_prefix {
                if let Some(stripped) = req_clone.request_uri.strip_prefix(sp.as_str()) {
                    req_clone.request_uri = if stripped.is_empty() || !stripped.starts_with('/') {
                        format!("/{}", stripped)
                    } else {
                        stripped.to_string()
                    };
                }
            }
            if let Some(ref ap) = self.add_prefix {
                req_clone.request_uri = format!("{}{}", ap, req_clone.request_uri);
            }
            &req_clone
        } else {
            request
        };

        let result = if self.tls {
            #[cfg(any(feature = "http-client", feature = "http2"))]
            {
                crate::proxy::proxy_https1(
                    effective_request,
                    &conn.client.ip,
                    &host,
                    port,
                    self.connect_timeout,
                    self.read_timeout,
                )
            }
            #[cfg(not(any(feature = "http-client", feature = "http2")))]
            {
                eprintln!("[proxy] HTTPS upstream requires http-client or http2 feature");
                Err("TLS upstream not supported in this build".to_string())
            }
        } else {
            crate::proxy::proxy_http1(
                effective_request,
                &conn.client.ip,
                &host,
                port,
                self.connect_timeout,
                self.read_timeout,
            )
        };

        match result {
            Ok(r) => Ok(r),
            Err(_) => Ok(bad_gateway()),
        }
    }
}

fn bad_gateway() -> Response {
    use crate::mime_type::MimeType;
    use crate::range::Range;
    let cr = Range::get_content_range(
        b"502 Bad Gateway".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}

// ── RedirectAdapter ────────────────────────────────────────────────────────────

/// Action adapter that issues HTTP redirects.
///
/// `$path` in `location_template` is replaced with the request URI at runtime.
#[derive(Clone)]
pub(crate) struct RedirectAdapter {
    location_template: Arc<String>,
    status: i16,
    reason: Arc<String>,
}

impl RedirectAdapter {
    pub(crate) fn new(location: String, status: u16) -> Self {
        let (code, reason) = redirect_status(status);
        RedirectAdapter {
            location_template: Arc::new(location),
            status: code,
            reason: Arc::new(reason),
        }
    }
}

fn redirect_status(code: u16) -> (i16, String) {
    let phrase = match code {
        301 => STATUS_CODE_REASON_PHRASE.n301_moved_permanently.reason_phrase,
        302 => STATUS_CODE_REASON_PHRASE.n302_found.reason_phrase,
        307 => STATUS_CODE_REASON_PHRASE.n307_temporary_redirect.reason_phrase,
        308 => STATUS_CODE_REASON_PHRASE.n308_permanent_redirect.reason_phrase,
        _ => "Redirect",
    };
    (code as i16, phrase.to_string())
}

impl Application for RedirectAdapter {
    fn execute(&self, request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        let location = self
            .location_template
            .replace("$path", &request.request_uri);
        use crate::header::Header;
        let mut r = Response::new();
        r.status_code = self.status;
        r.reason_phrase = self.reason.as_ref().clone();
        r.headers.push(Header { name: "Location".to_string(), value: location });
        Ok(r)
    }
}

// ── RespondAdapter ─────────────────────────────────────────────────────────────

/// Action adapter that returns a fixed response body.
#[derive(Clone)]
pub(crate) struct RespondAdapter {
    status: i16,
    reason: Arc<String>,
    body: Arc<Vec<u8>>,
    content_type: Arc<String>,
}

impl RespondAdapter {
    pub(crate) fn new(status: u16, body: String, content_type: String) -> Self {
        use crate::response::STATUS_CODE_REASON_PHRASE;
        let reason = match status {
            200 => STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
            201 => STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string(),
            204 => STATUS_CODE_REASON_PHRASE.n204_no_content.reason_phrase.to_string(),
            400 => STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string(),
            401 => STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string(),
            403 => STATUS_CODE_REASON_PHRASE.n403_forbidden.reason_phrase.to_string(),
            404 => STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string(),
            500 => STATUS_CODE_REASON_PHRASE.n500_internal_server_error.reason_phrase.to_string(),
            _ => "OK".to_string(),
        };
        RespondAdapter {
            status: status as i16,
            reason: Arc::new(reason),
            body: Arc::new(body.into_bytes()),
            content_type: Arc::new(content_type),
        }
    }
}

impl Application for RespondAdapter {
    fn execute(&self, _request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        use crate::range::Range;
        let mut r = Response::new();
        r.status_code = self.status;
        r.reason_phrase = self.reason.as_ref().clone();
        if !self.body.is_empty() {
            r.content_range_list = vec![Range::get_content_range(
                self.body.as_ref().clone(),
                self.content_type.as_ref().clone(),
            )];
        }
        Ok(r)
    }
}

// ── StaticAdapter ──────────────────────────────────────────────────────────────

/// Action adapter that serves static files from a configured `root` directory.
///
/// Unlike `StaticResourceController` (which always resolves paths relative to
/// the process's current working directory), this adapter is parameterized
/// per-route by `ActionConfig::Static { root, index }` from `rws.config.toml`,
/// so a config-driven proxy can serve an arbitrary directory without Rust code.
#[derive(Clone)]
pub(crate) struct StaticAdapter {
    root: Arc<std::path::PathBuf>,
    index: Arc<Vec<String>>,
}

impl StaticAdapter {
    pub(crate) fn new(root: String, index: Vec<String>) -> Self {
        let index = if index.is_empty() { vec!["index.html".to_string()] } else { index };
        StaticAdapter {
            root: Arc::new(std::path::PathBuf::from(root)),
            index: Arc::new(index),
        }
    }

    /// Resolves `request_uri` against `root`. Returns `None` if the decoded
    /// path contains a `..` segment, which would otherwise let a request
    /// escape the configured root directory.
    fn resolve(&self, request_uri: &str) -> Option<std::path::PathBuf> {
        let raw_path = request_uri.split('?').next().unwrap_or(request_uri);
        let decoded = crate::url::URL::percent_decode(raw_path);

        if decoded.split('/').any(|segment| segment == "..") {
            return None;
        }

        let relative = decoded.trim_start_matches('/');
        Some(self.root.join(relative))
    }
}

impl Application for StaticAdapter {
    fn execute(&self, request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        let mut response = Response::new();

        let not_found = |mut response: Response| {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
            response
        };

        let candidate = match self.resolve(&request.request_uri) {
            Some(p) => p,
            None => {
                response.status_code = *STATUS_CODE_REASON_PHRASE.n403_forbidden.status_code;
                response.reason_phrase = STATUS_CODE_REASON_PHRASE.n403_forbidden.reason_phrase.to_string();
                return Ok(response);
            }
        };

        let mut file_path = candidate;
        if file_path.is_dir() {
            let indexed = self
                .index
                .iter()
                .map(|name| file_path.join(name))
                .find(|p| p.is_file());

            file_path = match indexed {
                Some(p) => p,
                None => return Ok(not_found(response)),
            };
        }

        if !file_path.is_file() {
            return Ok(not_found(response));
        }

        // Defense-in-depth against symlinks inside `root` that point outside it —
        // the `..`-segment check above only catches traversal in the request URI.
        if let (Ok(root_canon), Ok(file_canon)) =
            (self.root.canonicalize(), file_path.canonicalize())
        {
            if !file_canon.starts_with(&root_canon) {
                return Ok(not_found(response));
            }
        }

        let path_str = match file_path.to_str() {
            Some(s) => s,
            None => return Ok(not_found(response)),
        };

        match crate::range::Range::get_content_range_of_a_file(path_str) {
            Ok(content_range) => {
                response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                response.content_range_list = vec![content_range];
                Ok(response)
            }
            Err(_) => Ok(not_found(response)),
        }
    }
}

// ── PerRouteRateLimit middleware ───────────────────────────────────────────────

/// A per-route rate limiter middleware backed by a shared `RateLimiter`.
pub(crate) struct PerRouteRateLimit(pub(crate) Arc<crate::rate_limit::RateLimiter>);

impl crate::middleware::Middleware for PerRouteRateLimit {
    fn handle(
        &self,
        request: &Request,
        conn: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        use crate::error::{AppError, IntoResponse};
        if self.0.check(&conn.client.ip) {
            next.execute(request, conn)
        } else {
            Ok(AppError::TooManyRequests.into_response())
        }
    }
}

// ── BearerAuthMiddleware ───────────────────────────────────────────────────────

/// Bearer token authentication middleware.
pub(crate) struct BearerAuthMiddleware {
    pub(crate) token: Arc<String>,
}

impl crate::middleware::Middleware for BearerAuthMiddleware {
    fn handle(
        &self,
        request: &Request,
        conn: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        use crate::error::{AppError, IntoResponse};
        let expected = format!("Bearer {}", self.token);
        let authorized = request
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("authorization") && h.value == expected);
        if authorized {
            next.execute(request, conn)
        } else {
            Ok(AppError::Unauthorized.into_response())
        }
    }
}

// ── arc_app helper ─────────────────────────────────────────────────────────────

/// Box any `Application + Send + Sync + 'static` into an `Arc<dyn …>`.
pub(crate) fn arc_app<A: Application + Send + Sync + 'static>(
    a: A,
) -> Arc<dyn Application + Send + Sync> {
    Arc::new(a)
}

// ── Public entry points ────────────────────────────────────────────────────────

/// Build a `ConfigDrivenApp` from `rws.config.toml` and spawn L4/WS proxy
/// threads. Returns the app and a list of thread handles.
pub fn build_from_file() -> (ConfigDrivenApp, Vec<std::thread::JoinHandle<()>>) {
    builder::build_from_file()
}
