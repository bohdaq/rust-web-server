//! Wiring only: serve the MCP server from `src/lib.rs` at `/mcp`, behind
//! `MCP_TOKEN`; the dev MCP endpoint (`/_dev/mcp`) in front of it when
//! `RWS_ENV=dev` or `DEV_MCP_TOKEN` is set.
//!
//! ```sh
//! MCP_TOKEN=$(openssl rand -hex 32) cargo run
//! claude mcp add --transport http notes http://127.0.0.1:7878/mcp --header "Authorization: Bearer $MCP_TOKEN"
//! ```

use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::server::Server;

#[tokio::main]
async fn main() {
    let token = std::env::var("MCP_TOKEN").unwrap_or_default();
    if token.trim().is_empty() {
        eprintln!("set MCP_TOKEN (e.g. MCP_TOKEN=$(openssl rand -hex 32)) — /mcp never runs without a token");
        std::process::exit(2);
    }
    let (listener, pool) = Server::setup().expect("server setup");
    let app = {{crate_name}}::server({{crate_name}}::Store::default(), token.trim()).dev_mcp_with({{crate_name}}::dev_tools());
    Server::run_tls(listener, pool, app).await;
}
