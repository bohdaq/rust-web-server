use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::mcp::{McpContent, extract_arg};
use rust_web_server::metrics::SERVER_READY;
use rust_web_server::request::Request;
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
    // 1. Declare HTTP routes with shared state.
    //    Unmatched requests fall through to the built-in App controller chain.
    let http = routes! {
        App::with_state(AppState { version: env!("CARGO_PKG_VERSION") }),
        GET  "/api/version" => get_version,
        POST "/api/echo"    => echo_post,
    };

    // 2. Attach the MCP server.
    //    POST /mcp is handled here; everything else is forwarded to `http` above.
    //    Set MCP_TOKEN env var to require bearer auth (recommended in production).
    let mut mcp = http.mcp("rws", env!("CARGO_PKG_VERSION"));
    if let Ok(token) = std::env::var("MCP_TOKEN") {
        mcp = mcp.require_bearer(token);
    }

    mcp
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
            "Live server counters: requests processed, errors, active connections, ready state",
            r#"{"type":"object"}"#,
            |_| {
                use rust_web_server::metrics;
                use std::sync::atomic::Ordering;
                Ok(McpContent::json(format!(
                    r#"{{"requests_total":{},"errors_total":{},"active_connections":{},"ready":{}}}"#,
                    metrics::REQUESTS_TOTAL.load(Ordering::Relaxed),
                    metrics::ERRORS_TOTAL.load(Ordering::Relaxed),
                    metrics::ACTIVE_CONNECTIONS.load(Ordering::Relaxed),
                    metrics::SERVER_READY.load(Ordering::Relaxed),
                )))
            },
        )
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
                use std::sync::atomic::Ordering;
                rust_web_server::config_reload::RELOAD_REQUESTED.store(true, Ordering::SeqCst);
                Ok(McpContent::text("config reload requested"))
            },
        )
}

// ── main ──────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "http2"))]
fn main() {
    let (listener, pool) = Server::setup().expect("server setup failed");
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

    SERVER_READY.store(true, Ordering::SeqCst);
    tokio::join!(
        Server::run_tls(listener, pool, build_app()),
        Server::run_quic(build_app()),
        Server::run_redirect(),
    );
}
