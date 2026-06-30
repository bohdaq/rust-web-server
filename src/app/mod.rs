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
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::state::AppWithState;

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
        let header_list = Header::get_header_list(&request);

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            Some(header_list),
            None
        );



        if IndexController::is_matching(&request, connection) {
            response = IndexController::process(&request, response, connection);
            return Ok(response)
        }

        if StyleController::is_matching(&request, connection) {
            response = StyleController::process(&request, response, connection);
            return Ok(response)
        }

        if ScriptController::is_matching(&request, connection) {
            response = ScriptController::process(&request, response, connection);
            return Ok(response)
        }

        if FileUploadInitiateController::is_matching(&request, connection) {
            response = FileUploadInitiateController::process(&request, response, connection);
            return Ok(response)
        }

        if FormUrlEncodedEnctypePostMethodController::is_matching(&request, connection) {
            response = FormUrlEncodedEnctypePostMethodController::process(&request, response, connection);
            return Ok(response)
        }

        if FormGetMethodController::is_matching(&request, connection) {
            response = FormGetMethodController::process(&request, response, connection);
            return Ok(response)
        }

        if FormMultipartEnctypePostMethodController::is_matching(&request, connection) {
            response = FormMultipartEnctypePostMethodController::process(&request, response, connection);
            return Ok(response)
        }

        if HealthController::is_matching(&request, connection) {
            response = HealthController::process(&request, response, connection);
            return Ok(response)
        }

        if ReadyController::is_matching(&request, connection) {
            response = ReadyController::process(&request, response, connection);
            return Ok(response)
        }

        if MetricsController::is_matching(&request, connection) {
            response = MetricsController::process(&request, response, connection);
            return Ok(response)
        }

        if FaviconController::is_matching(&request, connection) {
            response = FaviconController::process(&request, response, connection);
            return Ok(response)
        }

        if StaticResourceController::is_matching(&request, connection) {
            response = StaticResourceController::process(&request, response, connection);
            return Ok(response)
        }

        if NotFoundController::is_matching(&request, connection) {
            response = NotFoundController::process(&request, response, connection);
            return Ok(response)
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
}