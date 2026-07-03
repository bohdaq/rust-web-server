//! Builder: converts a `ProxyConfig` into a live `ConfigDrivenApp` with
//! health checkers and L4/WS proxy threads.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::middleware::WithMiddleware;
use crate::proxy_config::{
    ActionConfig, AuthConfig, CompiledRoute, ConfigDrivenApp, DynamicProxy, MiddlewareConfig,
    ProxyConfig, RedirectAdapter, RespondAdapter, RouteMatcher, StaticAdapter, arc_app,
    BearerAuthMiddleware, PerRouteRateLimit,
};

/// Build a `ConfigDrivenApp` from `rws.config.toml` and spawn L4/WS proxy
/// threads. Returns the app and a list of background thread handles.
pub fn build_from_file() -> (ConfigDrivenApp, Vec<std::thread::JoinHandle<()>>) {
    let config = ProxyConfig::load();
    build(config)
}

/// Build a `ConfigDrivenApp` from a `ProxyConfig` struct.
pub fn build(config: ProxyConfig) -> (ConfigDrivenApp, Vec<std::thread::JoinHandle<()>>) {
    // ── 1. Build upstream name → live backends map ─────────────────────────
    let mut upstream_lives: HashMap<String, Arc<RwLock<Vec<String>>>> = HashMap::new();

    for upstream in &config.upstreams {
        let live = Arc::new(RwLock::new(upstream.backends.clone()));
        upstream_lives.insert(upstream.name.clone(), Arc::clone(&live));

        if let Some(ref hc) = upstream.health_check {
            crate::proxy_config::health::start_health_checker(
                upstream.name.clone(),
                upstream.backends.clone(),
                Arc::clone(&live),
                hc.clone(),
            );
        }
    }

    // ── 2. Build compiled routes ───────────────────────────────────────────
    let mut compiled: Vec<CompiledRoute> = Vec::new();

    for route in &config.routes {
        let matcher = RouteMatcher::from_match_config(&route.match_);

        // Build the action handler (base application)
        let base_handler: Arc<dyn crate::application::Application + Send + Sync> =
            match &route.action {
                ActionConfig::Proxy {
                    upstream,
                    connect_timeout_ms,
                    read_timeout_ms,
                    strip_path_prefix,
                    add_path_prefix,
                } => {
                    let live = upstream_lives
                        .get(upstream.as_str())
                        .cloned()
                        .unwrap_or_else(|| {
                            // Fallback: upstream not declared, use name as single backend
                            Arc::new(RwLock::new(vec![upstream.clone()]))
                        });
                    let upstream_cfg = config.upstreams.iter().find(|u| u.name == *upstream);
                    let upstream_tls = upstream_cfg.map(|u| u.tls).unwrap_or(false);
                    let upstream_strategy = upstream_cfg
                        .map(|u| u.strategy.clone())
                        .unwrap_or_else(|| "round_robin".to_string());
                    arc_app(DynamicProxy::new(
                        live,
                        *connect_timeout_ms,
                        *read_timeout_ms,
                        strip_path_prefix.clone(),
                        add_path_prefix.clone(),
                        upstream_tls,
                        upstream_strategy,
                    ))
                }

                ActionConfig::Redirect { location, status } => {
                    arc_app(RedirectAdapter::new(location.clone(), *status))
                }

                ActionConfig::Respond { status, body, content_type } => {
                    arc_app(RespondAdapter::new(*status, body.clone(), content_type.clone()))
                }

                ActionConfig::Static { root, index } => {
                    arc_app(StaticAdapter::new(root.clone(), index.clone()))
                }

                // Grpc action — use DynamicProxy (it forwards over HTTP/1.1;
                // a future version could plug in H2ReverseProxy here)
                ActionConfig::Grpc { upstream, connect_timeout_ms, read_timeout_ms } => {
                    let live = upstream_lives
                        .get(upstream.as_str())
                        .cloned()
                        .unwrap_or_else(|| Arc::new(RwLock::new(vec![upstream.clone()])));
                    let upstream_cfg = config.upstreams.iter().find(|u| u.name == *upstream);
                    let upstream_tls = upstream_cfg.map(|u| u.tls).unwrap_or(false);
                    let upstream_strategy = upstream_cfg
                        .map(|u| u.strategy.clone())
                        .unwrap_or_else(|| "round_robin".to_string());
                    arc_app(DynamicProxy::new(live, *connect_timeout_ms, *read_timeout_ms, None, None, upstream_tls, upstream_strategy))
                }

                ActionConfig::Mcp | ActionConfig::Unknown(_) => {
                    // No-op: these fall through to fallback
                    arc_app(crate::proxy_config::NullApp)
                }
            };

        // Wrap the base handler with middleware layers
        let handler = apply_middleware(base_handler, &route.middleware);

        compiled.push(CompiledRoute { matcher, handler });
    }

    // ── 3. Spawn L4/WS proxy threads ──────────────────────────────────────
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    for tcp_cfg in &config.tcp_proxies {
        let listen = tcp_cfg.listen.clone();
        let backends = tcp_cfg.backends.clone();
        let timeout_ms = tcp_cfg.connect_timeout_ms;
        let name = tcp_cfg.name.clone();
        let h = std::thread::Builder::new()
            .name(format!("tcp-proxy-{}", name))
            .spawn(move || {
                let proxy = crate::tcp_proxy::TcpProxy::new(backends)
                    .connect_timeout_ms(timeout_ms);
                if let Err(e) = proxy.bind(&listen) {
                    eprintln!("[tcp_proxy:{}] {}", name, e);
                }
            })
            .expect("failed to spawn tcp proxy thread");
        handles.push(h);
    }

    for udp_cfg in &config.udp_proxies {
        let listen = udp_cfg.listen.clone();
        let backends = udp_cfg.backends.clone();
        let reply_timeout_ms = udp_cfg.reply_timeout_ms;
        let buffer_size = udp_cfg.buffer_size;
        let name = udp_cfg.name.clone();
        let h = std::thread::Builder::new()
            .name(format!("udp-proxy-{}", name))
            .spawn(move || {
                let proxy = crate::udp_proxy::UdpProxy::new(backends)
                    .reply_timeout_ms(reply_timeout_ms)
                    .buffer_size(buffer_size);
                if let Err(e) = proxy.bind(&listen) {
                    eprintln!("[udp_proxy:{}] {}", name, e);
                }
            })
            .expect("failed to spawn udp proxy thread");
        handles.push(h);
    }

    for ws_cfg in &config.ws_proxies {
        let listen = ws_cfg.listen.clone();
        let backends = ws_cfg.backends.clone();
        let connect_timeout_ms = ws_cfg.connect_timeout_ms;
        let read_timeout_ms = ws_cfg.read_timeout_ms;
        let name = ws_cfg.name.clone();
        let h = std::thread::Builder::new()
            .name(format!("ws-proxy-{}", name))
            .spawn(move || {
                let proxy = crate::ws_proxy::WsProxy::new(backends)
                    .connect_timeout_ms(connect_timeout_ms)
                    .read_timeout_ms(read_timeout_ms);
                if let Err(e) = proxy.bind(&listen) {
                    eprintln!("[ws_proxy:{}] {}", name, e);
                }
            })
            .expect("failed to spawn ws proxy thread");
        handles.push(h);
    }

    (ConfigDrivenApp::new(compiled), handles)
}

/// Wrap `handler` with the middleware layers described by `mw`.
/// Layers are applied innermost → outermost in the order:
///   1. IP filter (outermost — rejects early)
///   2. Rate limit
///   3. Auth
///   4. Rewrite (request + response rules combined)
///   5. Cache (innermost middleware — closest to handler)
fn apply_middleware(
    handler: Arc<dyn crate::application::Application + Send + Sync>,
    mw: &MiddlewareConfig,
) -> Arc<dyn crate::application::Application + Send + Sync> {
    // Wrap the Arc<dyn Application> in an ArcApp adapter so it implements
    // Application itself (needed for WithMiddleware::new).
    let mut app: Arc<dyn crate::application::Application + Send + Sync> = handler;

    // ── Cache (applied first so it's the innermost middleware) ─────────────
    if let Some(ref cache_cfg) = mw.cache {
        let mut layer = crate::cache::CacheLayer::memory(1000).ttl(cache_cfg.ttl_secs);
        for vh in &cache_cfg.vary_by {
            layer = layer.vary_by_header(vh.as_str());
        }
        app = arc_app(WithMiddleware::new(ArcApp(Arc::clone(&app))).wrap(layer));
    }

    // ── Rewrite ────────────────────────────────────────────────────────────
    if !mw.rewrite_request.is_empty() || !mw.rewrite_response.is_empty() {
        let mut layer = crate::rewrite::RewriteLayer::new();
        for rule in &mw.rewrite_request {
            layer = apply_request_rewrite_rule(layer, rule);
        }
        for rule in &mw.rewrite_response {
            layer = apply_response_rewrite_rule(layer, rule);
        }
        app = arc_app(WithMiddleware::new(ArcApp(Arc::clone(&app))).wrap(layer));
    }

    // ── Auth ───────────────────────────────────────────────────────────────
    if let Some(ref auth_cfg) = mw.auth {
        match auth_cfg {
            AuthConfig::Bearer { token_env } => {
                let token = std::env::var(token_env).unwrap_or_default();
                if !token.is_empty() {
                    app = arc_app(
                        WithMiddleware::new(ArcApp(Arc::clone(&app)))
                            .wrap(BearerAuthMiddleware { token: Arc::new(token) }),
                    );
                }
            }
            #[cfg(feature = "auth")]
            AuthConfig::Jwt { secret_env } => {
                let _secret = std::env::var(secret_env).unwrap_or_default();
                // JWT verification would be wired in here when auth feature is enabled.
                // For now this is a no-op placeholder.
            }
            #[cfg(not(feature = "auth"))]
            AuthConfig::Jwt { .. } => {
                eprintln!("[proxy_config] JWT auth requires the 'auth' feature; skipping.");
            }
            AuthConfig::Basic { .. } => {
                eprintln!("[proxy_config] Basic auth is not yet implemented; skipping.");
            }
        }
    }

    // ── Rate limit ─────────────────────────────────────────────────────────
    if let Some(ref rl_cfg) = mw.rate_limit {
        let limiter = Arc::new(crate::rate_limit::RateLimiter::new(
            rl_cfg.max_requests,
            rl_cfg.window_secs,
        ));
        app = arc_app(
            WithMiddleware::new(ArcApp(Arc::clone(&app)))
                .wrap(PerRouteRateLimit(limiter)),
        );
    }

    // ── IP filter (outermost) ──────────────────────────────────────────────
    if !mw.ip_allow.is_empty() {
        let filter = crate::ip_filter::IpFilter::allow(mw.ip_allow.iter().map(|s| s.as_str()));
        app = arc_app(WithMiddleware::new(ArcApp(Arc::clone(&app))).wrap(filter));
    } else if !mw.ip_deny.is_empty() {
        let filter = crate::ip_filter::IpFilter::deny(mw.ip_deny.iter().map(|s| s.as_str()));
        app = arc_app(WithMiddleware::new(ArcApp(Arc::clone(&app))).wrap(filter));
    }

    app
}

fn apply_request_rewrite_rule(
    layer: crate::rewrite::RewriteLayer,
    rule: &crate::proxy_config::RewriteRuleConfig,
) -> crate::rewrite::RewriteLayer {
    match rule.type_.as_str() {
        "header_set" => {
            if let (Some(name), Some(value)) = (&rule.name, &rule.value) {
                return layer.request_header_set(name, value);
            }
        }
        "header_remove" => {
            if let Some(name) = &rule.name {
                return layer.request_header_remove(name);
            }
        }
        "uri_set" => {
            if let Some(value) = &rule.value {
                return layer.request_uri_set(value);
            }
        }
        "uri_strip_prefix" | "strip_prefix" => {
            if let Some(prefix) = rule.prefix.as_ref().or(rule.value.as_ref()) {
                return layer.request_uri_strip_prefix(prefix);
            }
        }
        "uri_add_prefix" | "add_prefix" => {
            if let Some(prefix) = rule.prefix.as_ref().or(rule.value.as_ref()) {
                return layer.request_uri_add_prefix(prefix);
            }
        }
        _ => {}
    }
    layer
}

fn apply_response_rewrite_rule(
    layer: crate::rewrite::RewriteLayer,
    rule: &crate::proxy_config::RewriteRuleConfig,
) -> crate::rewrite::RewriteLayer {
    match rule.type_.as_str() {
        "header_set" => {
            if let (Some(name), Some(value)) = (&rule.name, &rule.value) {
                return layer.response_header_set(name, value);
            }
        }
        "header_remove" => {
            if let Some(name) = &rule.name {
                return layer.response_header_remove(name);
            }
        }
        "status" => {
            if let (Some(code), Some(reason)) = (&rule.code, &rule.reason) {
                return layer.response_status(*code as i16, reason);
            }
        }
        "body_replace" => {
            if let (Some(from), Some(to)) = (&rule.from, &rule.to) {
                return layer.response_body_replace(from, to);
            }
        }
        _ => {}
    }
    layer
}

// ── ArcApp adapter ─────────────────────────────────────────────────────────────

/// Wraps `Arc<dyn Application + Send + Sync>` to implement `Application`
/// (needed because you can't implement a foreign trait on a foreign type).
struct ArcApp(Arc<dyn crate::application::Application + Send + Sync>);

impl crate::application::Application for ArcApp {
    fn execute(
        &self,
        request: &crate::request::Request,
        conn: &crate::server::ConnectionInfo,
    ) -> Result<crate::response::Response, String> {
        self.0.execute(request, conn)
    }
}

impl Clone for ArcApp {
    fn clone(&self) -> Self {
        ArcApp(Arc::clone(&self.0))
    }
}
