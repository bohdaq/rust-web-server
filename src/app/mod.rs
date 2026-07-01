#[cfg(test)]
mod tests;

pub mod controller;

use crate::app::controller::favicon::FaviconController;
use crate::app::controller::health::HealthController;
use crate::app::controller::ready::ReadyController;
use crate::app::controller::metrics::MetricsController;
use crate::app::controller::file::initiate::FileUploadInitiateController;
use crate::app::controller::form::get_method::FormGetMethodController;
use crate::app::controller::form::multipart_enctype_post_method::FormMultipartEnctypePostMethodController;
use crate::app::controller::form::url_encoded_enctype_post_method::FormUrlEncodedEnctypePostMethodController;
use crate::app::controller::index::IndexController;
use crate::app::controller::not_found::NotFoundController;
use crate::app::controller::script::ScriptController;
use crate::app::controller::static_resource::StaticResourceController;
use crate::app::controller::style::StyleController;
use crate::application::Application;
use crate::controller::Controller;
use crate::core::New;
use crate::header::Header;
use crate::mcp::McpServer;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::state::AppWithState;

/// A pair of function pointers representing one entry in the controller chain.
struct ControllerEntry {
    is_matching: fn(&Request, &ConnectionInfo) -> bool,
    process: fn(&Request, Response, &ConnectionInfo) -> Response,
}

/// Build a [`ControllerEntry`] from any type that implements [`Controller`].
fn entry<C: Controller>() -> ControllerEntry {
    ControllerEntry {
        is_matching: C::is_matching,
        process: C::process,
    }
}

/// The built-in HTTP application. Serves static files, favicons, forms,
/// file uploads, health probes, metrics, and a 404 fallback.
///
/// Use as-is or compose with the framework's building blocks:
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::middleware::{WithMiddleware, RateLimitLayer};
/// use rust_web_server::core::New;
///
/// // Middleware stack around the built-in app
/// let app = App::new().wrap(RateLimitLayer);
/// ```
///
/// For user-defined routes with shared state, call [`App::with_state`]:
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
/// use rust_web_server::core::New;
///
/// struct State { version: &'static str }
///
/// let app = App::with_state(State { version: "1.0" })
///     .get("/version", |_req, _params, _conn, state| {
///         let mut r = Response::new();
///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///         r
///     });
/// ```
#[derive(Copy, Clone)]
pub struct App {}

impl New for App {
    fn new() -> Self {
        App{}
    }
}

impl Application for App {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let header_list = Header::get_header_list(request);
        let response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            Some(header_list),
            None,
        );

        let controllers = [
            entry::<IndexController>(),
            entry::<StyleController>(),
            entry::<ScriptController>(),
            entry::<FileUploadInitiateController>(),
            entry::<FormUrlEncodedEnctypePostMethodController>(),
            entry::<FormGetMethodController>(),
            entry::<FormMultipartEnctypePostMethodController>(),
            entry::<HealthController>(),
            entry::<ReadyController>(),
            entry::<MetricsController>(),
            entry::<FaviconController>(),
            entry::<StaticResourceController>(),
            entry::<NotFoundController>(),
        ];

        for c in &controllers {
            if (c.is_matching)(request, connection) {
                return Ok((c.process)(request, response, connection));
            }
        }

        Ok(response)
    }
}

impl App {
    /// Dispatch `request` through the controller chain and return the response.
    ///
    /// This is a convenience wrapper over [`Application::execute`] that uses a
    /// synthetic loopback [`ConnectionInfo`]. Use it in tests or when no real
    /// connection context is available. Prefer [`TestClient`] for structured
    /// test code.
    ///
    /// [`TestClient`]: crate::test_client::TestClient
    pub fn handle_request(request: Request) -> (Response, Request) {
        use crate::server::Address;
        let conn = ConnectionInfo {
            client: Address { ip: "127.0.0.1".to_string(), port: 0 },
            server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
            request_size: 16000,
            sni_hostname: None,
        };
        let app = App::new();
        let response = app.execute(&request, &conn).unwrap_or_else(|_| {
            let header_list = Header::get_header_list(&request);
            Response::get_response(
                STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                Some(header_list),
                None,
            )
        });
        (response, request)
    }

    /// Create a state-aware application. Routes registered on the returned
    /// [`AppWithState<S>`] are tried first; unmatched requests fall through to
    /// the built-in controller chain (static files, health probes, etc.).
    ///
    /// The state is stored as `Arc<S>` and shared across all handlers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    /// use rust_web_server::core::New;
    ///
    /// struct Db { url: String }
    ///
    /// let app = App::with_state(Db { url: "postgres://...".to_string() })
    ///     .get("/ping", |_req, _params, _conn, db| {
    ///         let mut r = Response::new();
    ///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    ///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    ///         r
    ///     });
    /// ```
    pub fn with_state<S: Send + Sync + 'static>(state: S) -> AppWithState<S> {
        AppWithState::new(state)
    }

    /// Wrap this application in a middleware layer.
    ///
    /// Returns a [`WithMiddleware<App>`] that runs `layer` before every
    /// request. Chain `.wrap()` calls to stack multiple layers:
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::middleware::RateLimitLayer;
    /// use rust_web_server::core::New;
    ///
    /// let app = App::new().wrap(RateLimitLayer);
    /// ```
    pub fn wrap<M: Middleware + 'static>(self, layer: M) -> WithMiddleware<App> {
        WithMiddleware::new(self).wrap(layer)
    }

    /// Attach an MCP server to this application. Tools, resources, and
    /// prompts are registered on the returned [`McpServer`]; requests that
    /// do not match the MCP endpoint are forwarded to `self` (static files,
    /// health probes, any custom routes registered before this call).
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::mcp::{McpContent, extract_arg};
    /// use rust_web_server::core::New;
    ///
    /// // Pure MCP — unmatched paths handled by built-in App
    /// let app = App::new()
    ///     .mcp("my-server", "1.0")
    ///     .tool(
    ///         "echo",
    ///         "Echo text back",
    ///         r#"{"type":"object","properties":{"text":{"type":"string"}}}"#,
    ///         |args| Ok(McpContent::text(extract_arg(args, "text").unwrap_or_default())),
    ///     );
    /// ```
    ///
    /// To combine with custom HTTP routes, start from [`App::with_state`]:
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::mcp::McpContent;
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    /// use rust_web_server::core::New;
    ///
    /// let app = App::with_state(())
    ///     .get("/api/ping", |_, _, _, _| {
    ///         let mut r = Response::new();
    ///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    ///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    ///         r
    ///     })
    ///     .mcp("my-server", "1.0")
    ///     .tool("ping", "Ping the server", "{}", |_| Ok(McpContent::text("pong")));
    /// ```
    pub fn mcp(self, name: impl Into<String>, version: impl Into<String>) -> McpServer {
        McpServer::new(name, version).wrap(self)
    }

    /// Create an async state-aware application (requires the `http2` feature).
    ///
    /// Handlers are `async fn` closures that can `await` database queries,
    /// HTTP clients, or any other async I/O. Unmatched routes fall through to
    /// the built-in controller chain.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use rust_web_server::app::App;
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    /// use rust_web_server::core::New;
    ///
    /// struct Db { url: String }
    ///
    /// let app = App::with_async_state(Db { url: "postgres://...".to_string() })
    ///     .get("/ping", |_req, _params, _conn, state| async move {
    ///         // state: Arc<Db>
    ///         let mut r = Response::new();
    ///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    ///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    ///         r
    ///     });
    /// ```
    #[cfg(feature = "http2")]
    pub fn with_async_state<S: Send + Sync + 'static>(state: S) -> crate::async_state::AsyncAppWithState<S> {
        crate::async_state::AsyncAppWithState::new(state)
    }
}