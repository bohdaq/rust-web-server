//! Wiring only: serve the app from `src/lib.rs`, with the dev MCP endpoint
//! (`/mcp`) in front of it when `RWS_ENV=dev` or `MCP_TOKEN` is set.

use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::server::Server;

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup");
    let app = {{crate_name}}::app().dev_mcp_with({{crate_name}}::dev_tools());
    Server::run_tls(listener, pool, app).await;
}
