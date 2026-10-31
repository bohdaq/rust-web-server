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
pub(crate) mod upstream;

pub use upstream::{BreakerConfig, RetryConfig, StickyConfig};
pub mod builder;
pub mod validate;
pub mod mcp_gateway;
pub mod llm;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::request::Request;
use crate::header::Header;
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
    /// `tls_ca_file` — PEM file of CA certificates to trust for this
    /// upstream's `https://` backends **instead of** the public WebPKI roots
    /// (a private CA or a self-signed certificate). `None` = WebPKI.
    pub tls_ca_file: Option<String>,
    /// `tls_client_cert_file` / `tls_client_key_file` — a client certificate
    /// (PEM chain and key) presented to TLS backends that require one.
    /// Since 18.3.0.
    pub tls_client_cert_file: Option<String>,
    pub tls_client_key_file: Option<String>,
    /// `weights` — one per backend, in order (default 1 each). Since 18.6.0.
    pub weights: Vec<u32>,
    /// `[upstream.sticky]` — keep a client on one backend. Since 18.6.0.
    pub sticky: Option<StickyConfig>,
    /// `[upstream.circuit_breaker]` — take failing backends out of rotation
    /// (passive health checks). Since 18.6.0.
    pub circuit_breaker: Option<BreakerConfig>,
    /// `[upstream.retry]` — try another backend. Since 18.6.0.
    pub retry: Option<RetryConfig>,
    /// `protocol` — `"h3"` sends requests over HTTP/3 (QUIC) to the
    /// `https://` backends, falling back to TCP for a backend QUIC can't
    /// reach. Empty (the default) or `"http1"`: HTTP/1.1 as before. Since
    /// 18.13.0.
    pub protocol: String,
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
    /// The MCP gateway ([`mcp_gateway`]).
    Mcp(Box<mcp_gateway::McpGatewayConfig>),
    /// The LLM proxy ([`llm`]).
    Llm(Box<llm::LlmConfig>),
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
    /// `max_body_size` — reject this route's request with `413 Payload Too
    /// Large` if its body exceeds this many bytes.
    ///
    /// This is a *route-specific* ceiling on top of the global, process-wide
    /// `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` (see `crate::entry_point::get_max_body_size`)
    /// — e.g. a small JSON API route can be capped stricter than a
    /// file-upload route without lowering the global limit for everyone.
    /// It is **not** a substitute for the global limit: by the time any
    /// route-level middleware runs, `Request` (including its body) has
    /// already been fully read off the socket — route matching itself
    /// requires the complete request, so a per-route check can only run
    /// after that memory is already spent, not before. The global limit is
    /// what protects total server memory from an oversized upload;
    /// `max_body_size` protects only this route's own logic/upstream from
    /// an oversized-but-still-within-the-global-limit body.
    pub max_body_size: Option<u64>,
    /// `[route.middleware.wasm]`: a WebAssembly plugin (feature
    /// `wasm-plugins`, 18.10.0).
    pub wasm: Option<WasmConfig>,
    /// `early_hints = ["</app.css>; rel=preload; as=style", …]`: `Link`
    /// values sent as a `103 Early Hints` before the route answers (18.12.0).
    pub early_hints: Vec<String>,
    /// `[route.middleware.mirror]`: copy a share of requests to other
    /// backends (18.11.0).
    pub mirror: Option<MirrorConfig>,
}

/// `[route.middleware.mirror]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MirrorConfig {
    /// A declared `[[upstream]]` (its health-checked backends) ...
    pub upstream: String,
    /// ... or backend URLs.
    pub backends: Vec<String>,
    /// Share of requests copied, 0–100.
    pub percent: f64,
}

/// `[route.middleware.wasm]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WasmConfig {
    /// Path of the `.wasm` file.
    pub module: String,
    pub fuel: u64,
    pub memory_mb: u64,
    /// The string the plugin's `rws_init` gets.
    pub config: String,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_secs: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CacheConfig {
    pub ttl_secs: u64,
    pub vary_by: Vec<String>,
    /// Keep entries in files under this directory (18.11.0); empty = memory.
    pub dir: String,
    /// Bound on the disk store's total size, e.g. `"512M"` (default 256 MiB).
    pub max_size: String,
    /// Environment variable with the bearer token `PURGE` requests need;
    /// empty = no `PURGE` (18.11.0).
    pub purge_token_env: String,
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


/// An action's settings may sit directly under `[route.action]` next to
/// `type` (the short form README.md and the docs use), or in a nested
/// `[route.action.<type>]` table (the long form). Copy short-form keys into
/// the nested table so the parser reads one place; a key set in both keeps
/// the nested value.
fn merge_action_keys(map: &mut parser::SectionMap, action_section: &str, action_type: &str) {
    if action_type.is_empty() {
        return;
    }
    let short_form: Vec<(String, String)> = match map.get(action_section) {
        Some(entries) => entries.iter().filter(|(k, _)| k != "type").cloned().collect(),
        None => return,
    };
    let nested = map.entry(format!("{action_section}.{action_type}")).or_default();
    for (key, value) in short_form {
        if !nested.iter().any(|(k, _)| *k == key) {
            nested.push((key, value));
        }
    }
}

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

        let mut map = parser::parse(toml);

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
            let tls = backends.iter().any(|b| ["https://", "h2s://", "grpcs://"].iter().any(|s| b.starts_with(s)));
            let tls_ca_file = Some(get_str(&map, &sec, "tls_ca_file")).filter(|v| !v.is_empty());
            let tls_client_cert_file = Some(get_str(&map, &sec, "tls_client_cert_file")).filter(|v| !v.is_empty());
            let tls_client_key_file = Some(get_str(&map, &sec, "tls_client_key_file")).filter(|v| !v.is_empty());
            let weights: Vec<u32> = get_array(&map, &sec, "weights").iter().map(|w| w.trim().parse().unwrap_or(0)).collect();
            let sticky_sec = format!("{}.sticky", sec);
            let sticky = section_exists(&map, &sticky_sec).then(|| StickyConfig {
                cookie: Some(get_str(&map, &sticky_sec, "cookie")).filter(|c| !c.is_empty()).unwrap_or_else(|| "rws_backend".to_string()),
                max_age_secs: Some(get_u64(&map, &sticky_sec, "max_age_secs", 0)).filter(|s| *s > 0),
            });
            let cb_sec = format!("{}.circuit_breaker", sec);
            let circuit_breaker = section_exists(&map, &cb_sec).then(|| BreakerConfig {
                failure_threshold: get_u32(&map, &cb_sec, "failure_threshold", 5),
                recovery_secs: get_u64(&map, &cb_sec, "recovery_secs", 30),
                half_open_probes: get_u32(&map, &cb_sec, "half_open_probes", 1),
            });
            let retry_sec = format!("{}.retry", sec);
            let retry = section_exists(&map, &retry_sec).then(|| {
                let status: Vec<i16> = get_array(&map, &retry_sec, "status").iter().filter_map(|s| s.trim().parse().ok()).collect();
                RetryConfig { attempts: get_u32(&map, &retry_sec, "attempts", 1), status: if status.is_empty() { vec![502, 503, 504] } else { status } }
            });
            upstreams.push(UpstreamConfig {
                name, backends, strategy, health_check, tls, tls_ca_file, tls_client_cert_file, tls_client_key_file,
                weights, sticky, circuit_breaker, retry,
                protocol: get_str(&map, &sec, "protocol"),
            });
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
            merge_action_keys(&mut map, &a_sec, &action_type);
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
                "mcp" => ActionConfig::Mcp(Box::new(mcp_gateway::parse(&map, &a_sec))),
                "llm" => ActionConfig::Llm(Box::new(llm::parse(&map, &a_sec))),
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
            dir: get_str(map, &c_sec, "dir"),
            max_size: get_str(map, &c_sec, "max_size"),
            purge_token_env: get_str(map, &c_sec, "purge_token_env"),
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

    // Same flat-scalar convention as timeout_ms — 0/absent means "no
    // route-specific limit" (the global RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    // still applies regardless).
    let max_body_size = match get_u64(map, mw_sec, "max_body_size", 0) {
        0 => None,
        n => Some(n),
    };

    let w_sec = format!("{}.wasm", mw_sec);
    let wasm = section_exists(map, &w_sec).then(|| WasmConfig {
        module: get_str(map, &w_sec, "module"),
        fuel: get_u64(map, &w_sec, "fuel", 50_000_000),
        memory_mb: get_u64(map, &w_sec, "memory_mb", 16),
        config: get_str(map, &w_sec, "config"),
    });

    let m_sec = format!("{}.mirror", mw_sec);
    let mirror = section_exists(map, &m_sec).then(|| MirrorConfig {
        upstream: get_str(map, &m_sec, "upstream"),
        backends: get_array(map, &m_sec, "backends"),
        percent: {
            let raw = get_str(map, &m_sec, "percent");
            if raw.is_empty() { 100.0 } else { raw.parse::<f64>().unwrap_or(f64::NAN) }
        },
    });

    MiddlewareConfig {
        rate_limit, cache, auth, rewrite_request, rewrite_response, ip_allow, ip_deny,
        timeout_ms, max_body_size, wasm, mirror,
        early_hints: get_array(map, mw_sec, "early_hints"),
    }
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

pub(crate) fn config_file_path() -> String {
    std::env::var("RWS_CONFIG_FILE").unwrap_or_else(|_| "rws.config.toml".to_string())
}

// ── ConfigDrivenApp ────────────────────────────────────────────────────────────

/// A compiled route: a matcher paired with a handler application.
pub(crate) struct CompiledRoute {
    pub(crate) matcher: RouteMatcher,
    /// Shared, type-erased handler. `Arc` makes `Clone` cheap (pointer copy).
    pub(crate) handler: Arc<dyn Application + Send + Sync>,
    /// The route's proxy when it can be served without blocking a thread
    /// (plain-HTTP upstream, no route middleware) — see
    /// [`DynamicProxy::execute_async`].
    #[cfg_attr(not(feature = "http2"), allow(dead_code))]
    pub(crate) async_proxy: Option<Arc<DynamicProxy>>,
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
                        .map(|h| h.value.as_ref())
                })
                .unwrap_or("");
            if !host_matches(expected_host, actual_host) {
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
                .map(|h| h.value.as_ref())
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
/// Clones share one live configuration: a hot reload
/// ([`ConfigDrivenApp::reload_from_str`], run on `SIGHUP` for an app built by
/// [`build_from_file`]) swaps it for every clone at once. In-flight requests
/// finish on the configuration they started with.
#[derive(Clone)]
pub struct ConfigDrivenApp {
    current: Arc<RwLock<Arc<Generation>>>,
    /// Serializes reloads: one SIGHUP reaches every accept loop.
    reload_lock: Arc<std::sync::Mutex<()>>,
}

/// One loaded configuration: its compiled routes plus what a reload needs.
struct Generation {
    dispatch: RouteDispatch,
    /// `dispatch` wrapped in the top-level `[middleware]` table; `None` when
    /// that table is empty.
    entry: Option<Arc<dyn Application + Send + Sync>>,
    config: ProxyConfig,
    /// The file text this generation was built from, to skip no-op reloads.
    source: Option<String>,
    /// Set when this generation is replaced: stops its health checkers.
    health_stop: Arc<AtomicBool>,
}

impl Generation {
    fn new(routes: Arc<Vec<CompiledRoute>>, fallback: App, config: ProxyConfig, health_stop: Arc<AtomicBool>, source: Option<String>) -> Self {
        let dispatch = RouteDispatch { routes, fallback };
        let global = &config.global_middleware;
        let entry = if global.is_empty() && global.early_hints.is_empty() {
            None
        } else {
            let mut entry = builder::apply_middleware(arc_app(dispatch.clone()), global);
            // Global early hints go out first, for every request (routes,
            // static files, health checks alike).
            if !global.early_hints.is_empty() {
                let layer = crate::early_hints::EarlyHintsLayer::new(global.early_hints.iter().cloned());
                entry = arc_app(crate::middleware::WithMiddleware::new(builder::ArcApp(entry)).wrap(layer));
            }
            Some(entry)
        };
        Generation { dispatch, entry, config, source, health_stop }
    }
}

/// First-match routing over the compiled routes, then the fallback.
#[derive(Clone)]
struct RouteDispatch {
    routes: Arc<Vec<CompiledRoute>>,
    /// Fallback for unmatched requests — handles /healthz, /readyz, /metrics,
    /// static files, and the 404 controller. Reads `RWS_CONFIG_*` env vars
    /// per request (`App::new()`'s default) unless pinned via
    /// [`ConfigDrivenApp::with_config`].
    fallback: App,
}

impl Application for RouteDispatch {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        for route in self.routes.iter() {
            if route.matcher.matches(request, conn) {
                return route.handler.execute(request, conn);
            }
        }
        self.fallback.execute(request, conn)
    }

    fn streams_body(&self, request: &Request, conn: &ConnectionInfo) -> bool {
        match self.routes.iter().find(|r| r.matcher.matches(request, conn)) {
            Some(route) => route.handler.streams_body(request, conn),
            None => self.fallback.streams_body(request, conn),
        }
    }
}

impl MiddlewareConfig {
    /// `true` when no layer is configured. A mirror and early hints don't
    /// count: proxy routes send both from `DynamicProxy` itself, so such a
    /// route keeps the non-blocking forwarder.
    pub fn is_empty(&self) -> bool {
        self.rate_limit.is_none() && self.cache.is_none() && self.auth.is_none()
            && self.rewrite_request.is_empty() && self.rewrite_response.is_empty()
            && self.ip_allow.is_empty() && self.ip_deny.is_empty()
            && self.timeout_ms.is_none() && self.max_body_size.is_none() && self.wasm.is_none()
    }
}

/// What a successful [`ConfigDrivenApp::reload_from_str`] changed.
#[derive(Debug, Clone, PartialEq)]
pub enum ReloadOutcome {
    /// The configuration text is unchanged; nothing was rebuilt.
    Unchanged,
    /// New routes are live. `warnings` includes `[[tcp_proxy]]` /
    /// `[[udp_proxy]]` / `[[ws_proxy]]` changes, which need a restart.
    Reloaded { routes: usize, warnings: Vec<String> },
}

impl ConfigDrivenApp {
    pub(crate) fn new(routes: Vec<CompiledRoute>, config: ProxyConfig, health_stop: Arc<AtomicBool>, source: Option<String>) -> Self {
        use crate::core::New;
        let generation = Generation::new(Arc::new(routes), App::new(), config, health_stop, source);
        ConfigDrivenApp {
            current: Arc::new(RwLock::new(Arc::new(generation))),
            reload_lock: Arc::new(std::sync::Mutex::new(())),
        }
    }

    fn current(&self) -> Arc<Generation> {
        Arc::clone(&self.current.read().unwrap_or_else(|e| e.into_inner()))
    }

    /// Pin the fallback [`App`] (used for any request none of the
    /// config-driven routes match) to an explicit [`ServerConfig`], instead
    /// of reading `RWS_CONFIG_*` environment variables per request.
    ///
    /// Mirrors [`App::with_config`] / [`crate::state::AppWithState::with_config`]
    /// — same rationale: safe for parallel tests, and lets multiple
    /// differently-configured proxy instances coexist in one process. The
    /// pinned fallback is kept across hot reloads.
    ///
    /// ```rust,no_run
    /// use rust_web_server::proxy_config::build_from_file;
    /// use rust_web_server::server_config::ServerConfig;
    ///
    /// let (app, _handles) = build_from_file();
    /// let app = app.with_config(ServerConfig::default());
    /// ```
    pub fn with_config(self, config: ServerConfig) -> Self {
        {
            let mut slot = self.current.write().unwrap_or_else(|e| e.into_inner());
            let old = Arc::clone(&slot);
            *slot = Arc::new(Generation::new(
                Arc::clone(&old.dispatch.routes),
                App::with_config(config),
                old.config.clone(),
                Arc::clone(&old.health_stop),
                old.source.clone(),
            ));
        }
        self
    }

    /// Replace the routes, upstreams and `[middleware]` with those in `toml`.
    ///
    /// The new configuration is validated first ([`ProxyConfig::validate`]
    /// plus unknown keys); if it has errors, the running configuration stays
    /// in place and the errors are returned — a typo never takes the proxy
    /// down. Otherwise the new routes go live for every clone of this app,
    /// the previous configuration's health checkers stop, and new ones start.
    /// `[[tcp_proxy]]` / `[[udp_proxy]]` / `[[ws_proxy]]` listeners are not
    /// rebound; a change to them is reported as a warning.
    pub fn reload_from_str(&self, toml: &str) -> Result<ReloadOutcome, crate::config_check::ConfigReport> {
        let _serialized = self.reload_lock.lock().unwrap_or_else(|e| e.into_inner());
        let old = self.current();
        if old.source.as_deref() == Some(toml) {
            return Ok(ReloadOutcome::Unchanged);
        }
        let config = ProxyConfig::from_str(toml);
        let mut report = crate::config_check::ConfigReport::default();
        validate::check_unknown_keys(toml, &mut report);
        config.validate(&mut report);
        if !report.is_ok() {
            return Err(report);
        }

        let mut warnings = report.warnings;
        let listeners = |c: &ProxyConfig| format!("{:?}{:?}{:?}", c.tcp_proxies, c.udp_proxies, c.ws_proxies);
        if listeners(&config) != listeners(&old.config) {
            warnings.push("[[tcp_proxy]] / [[udp_proxy]] / [[ws_proxy]] changes need a restart; the old listeners keep running".to_string());
        }

        let health_stop = Arc::new(AtomicBool::new(false));
        let routes = builder::compile_routes(&config, &health_stop);
        let route_count = routes.len();
        let generation = Generation::new(Arc::new(routes), old.dispatch.fallback.clone(), config, health_stop, Some(toml.to_string()));
        let replaced = {
            let mut slot = self.current.write().unwrap_or_else(|e| e.into_inner());
            std::mem::replace(&mut *slot, Arc::new(generation))
        };
        replaced.health_stop.store(true, Ordering::Relaxed);
        Ok(ReloadOutcome::Reloaded { routes: route_count, warnings })
    }

    /// Re-read the configuration file (`RWS_CONFIG_FILE` or
    /// `rws.config.toml`) and [`reload_from_str`](Self::reload_from_str) it,
    /// logging the outcome. Registered as a [`crate::config_reload`] hook by
    /// [`build_from_file`].
    pub fn reload_from_file(&self) {
        let path = config_file_path();
        let toml = match std::fs::read_to_string(&path) {
            Ok(toml) => toml,
            Err(e) => {
                eprintln!("[proxy_config] reload: cannot read {path}: {e} — keeping the running routes");
                crate::config_reload::report_error(format!("cannot read {path}: {e}"));
                return;
            }
        };
        match self.reload_from_str(&toml) {
            Ok(ReloadOutcome::Unchanged) => {}
            Ok(ReloadOutcome::Reloaded { routes, warnings }) => {
                for w in &warnings {
                    eprintln!("[proxy_config] Warning: {w}");
                }
                println!("[proxy_config] reloaded {path}: {routes} route{}", if routes == 1 { "" } else { "s" });
            }
            Err(report) => {
                for e in &report.errors {
                    eprintln!("[proxy_config] reload ERROR: {e}");
                    crate::config_reload::report_error(e.clone());
                }
                eprintln!("[proxy_config] {path} has errors — keeping the running routes");
            }
        }
    }
}

/// The request with its target normalized ([`normalize_request_target`]),
/// borrowed when it already is.
fn normalized(request: &Request) -> std::borrow::Cow<'_, Request> {
    match normalize_request_target(&request.request_uri) {
        std::borrow::Cow::Borrowed(_) => std::borrow::Cow::Borrowed(request),
        std::borrow::Cow::Owned(target) => {
            let mut owned = request.clone();
            owned.request_uri = target;
            std::borrow::Cow::Owned(owned)
        }
    }
}

/// A request target's path normalized before routes match it, and sent
/// upstream that way (#50): unreserved characters percent-decoded (`%61` →
/// `a`; other escapes kept, hex upper-cased), repeated slashes collapsed,
/// `.` and `..` segments resolved (never above `/`). Otherwise `/%61dmin`,
/// `//admin` or `/x/../admin` missed a protected `/admin` route and reached
/// the backend through a catch-all route. The query is kept as is; a
/// target that isn't a path (`*`) is returned unchanged.
pub(crate) fn normalize_request_target(target: &str) -> std::borrow::Cow<'_, str> {
    let (path, query) = match target.find('?') {
        Some(i) => (&target[..i], &target[i..]),
        None => (target, ""),
    };
    if !path.starts_with('/') || !(path.contains('%') || path.contains("//") || path.contains("/.")) {
        return std::borrow::Cow::Borrowed(target);
    }
    let bytes = path.as_bytes();
    let mut decoded = String::with_capacity(path.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                let value = (h * 16 + l) as u8;
                if value.is_ascii_alphanumeric() || matches!(value, b'-' | b'.' | b'_' | b'~') {
                    decoded.push(value as char);
                } else {
                    decoded.push('%');
                    decoded.push(bytes[i + 1].to_ascii_uppercase() as char);
                    decoded.push(bytes[i + 2].to_ascii_uppercase() as char);
                }
                i += 3;
                continue;
            }
        }
        // A `%` that starts no escape is a literal percent sign: encoded, so
        // what follows can't form a new escape with it (normalizing twice
        // must change nothing — the fuzzer found `%%611` → `%a1`).
        if bytes[i] == b'%' {
            decoded.push_str("%25");
            i += 1;
            continue;
        }
        // `path` is a &str: copy whole UTF-8 sequences.
        let ch = path[i..].chars().next().unwrap_or('\u{FFFD}');
        decoded.push(ch);
        i += ch.len_utf8().max(1);
    }
    let mut segments: Vec<&str> = Vec::new();
    let parts: Vec<&str> = decoded.split('/').collect();
    let last = parts.len().saturating_sub(1);
    let mut trailing_slash = false;
    for (n, segment) in parts.iter().enumerate() {
        match *segment {
            "" => trailing_slash = n == last,
            "." => trailing_slash = n == last,
            ".." => {
                segments.pop();
                trailing_slash = n == last;
            }
            s => {
                segments.push(s);
                trailing_slash = false;
            }
        }
    }
    let mut out = String::with_capacity(decoded.len() + query.len());
    out.push('/');
    out.push_str(&segments.join("/"));
    if trailing_slash && !segments.is_empty() {
        out.push('/');
    }
    out.push_str(query);
    if out == target {
        std::borrow::Cow::Borrowed(target)
    } else {
        std::borrow::Cow::Owned(out)
    }
}

/// A route's `host` against the request's (SNI or `Host`): ignoring ASCII
/// case, and ignoring a port the route doesn't name (#50).
fn host_matches(expected: &str, actual: &str) -> bool {
    let actual = if expected.contains(':') {
        actual
    } else if let Some(rest) = actual.strip_prefix('[') {
        // [IPv6]:port
        rest.split(']').next().unwrap_or(rest)
    } else {
        actual.rsplit_once(':').map_or(actual, |(host, port)| if port.bytes().all(|b| b.is_ascii_digit()) { host } else { actual })
    };
    let expected = expected.trim_start_matches('[').trim_end_matches(']');
    actual.eq_ignore_ascii_case(expected)
}

impl Application for ConfigDrivenApp {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        let request = normalized(request);
        let generation = self.current();
        match &generation.entry {
            Some(entry) => entry.execute(&request, conn),
            None => generation.dispatch.execute(&request, conn),
        }
    }

    /// The matched route decides: a `grpc` route streams request bodies
    /// (client-streaming and bidirectional calls), the others don't yet.
    fn streams_body(&self, request: &Request, conn: &ConnectionInfo) -> bool {
        let request = normalized(request);
        let generation = self.current();
        match &generation.entry {
            Some(entry) => entry.streams_body(&request, conn),
            None => generation.dispatch.streams_body(&request, conn),
        }
    }

    /// Routes proxy upstream or run configured middleware: only requests
    /// that match no route and fall through to the static files run inline,
    /// and only without a global `[middleware]` table.
    fn runs_inline(&self, request: &Request, conn: &ConnectionInfo) -> bool {
        let request = normalized(request);
        let generation = self.current();
        generation.entry.is_none()
            && !generation.dispatch.routes.iter().any(|r| r.matcher.matches(&request, conn))
            && App::is_inline_request(&request)
    }

    /// A request for a proxy route with a plain-HTTP upstream and no
    /// middleware (route or global) waits for the backend without holding
    /// a thread.
    #[cfg(feature = "http2")]
    fn execute_async<'a>(&'a self, request: &'a Request, conn: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        let generation = self.current();
        if generation.entry.is_some() {
            return None;
        }
        let request = normalized(request);
        let route = generation.dispatch.routes.iter().find(|r| r.matcher.matches(&request, conn))?;
        let proxy = Arc::clone(route.async_proxy.as_ref()?);
        Some(Box::pin(async move { proxy.execute_async(&request, conn).await }))
    }
}

// ── H2Route ───────────────────────────────────────────────────────────────────

/// The `grpc` action: requests forwarded over HTTP/2 to the upstream's live
/// backends in turn, bodies streamed both ways, trailers kept; gRPC-web
/// requests translated to gRPC on the way ([`GrpcWebLayer`](crate::proxy::GrpcWebLayer)).
#[cfg(feature = "http2")]
pub(crate) struct H2Route {
    live: Arc<RwLock<Vec<String>>>,
    tls: bool,
    trust: Option<Arc<crate::proxy::UpstreamTls>>,
    connect_timeout: std::time::Duration,
    read_timeout: std::time::Duration,
    counter: std::sync::atomic::AtomicUsize,
}

#[cfg(feature = "http2")]
impl H2Route {
    pub(crate) fn new(
        live: Arc<RwLock<Vec<String>>>,
        tls: bool,
        trust: Option<Arc<crate::proxy::UpstreamTls>>,
        connect_timeout_ms: u64,
        read_timeout_ms: u64,
    ) -> H2Route {
        H2Route {
            live,
            tls,
            trust,
            connect_timeout: std::time::Duration::from_millis(connect_timeout_ms),
            read_timeout: std::time::Duration::from_millis(read_timeout_ms),
            counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[cfg(feature = "http2")]
impl Application for H2Route {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        use crate::middleware::Middleware;
        crate::proxy::GrpcWebLayer::new().handle(request, conn, &H2Forward(self))
    }

    fn streams_body(&self, _request: &Request, _conn: &ConnectionInfo) -> bool {
        true
    }
}

/// [`H2Route`] after the gRPC-web translation.
#[cfg(feature = "http2")]
struct H2Forward<'a>(&'a H2Route);

#[cfg(feature = "http2")]
impl Application for H2Forward<'_> {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        let route = self.0;
        let backends = route.live.read().unwrap_or_else(|e| e.into_inner()).clone();
        if backends.is_empty() {
            return Ok(crate::proxy::bad_gateway());
        }
        // A streamed request body can be sent once: no second backend then.
        let attempts = if matches!(request.body, crate::request::Body::Stream(_)) { 1 } else { backends.len() };
        let start = route.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        for attempt in 0..attempts {
            let url = &backends[(start + attempt) % backends.len()];
            let result = crate::proxy::forward_h2(
                request, crate::proxy::Peer::of(conn), url, route.tls, route.trust.as_deref(), route.connect_timeout, route.read_timeout,
            );
            match result {
                Ok(response) => return Ok(response),
                Err(e) => eprintln!("grpc route: {e}"),
            }
        }
        Ok(crate::proxy::bad_gateway())
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
use std::sync::atomic::AtomicUsize;
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
/// of backends without restarting, and — for a declared `[[upstream]]` — its
/// [`upstream::UpstreamPolicy`]: weights, stickiness, the circuit breaker,
/// retries and metrics.
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
    /// The upstream's `tls_ca_file` trust settings; `None` = WebPKI roots.
    trust: Option<Arc<crate::proxy::UpstreamTls>>,
    /// The upstream's name, for metrics.
    upstream: Arc<String>,
    /// The declared upstream's policy; `None` for an inline backend.
    policy: Option<Arc<upstream::UpstreamPolicy>>,
    /// `[route.middleware.mirror]`: copies of requests, sent once per
    /// request (not per retry).
    mirror: Option<crate::proxy::mirror::Mirror>,
    /// `early_hints`: sent once per request, before the first attempt.
    early_hints: Option<Arc<Vec<String>>>,
    /// `protocol = "h3"`: requests go over HTTP/3; backends QUIC couldn't
    /// reach are on TCP until the time kept here.
    #[cfg(feature = "http3")]
    h3: Option<Arc<std::sync::Mutex<HashMap<String, std::time::Instant>>>>,
}

impl DynamicProxy {
    /// Trust the given CA for TLS backends instead of the WebPKI roots.
    pub(crate) fn with_trust(mut self, trust: Option<Arc<crate::proxy::UpstreamTls>>) -> Self {
        self.trust = trust;
        self
    }

    /// Apply the upstream's weights, stickiness, circuit breaker, retries
    /// and metrics.
    /// Send these `Link` values as a `103` for each request (once, not per
    /// retry).
    pub(crate) fn with_early_hints(mut self, links: &[String]) -> Self {
        self.early_hints = (!links.is_empty()).then(|| Arc::new(links.to_vec()));
        self
    }

    /// Send requests over HTTP/3 (`protocol = "h3"`); needs TLS backends
    /// and the `http3` feature (a no-op without it — `rws --check` reports
    /// that).
    #[cfg_attr(not(feature = "http3"), allow(unused_mut))]
    pub(crate) fn with_h3(mut self, enabled: bool) -> Self {
        #[cfg(feature = "http3")]
        {
            self.h3 = (enabled && self.tls).then(Default::default);
        }
        #[cfg(not(feature = "http3"))]
        let _ = enabled;
        self
    }

    /// The HTTP/3 target for `backend`, unless this proxy doesn't use
    /// HTTP/3 or the backend is on TCP for now.
    #[cfg(feature = "http3")]
    fn h3_target(&self, backend: &str) -> Option<Arc<rustls::ClientConfig>> {
        let tcp_until = self.h3.as_ref()?;
        let mut tcp_until = tcp_until.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(until) = tcp_until.get(backend) {
            if std::time::Instant::now() < *until {
                return None;
            }
            tcp_until.remove(backend);
        }
        Some(match &self.trust {
            Some(trust) => trust.client_config(),
            None => crate::proxy::tls_client_config(),
        })
    }

    /// QUIC couldn't reach `backend`: TCP for [`H3_FALLBACK`], counted.
    #[cfg(feature = "http3")]
    fn fall_back_to_tcp(&self, backend: &str, error: &str) {
        if let Some(tcp_until) = &self.h3 {
            tcp_until.lock().unwrap_or_else(|e| e.into_inner()).insert(backend.to_string(), std::time::Instant::now() + H3_FALLBACK);
        }
        upstream::observe_h3_fallback(&self.upstream);
        eprintln!("upstream {}: HTTP/3 to {backend} failed ({error}); using TCP for {} s", self.upstream, H3_FALLBACK.as_secs());
    }

    /// Send a copy of each request to `mirror` (once, not per retry).
    pub(crate) fn with_mirror(mut self, mirror: Option<crate::proxy::mirror::Mirror>) -> Self {
        self.mirror = mirror;
        self
    }

    pub(crate) fn with_policy(mut self, upstream: &str, policy: Option<Arc<upstream::UpstreamPolicy>>) -> Self {
        self.upstream = Arc::new(upstream.to_string());
        self.policy = policy;
        self
    }

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
            trust: None,
            upstream: Arc::new(String::new()),
            policy: None,
            mirror: None,
            early_hints: None,
            #[cfg(feature = "http3")]
            h3: None,
        }
    }

    /// Pick by the strategy among `candidates` (not empty).
    fn pick(&self, candidates: &[String], client_ip: &str) -> String {
        let policy = self.policy.as_deref().filter(|p| p.weighted());
        let idx = match self.strategy {
            LoadBalanceStrategy::RoundRobin => match policy {
                Some(policy) => return policy.weighted_round_robin(candidates).unwrap_or_else(|| candidates[0].clone()),
                None => self.counter.fetch_add(1, Ordering::Relaxed) % candidates.len(),
            },
            LoadBalanceStrategy::Random => {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0) as usize;
                let salt = self.counter.fetch_add(1, Ordering::Relaxed);
                let roll = nanos.wrapping_add(salt.wrapping_mul(2_654_435_761));
                match policy {
                    Some(policy) => policy.weighted_index(candidates, roll),
                    None => roll % candidates.len(),
                }
            }
            LoadBalanceStrategy::IpHash => {
                let mut hasher = DefaultHasher::new();
                client_ip.hash(&mut hasher);
                (hasher.finish() as usize) % candidates.len()
            }
            LoadBalanceStrategy::LeastConnections => {
                let connections = self.connections.read().unwrap();
                let load = |backend: &String| connections.get(backend).map(|c| c.load(Ordering::Relaxed)).unwrap_or(0) as u64;
                let weight = |backend: &String| self.policy.as_deref().map_or(1, |p| p.weight(backend)) as u64;
                // Fewest in flight per unit of weight (compared crosswise).
                candidates
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| (load(a) * weight(b)).cmp(&(load(b) * weight(a))))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        };
        candidates[idx].clone()
    }

    /// The backend for this attempt: the sticky one if it's still in
    /// rotation, else one by the strategy, skipping `tried` and backends
    /// whose circuit breaker is open. `Err(true)`: none, because the breaker
    /// holds the rest open; `Err(false)`: none at all.
    fn next_backend(&self, request: &Request, client_ip: &str, tried: &[String]) -> Result<String, bool> {
        let mut candidates: Vec<String> = self.live.read().unwrap().iter().filter(|b| !tried.contains(b)).cloned().collect();
        if !candidates.is_empty() {
            candidates.retain(|b| !upstream::is_drained(&self.upstream, b));
            if candidates.is_empty() {
                // Every live backend drained: 503, like an open breaker.
                return Err(true);
            }
        }
        let Some(policy) = self.policy.as_deref() else {
            return if candidates.is_empty() { Err(false) } else { Ok(self.pick(&candidates, client_ip)) };
        };
        if let Some(sticky) = &policy.sticky {
            if let Some(id) = upstream::cookie_value(request, &sticky.cookie) {
                if let Some(backend) = candidates.iter().find(|b| upstream::backend_id(b) == id).cloned() {
                    if policy.admits(&backend) {
                        return Ok(backend);
                    }
                    candidates.retain(|b| *b != backend);
                }
            }
        }
        let ejected = policy.any_open(&candidates);
        while !candidates.is_empty() {
            let backend = self.pick(&candidates, client_ip);
            if policy.admits(&backend) {
                return Ok(backend);
            }
            candidates.retain(|b| *b != backend);
        }
        Err(ejected)
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

/// A request ready to send: the chosen backend and the (possibly rewritten)
/// request. Holding it keeps the backend's least-connections count raised.
struct Prepared<'a> {
    backend: String,
    host: String,
    port: u16,
    request: std::borrow::Cow<'a, Request>,
    _guard: Option<ConnectionGuard>,
}

/// How one attempt went.
enum Attempt {
    /// A response to pass on.
    Done(Response),
    /// A failure, retried when the policy allows: the backend's response (a
    /// retried status) or none (no response).
    Failed(Option<Response>),
}

impl DynamicProxy {
    /// Pick a backend (not one of `tried`) and apply the path rewriting;
    /// `Err` is the response to send instead (no backend in rotation — `503`
    /// when the circuit breaker took them out, `502` otherwise — or an
    /// unparsable backend address).
    fn prepare<'a>(&self, request: &'a Request, conn: &ConnectionInfo, tried: &[String]) -> Result<Prepared<'a>, Response> {
        let backend = self.next_backend(request, &conn.client.ip, tried).map_err(|ejected| if ejected { service_unavailable() } else { bad_gateway() })?;

        let guard = if self.strategy == LoadBalanceStrategy::LeastConnections {
            let counter = self.connection_counter(&backend);
            counter.fetch_add(1, Ordering::Relaxed);
            Some(ConnectionGuard { counter })
        } else {
            None
        };

        let (host, port, _) = crate::proxy_config::health::parse_backend_url(&backend).ok_or_else(bad_gateway)?;

        // Apply path rewriting if configured
        let request = if self.strip_prefix.is_some() || self.add_prefix.is_some() {
            let mut req_clone = request.clone();
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
            std::borrow::Cow::Owned(req_clone)
        } else {
            std::borrow::Cow::Borrowed(request)
        };

        Ok(Prepared { backend, host, port, request, _guard: guard })
    }

    /// Record an attempt's outcome (breaker, metrics) and classify it.
    fn settle(&self, backend: &str, result: Result<Response, ()>, started: std::time::Instant) -> Attempt {
        let status = result.as_ref().ok().map(|r| r.status_code);
        upstream::observe(&self.upstream, backend, status, started);
        let failed = status.is_none_or(|s| matches!(s, 502..=504));
        if let Some(policy) = &self.policy {
            policy.record(backend, failed);
        }
        match result {
            Ok(response) if self.policy.as_deref().is_some_and(|p| p.retries_status(response.status_code)) => Attempt::Failed(Some(response)),
            Ok(response) => Attempt::Done(response),
            Err(()) => Attempt::Failed(None),
        }
    }

    /// How many attempts `request` may get.
    fn attempts(&self, request: &Request) -> u32 {
        match self.policy.as_deref().and_then(|p| p.retry.as_ref()) {
            Some(retry) if upstream::replayable(request) => 1 + retry.attempts,
            _ => 1,
        }
    }

    /// Set the sticky cookie when the client doesn't hold this backend's.
    fn stick(&self, request: &Request, backend: &str, response: &mut Response) {
        let Some(sticky) = self.policy.as_deref().and_then(|p| p.sticky.as_ref()) else { return };
        let id = upstream::backend_id(backend);
        if upstream::cookie_value(request, &sticky.cookie).as_deref() == Some(id.as_str()) {
            return;
        }
        let max_age = sticky.max_age_secs.map(|s| format!("; Max-Age={s}")).unwrap_or_default();
        response.headers.push(crate::header::Header {
            name: "Set-Cookie".into(),
            value: format!("{}={id}; Path=/; HttpOnly; SameSite=Lax{max_age}", sticky.cookie).into(),
        });
    }

    /// The same as [`Application::execute`], without blocking a thread while
    /// the backend answers, for HTTP and HTTPS backends alike.
    #[cfg(feature = "http2")]
    pub(crate) async fn execute_async(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        if let Some(mirror) = &self.mirror {
            mirror.send(request, conn);
        }
        if let Some(links) = &self.early_hints {
            crate::early_hints::send_from(crate::early_hints::Source::Configured, links);
        }
        let mut tried: Vec<String> = Vec::new();
        let mut last: Option<Response> = None;
        for attempt in 0..self.attempts(request) {
            let prepared = match self.prepare(request, conn, &tried) {
                Ok(p) => p,
                Err(response) => return Ok(last.unwrap_or(response)),
            };
            if attempt > 0 {
                upstream::observe_retry(&self.upstream);
            }
            let tls = self.tls.then(|| match &self.trust {
                Some(trust) => trust.async_tls(),
                None => crate::proxy::async_upstream::AsyncTls {
                    config: crate::proxy::tls_client_config(),
                    pool: crate::proxy::async_upstream::default_tls_pool(),
                },
            });
            let started = std::time::Instant::now();
            #[cfg(feature = "http3")]
            if let Some(config) = self.h3_target(&prepared.backend) {
                let target = crate::proxy::h3_upstream::Target {
                    host: &prepared.host,
                    port: prepared.port,
                    tls: &config,
                    connect_timeout: self.connect_timeout,
                    read_timeout: self.read_timeout,
                };
                match crate::proxy::h3_upstream::forward_async(&prepared.request, crate::proxy::Peer::of(conn), &target).await {
                    Err(crate::proxy::h3_upstream::H3Error::Connect(e)) => self.fall_back_to_tcp(&prepared.backend, &e),
                    result => {
                        match self.settle(&prepared.backend, result.map_err(|_| ()), started) {
                            Attempt::Done(mut response) => {
                                self.stick(request, &prepared.backend, &mut response);
                                return Ok(response);
                            }
                            Attempt::Failed(response) => last = response,
                        }
                        tried.push(prepared.backend.clone());
                        continue;
                    }
                }
            }
            let result = crate::proxy::async_upstream::forward(
                &prepared.request,
                crate::proxy::Peer::of(conn),
                &prepared.host,
                prepared.port,
                tls,
                self.connect_timeout,
                self.read_timeout,
            )
            .await
            .map_err(|_| ());
            match self.settle(&prepared.backend, result, started) {
                Attempt::Done(mut response) => {
                    self.stick(request, &prepared.backend, &mut response);
                    return Ok(response);
                }
                Attempt::Failed(response) => last = response,
            }
            tried.push(prepared.backend.clone());
        }
        Ok(last.unwrap_or_else(bad_gateway))
    }

    /// Whether [`execute_async`](Self::execute_async) can serve this proxy.
    pub(crate) fn supports_async(&self) -> bool {
        cfg!(feature = "http2")
    }
}

impl Application for DynamicProxy {
    fn execute(&self, request: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        if let Some(mirror) = &self.mirror {
            mirror.send(request, conn);
        }
        if let Some(links) = &self.early_hints {
            crate::early_hints::send_from(crate::early_hints::Source::Configured, links);
        }
        let mut tried: Vec<String> = Vec::new();
        let mut last: Option<Response> = None;
        for attempt in 0..self.attempts(request) {
            let prepared = match self.prepare(request, conn, &tried) {
                Ok(p) => p,
                Err(response) => return Ok(last.unwrap_or(response)),
            };
            if attempt > 0 {
                upstream::observe_retry(&self.upstream);
            }
            // Pooled keep-alive connections to the backend, TLS or not (a
            // fresh connection per request halved throughput vs. Caddy/nginx).
            let started = std::time::Instant::now();
            #[cfg(feature = "http3")]
            if let Some(config) = self.h3_target(&prepared.backend) {
                let target = crate::proxy::h3_upstream::Target {
                    host: &prepared.host,
                    port: prepared.port,
                    tls: &config,
                    connect_timeout: self.connect_timeout,
                    read_timeout: self.read_timeout,
                };
                match crate::proxy::h3_upstream::forward(&prepared.request, crate::proxy::Peer::of(conn), &target) {
                    Err(crate::proxy::h3_upstream::H3Error::Connect(e)) => self.fall_back_to_tcp(&prepared.backend, &e),
                    result => {
                        match self.settle(&prepared.backend, result.map_err(|_| ()), started) {
                            Attempt::Done(mut response) => {
                                self.stick(request, &prepared.backend, &mut response);
                                return Ok(response);
                            }
                            Attempt::Failed(response) => last = response,
                        }
                        tried.push(prepared.backend.clone());
                        continue;
                    }
                }
            }
            let result = crate::proxy::forward_upstream_trusting(
                crate::proxy::shared_pool(),
                &prepared.request,
                crate::proxy::Peer::of(conn),
                &prepared.host,
                prepared.port,
                self.tls,
                self.trust.as_deref(),
                self.connect_timeout,
                self.read_timeout,
            )
            .map_err(|_| ());
            match self.settle(&prepared.backend, result, started) {
                Attempt::Done(mut response) => {
                    self.stick(request, &prepared.backend, &mut response);
                    return Ok(response);
                }
                Attempt::Failed(response) => last = response,
            }
            tried.push(prepared.backend.clone());
        }
        Ok(last.unwrap_or_else(bad_gateway))
    }
}

/// How long a backend QUIC couldn't reach is sent requests over TCP.
#[cfg(feature = "http3")]
pub(crate) const H3_FALLBACK: Duration = Duration::from_secs(300);

fn service_unavailable() -> Response {
    use crate::mime_type::MimeType;
    use crate::range::Range;
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n503_service_unavailable.reason_phrase.to_string();
    r.headers.push(crate::header::Header { name: "Retry-After".into(), value: "5".into() });
    r.content_range_list = vec![Range::get_content_range(b"503 Service Unavailable: every backend is out of rotation".to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
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
        r.headers.push(Header { name: "Location".into(), value: location.into() });
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
            let mut response = AppError::TooManyRequests.into_response();
            let retry_after = self.0.retry_after(&conn.client.ip);
            response.headers.push(Header {
                name: Header::_RETRY_AFTER.into(),
                value: retry_after.to_string().into(),
            });
            Ok(response)
        }
    }
}

// ── PerRouteMaxBodySize ───────────────────────────────────────────────────────

/// A route-specific request body size limit, from `[route.middleware]
/// max_body_size`.
///
/// Rejects with `413 Payload Too Large` if `request.body` is bigger than the
/// configured ceiling. This runs *after* the request has already been fully
/// read off the socket — route middleware only ever sees an already-built
/// `Request` — so, unlike the global `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`
/// (checked against the declared `Content-Length` before any of an
/// oversized body is buffered), this can't prevent the memory from being
/// spent for a single oversized request. What it protects is this route's
/// own downstream logic/upstream backend from a body that's within the
/// global limit but still too big for this specific route.
pub(crate) struct PerRouteMaxBodySize(pub(crate) u64);

impl crate::middleware::Middleware for PerRouteMaxBodySize {
    fn handle(
        &self,
        request: &Request,
        conn: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        use crate::error::{AppError, IntoResponse};
        if request.body.len() as u64 > self.0 {
            Ok(AppError::PayloadTooLarge(format!(
                "request body of {} bytes exceeds this route's {}-byte limit",
                request.body.len(),
                self.0,
            ))
            .into_response())
        } else {
            next.execute(request, conn)
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
            .any(|h| h.name.eq_ignore_ascii_case("authorization") && constant_time_eq(h.value.as_bytes(), expected.as_bytes()));
        if authorized {
            next.execute(request, conn)
        } else {
            Ok(AppError::Unauthorized.into_response())
        }
    }
}

/// Compares in time independent of where the inputs first differ, so response
/// timing doesn't reveal how much of a guessed token was right.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Stands in for an auth layer whose configuration can't be honoured (see
/// `builder::apply_middleware`): every request gets `503`, never the route.
pub(crate) struct AuthMisconfigured;

/// A disk cache's `max_size`, or the default.
pub(crate) fn cache_max_bytes(max_size: &str) -> Option<u64> {
    if max_size.trim().is_empty() {
        Some(256 * 1024 * 1024)
    } else {
        crate::log::access_sink::parse_size(max_size.trim()).filter(|n| *n > 0)
    }
}

/// A `[route.middleware.wasm]` plugin that couldn't be loaded: the route
/// fails closed.
pub(crate) struct PluginMisconfigured;

impl crate::middleware::Middleware for PluginMisconfigured {
    fn handle(&self, _request: &Request, _conn: &ConnectionInfo, _next: &dyn Application) -> Result<Response, String> {
        Ok(Response::text(
            STATUS_CODE_REASON_PHRASE.n503_service_unavailable,
            "503 Service Unavailable: this route's plugin is misconfigured",
        ))
    }
}

impl crate::middleware::Middleware for AuthMisconfigured {
    fn handle(
        &self,
        _request: &Request,
        _conn: &ConnectionInfo,
        _next: &dyn Application,
    ) -> Result<Response, String> {
        Ok(Response::text(
            STATUS_CODE_REASON_PHRASE.n503_service_unavailable,
            "503 Service Unavailable: authentication for this route is misconfigured",
        ))
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
