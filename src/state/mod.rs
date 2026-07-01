//! Shared application state and state-aware routing.
//!
//! [`AppWithState<S>`] combines a typed state value (database pools, config,
//! caches) with route registration.  Routes are tried first; requests that do
//! not match fall through to the built-in [`App`] controller chain (static
//! files, healthz, metrics, …).
//!
//! State is stored as an [`Arc<S>`] and shared across all handlers. Handlers
//! receive an immutable `&S` reference alongside the request context.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::state::AppWithState;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//! use rust_web_server::range::Range;
//! use rust_web_server::mime_type::MimeType;
//! use rust_web_server::core::New;
//!
//! struct AppState {
//!     greeting: String,
//! }
//!
//! let app = AppWithState::new(AppState { greeting: "Hello".to_string() })
//!     .get("/greet", |_req, _params, _conn, state| {
//!         let mut r = Response::new();
//!         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         r.content_range_list = vec![
//!             Range::get_content_range(
//!                 state.greeting.as_bytes().to_vec(),
//!                 MimeType::TEXT_PLAIN.to_string(),
//!             )
//!         ];
//!         r
//!     })
//!     .get("/users/:id", |_req, params, _conn, state| {
//!         let id = params.get("id").unwrap_or("?");
//!         let body = format!("{}, user {}!", state.greeting, id);
//!         let mut r = Response::new();
//!         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         r.content_range_list = vec![
//!             Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())
//!         ];
//!         r
//!     });
//! ```

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::Request;
use crate::response::Response;
use crate::router::{PathParams, Router};
use crate::server::ConnectionInfo;

/// An [`Application`] that combines user-defined state-aware routes with the
/// built-in [`App`] controller chain as a fallback.
///
/// Routes are matched in registration order. The first match wins; unmatched
/// requests are forwarded to [`App`] (static files, health probes, etc.).
#[derive(Clone)]
pub struct AppWithState<S> {
    state: Arc<S>,
    router: Router,
}

impl<S: Send + Sync + 'static> AppWithState<S> {
    /// Create a new `AppWithState` wrapping `state`.
    ///
    /// `state` is stored behind an `Arc` so it can be shared across threads
    /// without cloning. Register routes with the builder methods.
    pub fn new(state: S) -> Self {
        AppWithState {
            state: Arc::new(state),
            router: Router::new(),
        }
    }

    /// Return a reference to the shared state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Register a `GET` handler for `pattern`.
    pub fn get<F>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        let state = Arc::clone(&self.state);
        self.router = self.router.get(pattern, move |req, params, conn| {
            handler(req, params, conn, &state)
        });
        self
    }

    /// Register a `POST` handler for `pattern`.
    pub fn post<F>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        let state = Arc::clone(&self.state);
        self.router = self.router.post(pattern, move |req, params, conn| {
            handler(req, params, conn, &state)
        });
        self
    }

    /// Register a `PUT` handler for `pattern`.
    pub fn put<F>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        let state = Arc::clone(&self.state);
        self.router = self.router.put(pattern, move |req, params, conn| {
            handler(req, params, conn, &state)
        });
        self
    }

    /// Register a `PATCH` handler for `pattern`.
    pub fn patch<F>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        let state = Arc::clone(&self.state);
        self.router = self.router.patch(pattern, move |req, params, conn| {
            handler(req, params, conn, &state)
        });
        self
    }

    /// Register a `DELETE` handler for `pattern`.
    pub fn delete<F>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        let state = Arc::clone(&self.state);
        self.router = self.router.delete(pattern, move |req, params, conn| {
            handler(req, params, conn, &state)
        });
        self
    }

    /// Return a snapshot of all registered routes as `(method, pattern)` pairs.
    pub fn route_entries(&self) -> Vec<crate::router::RouteInfo> {
        self.router.route_entries()
    }

    /// Attach an MCP server to this application. Requests that do not match
    /// the MCP endpoint (`POST /mcp`) are forwarded to `self`, so all
    /// previously registered routes remain active.
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::mcp::{McpContent, extract_arg};
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    /// use rust_web_server::core::New;
    ///
    /// struct Db { url: String }
    ///
    /// let app = App::with_state(Db { url: "postgres://localhost/mydb".to_string() })
    ///     .get("/api/users", |_req, _params, _conn, _db| {
    ///         let mut r = Response::new();
    ///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    ///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    ///         r
    ///     })
    ///     .mcp("my-server", "1.0")
    ///     .tool("list_users", "List all users", "{}", |_| {
    ///         Ok(McpContent::json(r#"[{"id":1,"name":"Alice"}]"#))
    ///     });
    /// ```
    pub fn mcp(self, name: impl Into<String>, version: impl Into<String>) -> crate::mcp::McpServer {
        crate::mcp::McpServer::new(name, version).wrap(self)
    }

    /// Wrap this application in a middleware layer.
    ///
    /// Enables fluent composition:
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::core::New;
    /// use rust_web_server::middleware::RateLimitLayer;
    /// use rust_web_server::response::Response;
    ///
    /// let app = App::with_state(())
    ///     .get("/ping", |_, _, _, _| Response::new())
    ///     .wrap(RateLimitLayer);
    /// ```
    pub fn wrap<M: Middleware + 'static>(self, layer: M) -> WithMiddleware<AppWithState<S>> {
        WithMiddleware::new(self).wrap(layer)
    }
}

impl<S: Send + Sync + 'static> Application for AppWithState<S> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if let Some(response) = self.router.handle(request, connection) {
            return Ok(response);
        }
        App::new().execute(request, connection)
    }
}
