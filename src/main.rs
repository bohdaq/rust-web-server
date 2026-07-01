use rust_web_server::app::App;
use rust_web_server::blocklist::{self, BlocklistLayer};
use rust_web_server::cache::CacheLayer;
use rust_web_server::core::New;
use rust_web_server::feature;
use rust_web_server::maintenance::{MaintenanceLayer, MAINTENANCE_MODE};
use rust_web_server::mcp::{McpContent, extract_arg};
use rust_web_server::metrics::SERVER_READY;
use rust_web_server::request::Request;
use rust_web_server::request_log::{self, LogLayer};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::routes;
use rust_web_server::server::{ConnectionInfo, Server};
use std::sync::atomic::Ordering;

// ── shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    version: &'static str,
}

// ── HTTP route handlers ───────────────────────────────────────────────────────

fn get_version(_req: &Request, _p: &PathParams, _c: &ConnectionInfo, state: &AppState) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![rust_web_server::range::Range::get_content_range(
        format!(r#"{{"version":"{}"}}"#, state.version).into_bytes(),
        "application/json".to_string(),
    )];
    r
}

fn echo_post(req: &Request, _p: &PathParams, _c: &ConnectionInfo, _s: &AppState) -> Response {
    let body = req.body.clone();
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![rust_web_server::range::Range::get_content_range(
        body,
        "application/json".to_string(),
    )];
    r
}

// ── app builder ───────────────────────────────────────────────────────────────
//
// Dispatch chain (outermost → innermost):
//
//  McpServer                 — POST /mcp  →  JSON-RPC (tools/list, tools/call …)
//    └─ AppWithState<S>      — custom routes registered below via routes!
//         └─ App (built-in)  — IndexController, HealthController (/healthz),
//                              ReadyController (/readyz), MetricsController,
//                              StaticResourceController, NotFoundController (404)
//
// Nothing below needs to change to keep the built-in controllers active;
// they run automatically for any request that neither McpServer nor
// AppWithState match.

fn build_app() -> impl rust_web_server::application::Application + Send + Clone + 'static {
    // 1. HTTP routes with shared state.
    let http = routes! {
        App::with_state(AppState { version: env!("CARGO_PKG_VERSION") }),
        GET  "/api/version" => get_version,
        POST "/api/echo"    => echo_post,
    };

    // Snapshot registered routes before mcp() consumes `http`.
    let registered_routes: Vec<(String, String)> = http.route_entries()
        .into_iter()
        .map(|r| (r.method, r.pattern))
        .collect();

    // 2. Shared cache — two clones kept so MCP tools can read stats and clear.
    //    All three instances share the same Arc<Mutex<CacheStore>> underneath.
    let cache = CacheLayer::memory(500).ttl(60);
    let cache2 = cache.clone(); // captured by cache_stats tool
    let cache3 = cache.clone(); // captured by cache_clear tool

    // 3. Middleware stack (innermost → outermost):
    //    routes → log → blocklist → maintenance → cache → MCP
    let app = http
        .wrap(LogLayer)
        .wrap(BlocklistLayer)
        .wrap(MaintenanceLayer)
        .wrap(cache);

    // 4. MCP server — POST /mcp; everything else falls through to `app`.
    let mut mcp = app.mcp("rws", env!("CARGO_PKG_VERSION"));
    if let Ok(token) = std::env::var("MCP_TOKEN") {
        mcp = mcp.require_bearer(token);
    }

    mcp
        // ── server introspection ─────────────────────────────────────────────
        .tool(
            "server_config",
            "Active RWS_CONFIG_* environment variables and their current values",
            r#"{"type":"object"}"#,
            |_| {
                use rust_web_server::entry_point::Config;
                let keys = [
                    Config::RWS_CONFIG_IP,
                    Config::RWS_CONFIG_PORT,
                    Config::RWS_CONFIG_THREAD_COUNT,
                    Config::RWS_CONFIG_LOG_FORMAT,
                    Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES,
                    Config::RWS_CONFIG_TLS_CERT_FILE,
                    Config::RWS_CONFIG_TLS_KEY_FILE,
                    Config::RWS_CONFIG_HTTP_REDIRECT_PORT,
                    Config::RWS_CONFIG_CORS_ALLOW_ALL,
                    Config::RWS_CONFIG_CORS_ALLOW_ORIGINS,
                    Config::RWS_CONFIG_CORS_ALLOW_METHODS,
                    Config::RWS_CONFIG_CORS_ALLOW_HEADERS,
                    Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS,
                    Config::RWS_CONFIG_CORS_EXPOSE_HEADERS,
                    Config::RWS_CONFIG_CORS_MAX_AGE,
                ];
                let pairs: Vec<String> = keys.iter().map(|k| {
                    let v = std::env::var(k).unwrap_or_default();
                    format!(r#""{}":"{}""#, k, v.replace('\\', "\\\\").replace('"', "\\\""))
                }).collect();
                Ok(McpContent::json(format!("{{{}}}", pairs.join(","))))
            },
        )
        .tool(
            "feature_flags",
            "Compile-time feature flags active in this binary",
            r#"{"type":"object"}"#,
            |_| {
                Ok(McpContent::json(format!(
                    r#"{{"http1":{},"http2":{},"http3":{},"acme":{},"auth":{},"serde":{},"macros":{}}}"#,
                    cfg!(feature = "http1"),
                    cfg!(feature = "http2"),
                    cfg!(feature = "http3"),
                    cfg!(feature = "acme"),
                    cfg!(feature = "auth"),
                    cfg!(feature = "serde"),
                    cfg!(feature = "macros"),
                )))
            },
        )
        .tool(
            "server_metrics",
            "Live server counters: requests, errors, active connections, thread pool queue depth",
            r#"{"type":"object"}"#,
            |_| {
                use rust_web_server::metrics;
                Ok(McpContent::json(format!(
                    r#"{{"requests_total":{},"errors_total":{},"active_connections":{},"thread_pool_queued":{},"ready":{}}}"#,
                    metrics::REQUESTS_TOTAL.load(Ordering::Relaxed),
                    metrics::ERRORS_TOTAL.load(Ordering::Relaxed),
                    metrics::ACTIVE_CONNECTIONS.load(Ordering::Relaxed),
                    metrics::THREAD_POOL_QUEUED.load(Ordering::Relaxed),
                    metrics::SERVER_READY.load(Ordering::Relaxed),
                )))
            },
        )
        .tool(
            "list_routes",
            "All registered HTTP routes (method + pattern)",
            r#"{"type":"object"}"#,
            move |_| {
                let entries: Vec<String> = registered_routes.iter()
                    .map(|(m, p)| format!(r#"{{"method":"{}","pattern":"{}"}}"#, m, p))
                    .collect();
                Ok(McpContent::json(format!("[{}]", entries.join(","))))
            },
        )
        // ── cache ────────────────────────────────────────────────────────────
        .tool(
            "cache_stats",
            "Response cache hit/miss counts and current entry count",
            r#"{"type":"object"}"#,
            move |_| {
                Ok(McpContent::json(format!(
                    r#"{{"hits":{},"misses":{},"size":{}}}"#,
                    cache2.hits(),
                    cache2.misses(),
                    cache2.size(),
                )))
            },
        )
        .tool(
            "cache_clear",
            "Evict all entries from the response cache",
            r#"{"type":"object"}"#,
            move |_| {
                cache3.clear();
                Ok(McpContent::text("cache cleared"))
            },
        )
        // ── rate limiting ────────────────────────────────────────────────────
        .tool(
            "rate_limit_config",
            "Current rate limit settings (max requests per window and window duration)",
            r#"{"type":"object"}"#,
            |_| {
                let cfg = rust_web_server::config_reload::current();
                Ok(McpContent::json(format!(
                    r#"{{"max_requests":{},"window_secs":{}}}"#,
                    cfg.rate_limit_max_requests,
                    cfg.rate_limit_window_secs,
                )))
            },
        )
        .tool(
            "check_rate_limit",
            "Check remaining rate limit quota for a client IP address",
            r#"{"type":"object","properties":{"ip":{"type":"string","description":"Client IP address to check"}},"required":["ip"]}"#,
            |args| {
                let ip = extract_arg(args, "ip").unwrap_or_default();
                let rl = rust_web_server::rate_limit::global();
                let remaining = rl.remaining(&ip);
                let allowed = rl.check(&ip);
                Ok(McpContent::json(format!(
                    r#"{{"ip":"{}","remaining":{},"allowed":{}}}"#,
                    ip, remaining, allowed,
                )))
            },
        )
        // ── CORS / config ────────────────────────────────────────────────────
        .tool(
            "cors_config",
            "Current CORS configuration snapshot",
            r#"{"type":"object"}"#,
            |_| {
                let cfg = rust_web_server::config_reload::current();
                Ok(McpContent::json(format!(
                    r#"{{"allow_all":{},"origins":"{}","methods":"{}","headers":"{}","credentials":"{}","expose_headers":"{}","max_age":"{}"}}"#,
                    cfg.cors_allow_all,
                    cfg.cors_allow_origins,
                    cfg.cors_allow_methods,
                    cfg.cors_allow_headers,
                    cfg.cors_allow_credentials,
                    cfg.cors_expose_headers,
                    cfg.cors_max_age,
                )))
            },
        )
        .tool(
            "list_static_files",
            "List files the static file controller can serve from the working directory",
            r#"{"type":"object"}"#,
            |_| {
                let files: Vec<String> = std::fs::read_dir(".")
                    .map_err(|e| e.to_string())?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .map(|n| format!(r#""{}""#, n))
                    .collect();
                Ok(McpContent::json(format!("[{}]", files.join(","))))
            },
        )
        .tool(
            "reload_config",
            "Trigger a hot config reload from environment variables and rws.config.toml",
            r#"{"type":"object"}"#,
            |_| {
                rust_web_server::config_reload::RELOAD_REQUESTED.store(true, Ordering::SeqCst);
                Ok(McpContent::text("config reload requested"))
            },
        )
        // ── runtime controls ─────────────────────────────────────────────────
        .tool(
            "maintenance_status",
            "Check whether maintenance mode is currently active",
            r#"{"type":"object"}"#,
            |_| {
                let active = MAINTENANCE_MODE.load(Ordering::Relaxed);
                Ok(McpContent::json(format!(r#"{{"active":{}}}"#, active)))
            },
        )
        .tool(
            "set_maintenance",
            "Enable or disable maintenance mode (returns 503 for all non-health paths when active)",
            r#"{"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}"#,
            |args| {
                let enabled = extract_arg(args, "enabled")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                MAINTENANCE_MODE.store(enabled, Ordering::SeqCst);
                Ok(McpContent::json(format!(r#"{{"active":{}}}"#, enabled)))
            },
        )
        .tool(
            "block_ip",
            "Add an IP address to the runtime blocklist (returns 403 for future requests)",
            r#"{"type":"object","properties":{"ip":{"type":"string"}},"required":["ip"]}"#,
            |args| {
                let ip = extract_arg(args, "ip").unwrap_or_default();
                blocklist::global().block(&ip);
                Ok(McpContent::json(format!(r#"{{"blocked":"{}"}}"#, ip)))
            },
        )
        .tool(
            "unblock_ip",
            "Remove an IP address from the runtime blocklist",
            r#"{"type":"object","properties":{"ip":{"type":"string"}},"required":["ip"]}"#,
            |args| {
                let ip = extract_arg(args, "ip").unwrap_or_default();
                blocklist::global().unblock(&ip);
                Ok(McpContent::json(format!(r#"{{"unblocked":"{}"}}"#, ip)))
            },
        )
        .tool(
            "list_blocked_ips",
            "List all currently blocked IP addresses",
            r#"{"type":"object"}"#,
            |_| {
                let list: Vec<String> = blocklist::global().list()
                    .into_iter()
                    .map(|ip| format!(r#""{}""#, ip))
                    .collect();
                Ok(McpContent::json(format!("[{}]", list.join(","))))
            },
        )
        .tool(
            "feature_list",
            "List all runtime feature flags and their current enabled state",
            r#"{"type":"object"}"#,
            |_| {
                let pairs: Vec<String> = feature::global().list()
                    .into_iter()
                    .map(|(k, v)| format!(r#"{{\"name\":\"{}\",\"enabled\":{}}}"#, k, v))
                    .collect();
                Ok(McpContent::json(format!("[{}]", pairs.join(","))))
            },
        )
        .tool(
            "feature_set",
            "Enable or disable a named runtime feature flag",
            r#"{"type":"object","properties":{"name":{"type":"string"},"enabled":{"type":"boolean"}},"required":["name","enabled"]}"#,
            |args| {
                let name = extract_arg(args, "name").unwrap_or_default();
                let enabled = extract_arg(args, "enabled")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                feature::global().set(&name, enabled);
                Ok(McpContent::json(format!(r#"{{"name":"{}","enabled":{}}}"#, name, enabled)))
            },
        )
        // ── request log ──────────────────────────────────────────────────────
        .tool(
            "recent_requests",
            "Last N requests recorded by the server (default 20, max 100)",
            r#"{"type":"object","properties":{"n":{"type":"integer","description":"Number of entries to return (default 20)"}}}"#,
            |args| {
                let n = extract_arg(args, "n")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(20)
                    .min(100);
                let entries: Vec<String> = request_log::global().recent(n)
                    .into_iter()
                    .map(|e| format!(
                        r#"{{"ts":{},"method":"{}","path":"{}","status":{},"ip":"{}","latency_ms":{}}}"#,
                        e.timestamp, e.method, e.path, e.status, e.client_ip, e.latency_ms,
                    ))
                    .collect();
                Ok(McpContent::json(format!("[{}]", entries.join(","))))
            },
        )
        .tool(
            "recent_errors",
            "Last N requests with a 4xx or 5xx status code (default 20, max 100)",
            r#"{"type":"object","properties":{"n":{"type":"integer","description":"Number of error entries to return (default 20)"}}}"#,
            |args| {
                let n = extract_arg(args, "n")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(20)
                    .min(100);
                let entries: Vec<String> = request_log::global().recent_errors(n)
                    .into_iter()
                    .map(|e| format!(
                        r#"{{"ts":{},"method":"{}","path":"{}","status":{},"ip":"{}","latency_ms":{}}}"#,
                        e.timestamp, e.method, e.path, e.status, e.client_ip, e.latency_ms,
                    ))
                    .collect();
                Ok(McpContent::json(format!("[{}]", entries.join(","))))
            },
        )
}

// ── main ──────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "http2"))]
fn main() {
    let (listener, pool) = Server::setup().expect("server setup failed");
    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        SERVER_READY.store(true, Ordering::SeqCst);
        Server::run(listener, pool, proxy_app);
        return;
    }
    SERVER_READY.store(true, Ordering::SeqCst);
    Server::run(listener, pool, build_app());
}

#[cfg(all(feature = "http2", not(feature = "http3")))]
#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup failed");

    #[cfg(feature = "acme")]
    {
        use rust_web_server::acme::{AcmeConfig, AcmeManager};
        if let Some(cfg) = AcmeConfig::from_env() {
            let mgr = AcmeManager::new(cfg);
            if let Err(e) = mgr.provision_if_needed().await {
                eprintln!("[ACME] Startup provisioning failed: {e}");
            }
            tokio::spawn(mgr.run_renewal_loop());
        }
    }

    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        SERVER_READY.store(true, Ordering::SeqCst);
        tokio::join!(
            Server::run_tls(listener, pool, proxy_app.clone()),
            Server::run_redirect(),
        );
        return;
    }
    SERVER_READY.store(true, Ordering::SeqCst);
    tokio::join!(
        Server::run_tls(listener, pool, build_app()),
        Server::run_redirect(),
    );
}

#[cfg(feature = "http3")]
#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup failed");

    #[cfg(feature = "acme")]
    {
        use rust_web_server::acme::{AcmeConfig, AcmeManager};
        if let Some(cfg) = AcmeConfig::from_env() {
            let mgr = AcmeManager::new(cfg);
            if let Err(e) = mgr.provision_if_needed().await {
                eprintln!("[ACME] Startup provisioning failed: {e}");
            }
            tokio::spawn(mgr.run_renewal_loop());
        }
    }

    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        SERVER_READY.store(true, Ordering::SeqCst);
        tokio::join!(
            Server::run_tls(listener, pool, proxy_app.clone()),
            Server::run_quic(proxy_app),
            Server::run_redirect(),
        );
        return;
    }
    SERVER_READY.store(true, Ordering::SeqCst);
    tokio::join!(
        Server::run_tls(listener, pool, build_app()),
        Server::run_quic(build_app()),
        Server::run_redirect(),
    );
}
