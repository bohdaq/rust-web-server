//! Async-capable state-aware application — requires the `http2` feature (tokio).
//!
//! [`AsyncAppWithState<S>`] is the async counterpart to [`AppWithState<S>`]:
//! handlers are `async fn` closures that can `await` database queries, HTTP
//! clients, or any other async I/O without blocking the OS thread.
//!
//! The sync [`Application`] bridge works in any calling context: when inside
//! an existing tokio runtime (HTTP/2 / HTTP/3), it spawns a scoped OS thread
//! with its own single-threaded runtime; when called from the HTTP/1.1
//! thread-pool (no runtime), it creates a temporary single-threaded runtime.
//!
//! Unmatched routes fall through to the built-in [`App`] controller chain.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use rust_web_server::async_state::AsyncAppWithState;
//! use rust_web_server::core::New;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//! use rust_web_server::range::Range;
//! use rust_web_server::mime_type::MimeType;
//! use rust_web_server::router::PathParams;
//! use rust_web_server::request::Request;
//! use rust_web_server::server::ConnectionInfo;
//!
//! struct AppState {
//!     greeting: String,
//! }
//!
//! let app = AsyncAppWithState::new(AppState { greeting: "Hello".to_string() })
//!     .get("/greet/:name", |_req, params, _conn, state| async move {
//!         let name = params.get("name").unwrap_or("world");
//!         let body = format!("{}, {}!", state.greeting, name);
//!         let mut r = Response::new();
//!         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         r.content_range_list = vec![
//!             Range::get_content_range(body.into_bytes(), MimeType::TEXT_PLAIN.to_string())
//!         ];
//!         r
//!     });
//! ```
//!
//! [`AppWithState<S>`]: crate::state::AppWithState

#[cfg(test)]
mod tests;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::request::Request;
use crate::response::Response;
use crate::router::matcher::{self, Segment};
use crate::router::PathParams;
use crate::server::ConnectionInfo;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

type AsyncHandlerFn<S> = Arc<
    dyn Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> BoxFuture<Response> + Send + Sync,
>;

// ── AsyncRoute ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AsyncRoute<S> {
    method: String,
    segments: Vec<Segment>,
    handler: AsyncHandlerFn<S>,
}

// ── AsyncAppWithState ─────────────────────────────────────────────────────────

/// An [`Application`] whose route handlers are `async` functions.
///
/// State is stored as `Arc<S>` and passed by value (cheap clone) to each
/// handler invocation. Handlers receive owned `Request`, `PathParams`, and
/// `ConnectionInfo` values so the returned future is `'static`.
#[derive(Clone)]
pub struct AsyncAppWithState<S> {
    state: Arc<S>,
    routes: Vec<AsyncRoute<S>>,
    /// When `Some`, the fallback `App` is pinned to this config (see
    /// [`App::with_config`]); when `None`, the fallback reads
    /// `RWS_CONFIG_*` env vars per request via `App::new()`, same as `App`'s
    /// own default.
    config: Option<Arc<crate::server_config::ServerConfig>>,
}

impl<S: Send + Sync + 'static> AsyncAppWithState<S> {
    /// Create a new `AsyncAppWithState` wrapping `state`.
    pub fn new(state: S) -> Self {
        AsyncAppWithState { state: Arc::new(state), routes: Vec::new(), config: None }
    }

    /// Pin the fallback [`App`] (used for any request this app's own routes
    /// don't match) to an explicit [`crate::server_config::ServerConfig`],
    /// instead of reading `RWS_CONFIG_*` environment variables per request.
    ///
    /// Mirrors [`App::with_config`] / [`crate::state::AppWithState::with_config`].
    pub fn with_config(mut self, config: crate::server_config::ServerConfig) -> Self {
        self.config = Some(Arc::new(config));
        self
    }

    /// Return a reference to the shared state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// The fallback `App` for requests this app's own routes don't match —
    /// pinned to `self.config` if set, otherwise `App::new()`'s default
    /// per-request env read.
    fn fallback_app(&self) -> App {
        match &self.config {
            Some(c) => App::with_config((**c).clone()),
            None => App::new(),
        }
    }

    fn add<F, Fut>(mut self, method: &str, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.routes.push(AsyncRoute {
            method: method.to_string(),
            segments: matcher::parse_pattern(pattern),
            handler: Arc::new(move |req, params, conn, state| Box::pin(handler(req, params, conn, state))),
        });
        self
    }

    /// Register an async `GET` handler for `pattern`.
    pub fn get<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.add("GET", pattern, handler)
    }

    /// Register an async `POST` handler for `pattern`.
    pub fn post<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.add("POST", pattern, handler)
    }

    /// Register an async `PUT` handler for `pattern`.
    pub fn put<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.add("PUT", pattern, handler)
    }

    /// Register an async `PATCH` handler for `pattern`.
    pub fn patch<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.add("PATCH", pattern, handler)
    }

    /// Register an async `DELETE` handler for `pattern`.
    pub fn delete<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.add("DELETE", pattern, handler)
    }

    async fn execute_async(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
    ) -> Result<Response, String> {
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for route in &self.routes {
            if route.method != request.method {
                continue;
            }
            if let Some(params_map) = matcher::try_match(&route.segments, &path_segs) {
                let params = PathParams::from_map(params_map);
                let fut = (route.handler)(
                    request.clone(),
                    params,
                    connection.clone(),
                    Arc::clone(&self.state),
                );
                return Ok(fut.await);
            }
        }

        self.fallback_app().execute(request, connection)
    }
}

impl<S: Send + Sync + 'static> Application for AsyncAppWithState<S> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let request = request.clone();
        let connection = connection.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                // Inside an existing runtime: run the future on a scoped OS thread
                // with its own single-threaded runtime to avoid blocking the event loop.
                std::thread::scope(|s| {
                    s.spawn(|| {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(self.execute_async(&request, &connection))
                    })
                    .join()
                    .unwrap()
                })
            }
            Err(_) => {
                // Not inside any runtime (HTTP/1.1 thread pool): create a temporary one.
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(self.execute_async(&request, &connection))
            }
        }
    }
}
