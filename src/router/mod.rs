pub(crate) mod matcher;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;
use matcher::Segment;

/// Named path-segment values extracted from a matched route pattern.
///
/// Given the pattern `/users/:id/posts/:post_id` matched against
/// `/users/42/posts/7`, `params.get("id")` returns `Some("42")` and
/// `params.get("post_id")` returns `Some("7")`.
///
/// Wildcard segments (`*name`) capture everything after the prefix:
/// `/files/*path` matched against `/files/a/b/c` gives `path = "a/b/c"`.
#[derive(Clone)]
pub struct PathParams {
    params: HashMap<String, String>,
}

impl PathParams {
    /// Build a `PathParams` from an existing map — used to adapt
    /// [`matcher::try_match`]'s output for both `Router` and `AsyncAppWithState`.
    pub(crate) fn from_map(params: HashMap<String, String>) -> Self {
        PathParams { params }
    }

    /// Returns the value for the named parameter, or `None` if absent.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }
}

type HandlerFn =
    Arc<dyn Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static>;

#[derive(Clone)]
struct Route {
    method: String,
    segments: Vec<Segment>,
    handler: HandlerFn,
}

/// A path-based HTTP router with named parameter extraction.
///
/// Register routes with [`Router::get`], [`Router::post`], etc. Each handler
/// receives the parsed [`PathParams`] alongside the raw [`Request`] and
/// [`ConnectionInfo`]. Call [`Router::handle`] from inside a [`Controller`]
/// or an [`Application::execute`] implementation.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::router::{Router, PathParams};
/// use rust_web_server::request::Request;
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
/// use rust_web_server::range::Range;
/// use rust_web_server::mime_type::MimeType;
/// use rust_web_server::server::ConnectionInfo;
/// use rust_web_server::core::New;
///
/// let router = Router::new()
///     .get("/hello", |_req, _params, _conn| {
///         let mut r = Response::new();
///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///         r.content_range_list = vec![Range::get_content_range(b"hello".to_vec(), MimeType::TEXT_PLAIN.to_string())];
///         r
///     })
///     .get("/users/:id", |_req, params, _conn| {
///         let id = params.get("id").unwrap_or("unknown");
///         let mut r = Response::new();
///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///         r.content_range_list = vec![Range::get_content_range(
///             format!("user {}", id).into_bytes(),
///             MimeType::TEXT_PLAIN.to_string(),
///         )];
///         r
///     });
/// ```
/// A registered route entry returned by [`Router::route_entries`].
#[derive(Clone)]
pub struct RouteInfo {
    pub method: String,
    pub pattern: String,
}

#[derive(Clone)]
pub struct Router {
    routes: Vec<Route>,
    /// When set, `handle()` only matches if the request's SNI hostname (or
    /// `Host` header for plain HTTP) equals this value.
    host: Option<String>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new(), host: None }
    }

    /// Restrict this router to requests whose SNI hostname (TLS) or `Host`
    /// header (plain HTTP) matches `host`.  Call before registering routes.
    pub fn with_host(mut self, host: &str) -> Self {
        self.host = Some(host.to_string());
        self
    }

    /// Register a `GET` handler for `pattern`.
    pub fn get<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("GET", pattern, handler)
    }

    /// Register a `POST` handler for `pattern`.
    pub fn post<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("POST", pattern, handler)
    }

    /// Register a `PUT` handler for `pattern`.
    pub fn put<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("PUT", pattern, handler)
    }

    /// Register a `PATCH` handler for `pattern`.
    pub fn patch<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("PATCH", pattern, handler)
    }

    /// Register a `DELETE` handler for `pattern`.
    pub fn delete<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("DELETE", pattern, handler)
    }

    fn add<F>(mut self, method: &str, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.routes.push(Route {
            method: method.to_string(),
            segments: matcher::parse_pattern(pattern),
            handler: Arc::new(handler),
        });
        self
    }

    /// Return a snapshot of all registered routes as `(method, pattern)` pairs.
    ///
    /// Patterns are reconstructed from parsed segments, so the output exactly
    /// matches what was passed to `.get()`, `.post()`, etc. at registration time.
    pub fn route_entries(&self) -> Vec<RouteInfo> {
        self.routes.iter().map(|r| RouteInfo {
            method: r.method.clone(),
            pattern: matcher::segments_to_pattern(&r.segments),
        }).collect()
    }

    /// Try to match `request` against registered routes in registration order.
    ///
    /// Returns `Some(response)` on the first match, `None` if no route matches.
    /// The query string is stripped before matching; only the path is used.
    ///
    /// When `.with_host()` is set, this returns `None` immediately unless the
    /// request's SNI hostname (TLS) or `Host` header (plain HTTP) matches.
    pub fn handle(&self, request: &Request, connection: &ConnectionInfo) -> Option<Response> {
        if let Some(required_host) = &self.host {
            let actual = connection.sni_hostname.as_deref().or_else(|| {
                request.headers.iter()
                    .find(|h| h.name.eq_ignore_ascii_case("host"))
                    .map(|h| h.value.as_str())
            });
            if actual != Some(required_host.as_str()) {
                return None;
            }
        }

        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for route in &self.routes {
            if route.method != request.method {
                continue;
            }
            if let Some(params) = matcher::try_match(&route.segments, &path_segs) {
                let params = PathParams::from_map(params);
                return Some((route.handler)(request, &params, connection));
            }
        }
        None
    }
}
