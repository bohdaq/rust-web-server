//! rws-docs-mcp: the MCP server at https://rws8.tech/mcp.
//!
//! Read-only answers about rws for coding agents, for the release their
//! project uses: search the docs, read a page, look up a setting or an API
//! item, get an example, list changes between versions, and check a
//! `rws.config.toml` the way `rws --check` does (offline: nothing on this
//! machine is looked at). The data is the index the site build writes
//! (`site/dist/mcp/`), loaded once at startup.
//!
//! ```text
//! RWS_DOCS_MCP_INDEX=/var/www/rws8/mcp RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT=7890 rws-docs-mcp
//! ```
//!
//! The tool names are a public contract once agents use them: add tools,
//! don't rename them.

mod index;
mod tools;

use std::path::PathBuf;
use std::sync::Arc;

use rust_web_server::prelude::*;

#[tokio::main]
async fn main() {
    let dir = PathBuf::from(std::env::var("RWS_DOCS_MCP_INDEX").unwrap_or_else(|_| "/var/www/rws8/mcp".to_string()));
    let docs = match index::Docs::load(&dir) {
        Ok(docs) => Arc::new(docs),
        Err(e) => {
            eprintln!("rws-docs-mcp: cannot load the docs index: {e}");
            std::process::exit(1);
        }
    };
    println!("rws-docs-mcp: docs for {} (latest {})", docs.order.join(", "), docs.latest);
    let origins = std::env::var("RWS_DOCS_MCP_ORIGINS").unwrap_or_else(|_| "https://rws8.tech".to_string());
    let server = tools::server(docs).allowed_origins(origins.split(',').map(str::trim).filter(|o| !o.is_empty()).map(String::from));
    let (listener, pool) = Server::setup().expect("server setup failed");
    Server::run_tls(listener, pool, server).await;
}
