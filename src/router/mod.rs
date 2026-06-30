#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Named path-segment values extracted from a matched route pattern.
///
/// Given the pattern `/users/:id/posts/:post_id` matched against
/// `/users/42/posts/7`, `params.get("id")` returns `Some("42")` and
/// `params.get("post_id")` returns `Some("7")`.
///
/// Wildcard segments (`*name`) capture everything after the prefix:
/// `/files/*path` matched against `/files/a/b/c` gives `path = "a/b/c"`.
pub struct PathParams {
    params: HashMap<String, String>,
}

impl PathParams {
    fn new() -> Self {
        PathParams { params: HashMap::new() }
    }

    /// Build a `PathParams` from an existing map. Used by `AsyncAppWithState`.
    pub(crate) fn from_map(params: HashMap<String, String>) -> Self {
        PathParams { params }
    }

    /// Returns the value for the named parameter, or `None` if absent.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    fn insert(&mut self, key: String, value: String) {
        self.params.insert(key, value);
    }
}

enum Segment {
    Literal(String),
    Param(String),
    Wildcard(String),
}

type HandlerFn =
    Box<dyn Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static>;

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
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new() }
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
            segments: Self::parse_pattern(pattern),
            handler: Box::new(handler),
        });
        self
    }

    fn parse_pattern(pattern: &str) -> Vec<Segment> {
        if pattern == "/" {
            return vec![];
        }
        pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|seg| {
                if let Some(name) = seg.strip_prefix(':') {
                    Segment::Param(name.to_string())
                } else if let Some(name) = seg.strip_prefix('*') {
                    Segment::Wildcard(name.to_string())
                } else {
                    Segment::Literal(seg.to_string())
                }
            })
            .collect()
    }

    /// Try to match `request` against registered routes in registration order.
    ///
    /// Returns `Some(response)` on the first match, `None` if no route matches.
    /// The query string is stripped before matching; only the path is used.
    pub fn handle(&self, request: &Request, connection: &ConnectionInfo) -> Option<Response> {
        let path = request.request_uri.split('?').next().unwrap_or(&request.request_uri);
        let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for route in &self.routes {
            if route.method != request.method {
                continue;
            }
            if let Some(params) = Self::try_match(&route.segments, &path_segs) {
                return Some((route.handler)(request, &params, connection));
            }
        }
        None
    }

    fn try_match(pattern: &[Segment], path: &[&str]) -> Option<PathParams> {
        let mut params = PathParams::new();
        let mut pi = 0;

        for (si, seg) in pattern.iter().enumerate() {
            match seg {
                Segment::Literal(lit) => {
                    if pi >= path.len() || path[pi] != lit.as_str() {
                        return None;
                    }
                    pi += 1;
                }
                Segment::Param(name) => {
                    if pi >= path.len() {
                        return None;
                    }
                    params.insert(name.clone(), path[pi].to_string());
                    pi += 1;
                }
                Segment::Wildcard(name) => {
                    if si != pattern.len() - 1 {
                        return None; // wildcard must be the last segment
                    }
                    params.insert(name.clone(), path[pi..].join("/"));
                    pi = path.len();
                }
            }
        }

        if pi == path.len() { Some(params) } else { None }
    }
}
