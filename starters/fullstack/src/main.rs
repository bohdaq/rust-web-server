//! Wiring only: connect the store and the accounts, serve the site from `src/lib.rs` with
//! the built frontend as static files, and the dev MCP endpoint (`/mcp`)
//! in front of it when `RWS_ENV=dev` or `MCP_TOKEN` is set.

use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::server::Server;
use {{crate_name}}::{dev_tools, site, Accounts, Store};

#[tokio::main]
async fn main() {
    // Resolve the migrations before moving into the static directory: rws
    // serves files from its working directory, so it must be the built
    // frontend, never the source tree.
    let migrations = std::fs::canonicalize("migrations").expect("migrations directory");
    let migrations = migrations.to_string_lossy();
    let store = Store::connect(&migrations).await.expect("store");
    let accounts = Accounts::from_env(&store).await.expect("accounts");
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/dist".into());
    std::env::set_current_dir(&static_dir).unwrap_or_else(|e| panic!("STATIC_DIR {static_dir}: {e} (build the frontend first: cd frontend && npm ci && npm run build)"));

    let (listener, pool) = Server::setup().expect("server setup");
    let tools = dev_tools(&store, &migrations);
    Server::run_tls(listener, pool, site(store, &accounts).dev_mcp_with(tools)).await;
}
