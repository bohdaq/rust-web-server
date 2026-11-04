//! A JSON API on rust-web-server: typed handlers (`#[get]`, extractors as
//! arguments), serde types, an OpenAPI 3.1 document from the same types
//! (`/openapi.json`, Swagger UI at `/docs`), and tests that need no socket.
//!
//! The conventional rws layout (`rws.app.toml`): this file builds the app
//! (`app()`), `src/routes/` holds the handlers, `src/main.rs` only serves
//! it, `tests/` tests it. See AGENTS.md.

pub mod routes;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rust_web_server::dev_mcp::DevTools;
use rust_web_server::openapi::OpenApiConfig;
use rust_web_server::prelude::*;
use rust_web_server::state::AppWithState;

pub use routes::todos::{NewTodo, Problem, Todo};

/// Shared state: the app keeps it behind an `Arc` and hands each handler
/// `&Store`; the data sits behind a `Mutex` since handlers run in parallel.
#[derive(Clone, Default)]
pub struct Store {
    pub todos: Arc<Mutex<BTreeMap<u64, Todo>>>,
}

/// The app: every route, and the OpenAPI document describing them.
pub fn app() -> AppWithState<Store> {
    routes::todos::register(App::with_state(Store::default())).openapi(openapi())
}

/// The API's OpenAPI document: each resource describes its own routes.
fn openapi() -> OpenApiConfig {
    routes::todos::describe(OpenApiConfig::new("To-dos", env!("CARGO_PKG_VERSION")))
}

/// What the dev MCP endpoint (`/mcp`, with `RWS_ENV=dev` or `MCP_TOKEN`
/// set) shows a coding agent: routes, config, this project's layout, the
/// last requests and errors.
pub fn dev_tools() -> DevTools {
    DevTools::new().app_dir(env!("CARGO_MANIFEST_DIR"))
}
