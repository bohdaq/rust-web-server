/// The `mimalloc` feature: the binary allocates with mimalloc (v3) — less
/// CPU per request than glibc's allocator (most of what's left of the
/// per-request cost in user space is allocation), more so on musl. Its
/// defaults hold freed memory for a second; purging at once
/// (`MIMALLOC_PURGE_DELAY=0`) uses less memory under large TLS downloads but
/// re-faults pages: 70 faults per 1 MiB request, up to 50% more CPU.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use rust_web_server::app::App;
use rust_web_server::blocklist::{self, BlocklistLayer};
use rust_web_server::cache::CacheLayer;
use rust_web_server::core::New;
use rust_web_server::feature;
use rust_web_server::maintenance::{MaintenanceLayer, MAINTENANCE_MODE};
use rust_web_server::mcp::{McpContent, ToolAnnotations, extract_arg};
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
    let body = req.body.to_vec();
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

    // 3. Around the routes: maintenance mode and the response cache.
    //    Maintenance stays inside MCP: the `maintenance` tool is how an
    //    operator switches it off again.
    let app = http.wrap(MaintenanceLayer).wrap(cache);

    // 4. MCP — only with a token (#44): its tools change the running server
    //    (config reload, blocklist, maintenance). Without MCP_TOKEN, /mcp
    //    isn't mounted at all. Browser origins must be listed in
    //    MCP_ALLOWED_ORIGINS (comma-separated).
    let token = std::env::var("MCP_TOKEN").unwrap_or_default();
    let served = if token.trim().is_empty() {
        println!("MCP tools not mounted: set MCP_TOKEN to enable /mcp.");
        Served::Plain(app)
    } else {
        let origins: Vec<String> = std::env::var("MCP_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();
        let mcp = app
            .mcp("rws", env!("CARGO_PKG_VERSION"))
            .require_bearer(token.trim())
            .allowed_origins(origins);
        Served::Mcp(rws_mcp_tools(mcp, registered_routes, cache2, cache3))
    };

    // 5. Outermost — for /mcp as well: tracing (when configured), access
    //    log, IP blocklist, global rate limit.
    traced(rust_web_server::middleware::WithMiddleware::new(served))
        .wrap(LogLayer)
        .wrap(BlocklistLayer)
        .wrap(GlobalRateLimit::from_env())
}

/// Whether requests are traced: `OTEL_EXPORTER_OTLP_ENDPOINT` (or the
/// traces one) set and `OTEL_TRACES_EXPORTER` not `none` — see [`telemetry`].
static TRACING: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Start OpenTelemetry export as the environment asks: metrics
/// (`http.server.request.duration`) and traces, each to its OTLP endpoint.
/// Nothing is exported, and no layer added, without an endpoint.
fn telemetry() {
    if rust_web_server::otel::metrics::start_from_env() {
        println!("OTLP metrics export on.");
    }
    let tracing = rust_web_server::otel::traces_configured();
    if tracing {
        rust_web_server::otel::setup_from_env();
        println!("OTLP trace export on.");
    }
    let _ = TRACING.set(tracing);
}

/// `app` with `OtelLayer` outermost when tracing is on.
fn traced<A: rust_web_server::application::Application>(
    app: rust_web_server::middleware::WithMiddleware<A>,
) -> rust_web_server::middleware::WithMiddleware<A> {
    if TRACING.get().copied().unwrap_or(false) {
        app.wrap(rust_web_server::otel::OtelLayer)
    } else {
        app
    }
}

/// Either the MCP server (with the app behind it) or the app alone.
#[derive(Clone)]
enum Served<M, A> {
    Mcp(M),
    Plain(A),
}

impl<M: rust_web_server::application::Application, A: rust_web_server::application::Application>
    rust_web_server::application::Application for Served<M, A>
{
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        match self {
            Served::Mcp(m) => m.execute(request, connection),
            Served::Plain(a) => a.execute(request, connection),
        }
    }

    fn runs_inline(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        match self {
            Served::Mcp(m) => m.runs_inline(request, connection),
            Served::Plain(a) => a.runs_inline(request, connection),
        }
    }

    #[cfg(feature = "http2")]
    fn execute_async<'a>(
        &'a self,
        request: &'a Request,
        connection: &'a ConnectionInfo,
    ) -> Option<rust_web_server::application::AsyncResponse<'a>> {
        match self {
            Served::Mcp(m) => m.execute_async(request, connection),
            Served::Plain(a) => a.execute_async(request, connection),
        }
    }
}

/// The built-in rws tools on `mcp`.
fn rws_mcp_tools(
    mcp: rust_web_server::mcp::McpServer,
    registered_routes: Vec<(String, String)>,
    cache2: CacheLayer,
    cache3: CacheLayer,
) -> rust_web_server::mcp::McpServer {
    let mcp = mcp
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
                rust_web_server::config_reload::request_reload();
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
        );
    // Behaviour hints, so an AI client can tell looking from changing — and
    // ask before the latter.
    let read_only = ToolAnnotations { read_only_hint: Some(true), open_world_hint: Some(false), ..ToolAnnotations::default() };
    let changes = |destructive: bool| ToolAnnotations {
        read_only_hint: Some(false),
        destructive_hint: Some(destructive),
        idempotent_hint: Some(true),
        open_world_hint: Some(false),
    };
    let mut mcp = mcp;
    for name in ["server_config", "feature_flags", "server_metrics", "list_routes", "cache_stats", "rate_limit_config",
        "check_rate_limit", "cors_config", "list_static_files", "maintenance_status", "list_blocked_ips", "feature_list",
        "recent_requests", "recent_errors"] {
        mcp = mcp.tool_annotations(name, read_only.clone());
    }
    for name in ["cache_clear", "reload_config", "set_maintenance", "block_ip", "feature_set"] {
        mcp = mcp.tool_annotations(name, changes(true));
    }
    mcp.tool_annotations("unblock_ip", changes(false))
}

// ── main ──────────────────────────────────────────────────────────────────────

/// The `[rate_limit]` / `RWS_CONFIG_RATE_LIMIT_*` per-client-IP limit,
/// enforced only when configured (decided once at startup; enabling it later
/// needs a restart, changing the numbers is hot-reloadable).
struct GlobalRateLimit(bool);

impl GlobalRateLimit {
    fn from_env() -> Self {
        GlobalRateLimit(rust_web_server::rate_limit::is_configured())
    }
}

impl rust_web_server::middleware::Middleware for GlobalRateLimit {
    /// Headers only: a streamed body passes through untouched.
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn rust_web_server::application::Application,
    ) -> Result<Response, String> {
        if self.0 {
            rust_web_server::middleware::RateLimitLayer.handle(request, connection, next)
        } else {
            next.execute(request, connection)
        }
    }

    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true // an in-memory counter
    }

    fn is_passthrough(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        !self.0 // not configured: does nothing
    }
}

/// Automatic HTTPS: when `RWS_CONFIG_ACME_DOMAINS` is set, obtain (or reuse)
/// a certificate before the TLS listener starts, then keep it renewed.
#[cfg(feature = "http2")]
async fn start_acme(listener: Option<&std::net::TcpListener>) {
    #[cfg(feature = "acme")]
    {
        use rust_web_server::acme::{AcmeConfig, AcmeManager};
        if let Some(cfg) = AcmeConfig::from_env() {
            cfg.install_as_tls_certificate();
            // TLS-ALPN-01 before the server runs: answer on its own socket.
            if rust_web_server::acme::tls_alpn::enabled() {
                if let Some(socket) = listener.and_then(|l| l.try_clone().ok()) {
                    rust_web_server::acme::tls_alpn::set_startup_listener(socket);
                }
            }
            let mgr = AcmeManager::new(cfg.clone());
            mgr.provision_until_ready().await;
            tokio::spawn(mgr.run_renewal_loop());
            if rust_web_server::acme::on_demand::configured() {
                if let Err(e) = rust_web_server::acme::on_demand::start(&cfg) {
                    eprintln!("[ACME] {e}");
                }
            }
        }
    }
    #[cfg(not(feature = "acme"))]
    {
        let _ = listener;
        let domains = std::env::var(rust_web_server::entry_point::Config::RWS_CONFIG_ACME_DOMAINS)
            .unwrap_or_default();
        if !domains.trim().is_empty() {
            eprintln!(
                "[ACME] RWS_CONFIG_ACME_DOMAINS is set, but this rws binary was built without the \
                 `acme` feature — no certificate will be requested. Use a release binary or \
                 `cargo install rust-web-server --features acme`."
            );
        }
    }
}

/// `rws --version` / `rws --help`: print and exit before binding any socket.
/// `rws --check[=<file>]`: validate the configuration, exit 0 if it has no
/// errors, 1 otherwise.
fn exit_if_informational_flag() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(output) = rust_web_server::entry_point::command_line_args::informational_flag_output(&args) {
        print!("{output}");
        std::process::exit(0);
    }
    if let Some(path) = rust_web_server::config_check::check_target_from_args(&args) {
        let report = rust_web_server::config_check::check_file(&path);
        if rust_web_server::config_check::wants_json_output(&args) {
            println!("{}", report.render_json(&path));
        } else {
            print!("{}", report.render(&path));
        }
        std::process::exit(if report.is_ok() { 0 } else { 1 });
    }
}

#[cfg(not(feature = "http2"))]
fn main() {
    exit_if_informational_flag();
    let (listener, pool) = Server::setup().expect("server setup failed");
    rust_web_server::log::Log::reopen_access_log_on_sigusr1();
    rust_web_server::config_watch::start();
    telemetry();
    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        let proxy_app = traced(rust_web_server::middleware::WithMiddleware::new(proxy_app)).wrap(GlobalRateLimit::from_env());
        SERVER_READY.store(true, Ordering::SeqCst);
        Server::run(listener, pool, proxy_app);
    } else {
        SERVER_READY.store(true, Ordering::SeqCst);
        Server::run(listener, pool, build_app());
    }
    rust_web_server::otel::shutdown();
}

/// The server: everything `main` runs once the runtime is up.
#[cfg(all(feature = "http2", not(feature = "http3")))]
async fn serve() {
    rust_web_server::server::per_core::default_to_per_core();
    let (listener, pool) = Server::setup().expect("server setup failed");
    rust_web_server::log::Log::reopen_access_log_on_sigusr1();
    telemetry();
    #[cfg(feature = "gateway-api")]
    rust_web_server::gateway_api::start_from_env();

    start_acme(Some(&listener)).await;
    // After ACME, not before: provisioning writes the certificate and key the
    // watcher watches, and its SIGHUP arrived before run_tls installs the
    // handler, so the signal's default action ended the process.
    rust_web_server::config_watch::start();

    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        let proxy_app = traced(rust_web_server::middleware::WithMiddleware::new(proxy_app)).wrap(GlobalRateLimit::from_env());
        SERVER_READY.store(true, Ordering::SeqCst);
        tokio::join!(
            Server::run_tls(listener, pool, proxy_app.clone()),
            Server::run_redirect(),
        );
    } else {
        SERVER_READY.store(true, Ordering::SeqCst);
        tokio::join!(
            Server::run_tls(listener, pool, build_app()),
            Server::run_redirect(),
        );
    }
    let _ = tokio::task::spawn_blocking(rust_web_server::otel::shutdown).await;
}

/// The server: everything `main` runs once the runtime is up.
#[cfg(feature = "http3")]
async fn serve() {
    rust_web_server::server::per_core::default_to_per_core();
    let (listener, pool) = Server::setup().expect("server setup failed");
    rust_web_server::log::Log::reopen_access_log_on_sigusr1();
    telemetry();
    #[cfg(feature = "gateway-api")]
    rust_web_server::gateway_api::start_from_env();

    start_acme(Some(&listener)).await;
    // After ACME, not before: provisioning writes the certificate and key the
    // watcher watches, and its SIGHUP arrived before run_tls installs the
    // handler, so the signal's default action ended the process.
    rust_web_server::config_watch::start();

    if rust_web_server::proxy_config::ProxyConfig::is_proxy_mode() {
        let (proxy_app, _handles) = rust_web_server::proxy_config::build_from_file();
        let proxy_app = traced(rust_web_server::middleware::WithMiddleware::new(proxy_app)).wrap(GlobalRateLimit::from_env());
        SERVER_READY.store(true, Ordering::SeqCst);
        tokio::join!(
            Server::run_tls(listener, pool, proxy_app.clone()),
            Server::run_quic(proxy_app),
            Server::run_redirect(),
        );
    } else {
        SERVER_READY.store(true, Ordering::SeqCst);
        // One app for both listeners (building it twice printed its
        // startup notes twice).
        let app = build_app();
        tokio::join!(
            Server::run_tls(listener, pool, app.clone()),
            Server::run_quic(app),
            Server::run_redirect(),
        );
    }
    // Last metrics and spans out before the process ends; blocking, so off
    // the runtime's workers.
    let _ = tokio::task::spawn_blocking(rust_web_server::otel::shutdown).await;
}

/// A multi-threaded tokio runtime, as `#[tokio::main]` builds one.
#[cfg(feature = "http2")]
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime")
}

#[cfg(feature = "http2")]
fn main() {
    exit_if_informational_flag();
    #[cfg(windows)]
    if let Some(directory) = windows_service_mode::requested() {
        windows_service_mode::run(directory);
        return;
    }
    runtime().block_on(serve());
}

/// `rws --windows-service=<directory>`: run under the Windows Service Control
/// Manager, serving `<directory>` (and reading its `rws.config.toml`). A
/// service starts in `System32` with no console, so the directory is
/// required and logs belong in a file (`access_log`). Stop and Shutdown
/// controls shut the server down gracefully, as SIGTERM does on Unix.
#[cfg(all(windows, feature = "http2"))]
mod windows_service_mode {
    use std::ffi::OsString;
    use std::time::Duration;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    const FLAG: &str = "--windows-service=";

    /// The directory given with `--windows-service=`, if the flag is present.
    pub fn requested() -> Option<String> {
        std::env::args().find_map(|arg| arg.strip_prefix(FLAG).map(str::to_string))
    }

    define_windows_service!(ffi_service_main, service_main);

    pub fn run(directory: String) {
        if let Err(e) = std::env::set_current_dir(&directory) {
            eprintln!("--windows-service: can't serve {directory}: {e}");
            std::process::exit(1);
        }
        // The service's name comes from the SCM; for an own-process service
        // the name passed here isn't checked.
        if let Err(e) = service_dispatcher::start("rws", ffi_service_main) {
            eprintln!("--windows-service: not started by the Service Control Manager: {e}");
            std::process::exit(1);
        }
    }

    fn service_main(_arguments: Vec<OsString>) {
        let handler = |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                rust_web_server::server::request_shutdown();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let Ok(status) = service_control_handler::register("rws", handler) else { return };
        let report = |state, accepted| {
            let _ = status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accepted,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(10),
                process_id: None,
            });
        };
        report(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN);
        super::runtime().block_on(super::serve());
        report(ServiceState::Stopped, ServiceControlAccept::empty());
    }
}
