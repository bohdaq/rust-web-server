//! An MCP server on rust-web-server: typed tools an AI agent can call (input
//! and output schemas from serde types), a resource, a bearer token, and
//! tests that need no socket.
//!
//! The conventional rws layout (`rws.app.toml`; this app's "routes" are its
//! MCP tools, in `src/tools/`): this file builds the server (`server()`),
//! `src/main.rs` only serves it, `tests/` tests it. See AGENTS.md.

pub mod tools;

use std::sync::{Arc, Mutex};

use rust_web_server::dev_mcp::DevTools;
use rust_web_server::mcp::McpServer;

pub use tools::notes::Note;

/// The notes, shared by every tool (clones share the `Arc`).
pub type Store = Arc<Mutex<Vec<Note>>>;

/// The server: every tool and resource, behind `token`.
pub fn server(store: Store, token: &str) -> McpServer {
    let server = McpServer::new("notes", env!("CARGO_PKG_VERSION")).require_bearer(token);
    tools::notes::register(server, store)
}

/// The dev MCP endpoint, for a coding agent working on this project: its
/// own path (`/mcp` is the app's), its own token variable (`MCP_TOKEN` is
/// the app's). Mounted with `RWS_ENV=dev` or `DEV_MCP_TOKEN` set.
pub fn dev_tools() -> DevTools {
    DevTools::new().at("/_dev/mcp").token_env("DEV_MCP_TOKEN").app_dir(env!("CARGO_MANIFEST_DIR"))
}
