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
        App::with_state(AppState { version: "1.0.0" }),
        GET  "/api/version" => get_version,
        POST "/api/echo"    => echo_post,
    };

    // 2. Attach the MCP server.
    //    POST /mcp is handled here; everything else is forwarded to `http` above.
    http.mcp("rws", "1.0.0")
        .tool(
            "version",
            "Get the server version",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(r#"{"version":"1.0.0"}"#)),
        )
        .tool(
            "echo",
            "Echo a message back",
            r#"{"type":"object","properties":{"message":{"type":"string"}},"required":["message"]}"#,
            |args| {
                let msg = extract_arg(args, "message").unwrap_or_default();
                Ok(McpContent::text(msg))
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
