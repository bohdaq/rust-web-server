//! Async-capable state-aware application — requires the `http2` feature (tokio).
//!
//! [`AsyncAppWithState<S>`] is the async counterpart to [`AppWithState<S>`]:
//! handlers are `async fn` closures that can `await` database queries, HTTP
//! clients, or any other async I/O without blocking the OS thread.
//!
//! Where a handler runs:
//!
//! - The async servers (HTTP/1.1, HTTP/2, HTTP/3) await it through
//!   [`Application::execute_async`] when nothing synchronous wraps the app
//!   (every middleware layer [`is_passthrough`]): on the connection's own
//!   runtime when that's the shared `multi_thread` one, on the process-wide
//!   `rws-async` runtime under per-core (`current_thread`) runtimes.
//! - Otherwise — a middleware layer in front, the HTTP/1.1-only build,
//!   [`TestClient::send`], any direct [`Application::execute`] call — the
//!   calling thread waits while it runs on `rws-async`.
//!
//! Either way the runtime outlives the request, so tasks a handler leaves
//! behind run: a `DbPool` gets its connection back (sqlx returns it from a
//! spawned task) instead of closing it. Handlers must not block their
//! thread — they share worker threads with other requests.
//!
//! Unmatched routes fall through to the built-in [`App`] controller chain,
//! synchronously, without touching a runtime.
//!
//! [`is_passthrough`]: crate::middleware::Middleware::is_passthrough
//! [`TestClient::send`]: crate::test_client::TestRequest::send
//!
//! # Example
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
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
#[cfg(feature = "openapi")]
use crate::mime_type::MimeType;
#[cfg(feature = "openapi")]
use crate::range::Range;
#[cfg(feature = "openapi")]
use crate::response::STATUS_CODE_REASON_PHRASE;

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

    /// Return a snapshot of all registered routes as `(method, pattern)` pairs.
    pub fn route_entries(&self) -> Vec<crate::router::RouteInfo> {
        self.routes
            .iter()
            .map(|r| crate::router::RouteInfo {
                method: r.method.clone(),
                pattern: matcher::segments_to_pattern(&r.segments),
            })
            .collect()
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
        if let Some((last, earlier)) = self.routes.split_last() {
            let earlier = earlier.iter().map(|r| (r.method.as_str(), r.segments.as_slice()));
            if let Some(conflict) = crate::router::conflict_with(earlier, &last.method, &last.segments) {
                conflict.warn_once();
            }
        }
        self
    }

    /// Routes that never run because an earlier one with the same method
    /// matches every path they do (18.28.0). See
    /// [`Router::route_conflicts`](crate::router::Router::route_conflicts);
    /// each is also printed once to stderr when it's registered.
    pub fn route_conflicts(&self) -> Vec<crate::router::RouteConflict> {
        crate::router::conflicts(self.routes.iter().map(|r| (r.method.as_str(), r.segments.as_slice())))
    }

    /// Register a typed handler (18.28.0): an `async fn` (or a sync
    /// function) with extractor arguments annotated `#[get("/path")]` &
    /// co.; the method and pattern come from the attribute.
    ///
    /// ```rust,ignore
    /// #[get("/posts/{id}")]
    /// async fn show_post(Path(id): Path<i64>, State(pool): State<DbPool>) -> Result<Json<Post>, AppError> {
    ///     // … .await …
    /// }
    ///
    /// let app = App::with_async_state(pool).route(show_post);
    /// ```
    pub fn route<H: crate::handler::AsyncHandler<S>>(self, handler: H) -> Self {
        let handler = Arc::new(handler);
        self.add(H::METHOD, H::PATTERN, move |request, params, connection, state| {
            handler.handle_async(request, params, connection, state)
        })
    }

    /// Register routes under `prefix` that share guards (18.35.0, #94);
    /// see [`group`](crate::group).
    pub fn group(mut self, prefix: &str, build: impl FnOnce(crate::group::AsyncGroup<S>) -> crate::group::AsyncGroup<S>) -> Self {
        for route in build(crate::group::AsyncGroup::new(prefix)).finish() {
            self = self.add_group_route(route);
        }
        self
    }

    /// Register the conventional routes of a resource at `path` (18.35.0,
    /// #94); see [`group`](crate::group).
    pub fn resource(mut self, path: &str, build: impl FnOnce(crate::group::AsyncResource<S>) -> crate::group::AsyncResource<S>) -> Self {
        for route in crate::group::AsyncGroup::new("").resource(path, build).finish() {
            self = self.add_group_route(route);
        }
        self
    }

    /// Name the route registered last, for
    /// [`url_for`](crate::router::url_for) (18.35.0, #94).
    pub fn name(self, name: &str) -> Self {
        if let Some(route) = self.routes.last() {
            crate::router::name_route(name, &matcher::segments_to_pattern(&route.segments));
        }
        self
    }

    fn add_group_route(self, route: crate::group::GroupRoute<crate::group::AsyncHandlerFn<S>>) -> Self {
        if let Some(name) = &route.name {
            crate::router::name_route(name, &route.pattern);
        }
        let handler = route.handler;
        self.add(route.method, &route.pattern, move |request, params, connection, state| handler(request, params, connection, state))
    }

    /// Wrap this application in a middleware layer (18.35.0), as
    /// [`AppWithState::wrap`](crate::state::AppWithState::wrap): the first
    /// layer wrapped is the outermost.
    pub fn wrap<M: crate::middleware::Middleware + 'static>(self, layer: M) -> crate::middleware::WithMiddleware<AsyncAppWithState<S>> {
        crate::middleware::WithMiddleware::new(self).wrap(layer)
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

    /// Add `GET /openapi.json` (a generated OpenAPI 3.1 document covering
    /// every route registered so far) and `GET /docs` (Swagger UI, loaded
    /// from a CDN, pointed at `/openapi.json`).
    ///
    /// Call this *after* registering your routes — routes added afterward
    /// still work but won't appear in the generated spec, since it's built
    /// once at this call rather than read dynamically per request.
    ///
    /// Requires the `openapi` feature (and `http2`, since `AsyncAppWithState`
    /// already requires it).
    #[cfg(feature = "openapi")]
    pub fn openapi(self, config: crate::openapi::OpenApiConfig) -> Self {
        let spec_json = Arc::new(crate::openapi::build_spec(&config, &self.route_entries()));
        let html = Arc::new(crate::openapi::swagger_ui_html("/openapi.json"));

        let spec_for_route = Arc::clone(&spec_json);
        self.get("/openapi.json", move |_req, _params, _conn, _state| {
            let spec_for_route = Arc::clone(&spec_for_route);
            async move {
                let mut r = Response::new();
                r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                r.content_range_list = vec![Range::get_content_range(
                    spec_for_route.as_bytes().to_vec(),
                    MimeType::APPLICATION_JSON.to_string(),
                )];
                r
            }
        })
        .get("/docs", move |_req, _params, _conn, _state| {
            let html = Arc::clone(&html);
            async move {
                let mut r = Response::new();
                r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                r.content_range_list = vec![Range::get_content_range(
                    html.as_bytes().to_vec(),
                    MimeType::TEXT_HTML.to_string(),
                )];
                r
            }
        })
    }

    /// The matching route's handler future, or `None` when no route matches.
    fn dispatch(&self, request: &Request, connection: &ConnectionInfo) -> Option<BoxFuture<Response>> {
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for route in &self.routes {
            if route.method != request.method {
                continue;
            }
            if let Some(params_map) = matcher::try_match(&route.segments, &path_segs) {
                let params = PathParams::from_map(params_map);
                return Some((route.handler)(request.clone(), params, connection.clone(), Arc::clone(&self.state)));
            }
        }
        None
    }

    /// Await the response on the caller's runtime (see the module docs for
    /// where the handler runs); unmatched routes go to the fallback `App`.
    pub(crate) async fn respond(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        match self.dispatch(request, connection) {
            Some(handler) => Ok(crate::async_bridge::run_async(handler).await),
            None => self.fallback_app().execute(request, connection),
        }
    }
}

impl<S: Send + Sync + 'static> Application for AsyncAppWithState<S> {
    /// Runs a matching handler on the shared `rws-async` runtime and waits
    /// for it; anything else goes to the fallback `App` on this thread.
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        match self.dispatch(request, connection) {
            Some(handler) => Ok(crate::async_bridge::block_on_shared(handler)),
            None => self.fallback_app().execute(request, connection),
        }
    }

    /// A matching route's handler, awaited by the async servers; `None` for
    /// the fallback `App` (it's synchronous).
    fn execute_async<'a>(&'a self, request: &'a Request, connection: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        let handler = self.dispatch(request, connection)?;
        Some(Box::pin(async move { Ok(crate::async_bridge::run_async(handler).await) }))
    }

    fn registered_routes(&self) -> Vec<crate::router::RouteInfo> {
        self.route_entries()
    }
}
