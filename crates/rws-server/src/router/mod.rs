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
    #[doc(hidden)]
    pub fn from_map(params: HashMap<String, String>) -> Self {
        PathParams { params }
    }

    /// Returns the value for the named parameter, or `None` if absent.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }
}

/// The host a request asked for: its `Host` header (HTTP/2 and HTTP/3 turn
/// `:authority` into one), else the TLS SNI name, else `""`. Host comes first
/// because browsers reuse one connection for every name its certificate covers
/// (RFC 9113 §9.1.1), so SNI names the connection's first request, not this one.
#[doc(hidden)]
pub fn request_host<'a>(request: &'a Request, connection: &'a ConnectionInfo) -> &'a str {
    request.headers.iter()
        .find(|h| h.name.eq_ignore_ascii_case("host"))
        .map(|h| h.value.as_ref())
        .filter(|v: &&str| !v.is_empty())
        .or(connection.sni_hostname.as_deref())
        .unwrap_or("")
}

/// A route's `host` against the request's ([`request_host`]): ignoring ASCII
/// case, and ignoring a port the route doesn't name (#50).
#[doc(hidden)]
pub fn host_matches(expected: &str, actual: &str) -> bool {
    let actual = if expected.contains(':') {
        actual
    } else if let Some(rest) = actual.strip_prefix('[') {
        // [IPv6]:port
        rest.split(']').next().unwrap_or(rest)
    } else {
        actual.rsplit_once(':').map_or(actual, |(host, port)| if port.bytes().all(|b| b.is_ascii_digit()) { host } else { actual })
    };
    let expected = expected.trim_start_matches('[').trim_end_matches(']');
    actual.eq_ignore_ascii_case(expected)
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
/// # extern crate rws_server as rust_web_server;
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

/// A route that never runs because one registered before it, with the same
/// method, matches every path it does — what
/// [`Router::route_conflicts`] (and `AppWithState`/`AsyncAppWithState`'s)
/// report (18.28.0). Patterns are in the `:name`/`*name` form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteConflict {
    pub method: String,
    /// The route that never runs.
    pub pattern: String,
    /// The earlier route that answers its requests.
    pub earlier: String,
    pub kind: RouteConflictKind,
}

/// How a [`RouteConflict`]'s earlier route hides the later one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteConflictKind {
    /// The same pattern, parameter names aside: registered twice.
    Duplicate,
    /// The earlier pattern is more general (a parameter or wildcard where
    /// the later one has a literal).
    Shadowed,
}

impl std::fmt::Display for RouteConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            RouteConflictKind::Duplicate => write!(
                f,
                "route {} {} is registered twice (first as {} {}); the second never runs",
                self.method, self.pattern, self.method, self.earlier
            ),
            RouteConflictKind::Shadowed => write!(
                f,
                "route {} {} never runs: {} {}, registered before it, matches every path it does",
                self.method, self.pattern, self.method, self.earlier
            ),
        }
    }
}

impl RouteConflict {
    /// Print the conflict to stderr, once per process for each distinct one.
    #[doc(hidden)]
    pub fn warn_once(&self) {
        use std::collections::HashSet;
        use std::sync::{Mutex, OnceLock};
        static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        let message = self.to_string();
        let fresh = SEEN
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .map(|mut seen| seen.insert(message.clone()))
            .unwrap_or(false);
        if fresh {
            eprintln!("rws: warning: {message}");
        }
    }
}

/// The conflicts among `routes`, in registration order.
#[doc(hidden)]
pub fn conflicts<'a>(routes: impl Iterator<Item = (&'a str, &'a [Segment])>) -> Vec<RouteConflict> {
    let all: Vec<(&str, &[Segment])> = routes.collect();
    (1..all.len())
        .filter_map(|i| conflict_with(all[..i].iter().copied(), all[i].0, all[i].1))
        .collect()
}

/// The first of `earlier` that hides a route `method` `segments` added after them.
pub(crate) fn conflict_with<'a>(
    earlier: impl Iterator<Item = (&'a str, &'a [Segment])>,
    method: &str,
    segments: &[Segment],
) -> Option<RouteConflict> {
    if matcher::never_matches(segments) {
        return None;
    }
    for (m, segs) in earlier {
        if m != method {
            continue;
        }
        let kind = if matcher::equivalent(segs, segments) {
            RouteConflictKind::Duplicate
        } else if matcher::covers(segs, segments) {
            RouteConflictKind::Shadowed
        } else {
            continue;
        };
        return Some(RouteConflict {
            method: method.to_string(),
            pattern: matcher::segments_to_pattern(segments),
            earlier: matcher::segments_to_pattern(segs),
            kind,
        });
    }
    None
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

    /// Restrict this router to requests for `host`: their `Host` header
    /// (`:authority` over HTTP/2 and HTTP/3), else the TLS SNI name, ignoring
    /// case and a port `host` doesn't name.  Call before registering routes.
    pub fn with_host(mut self, host: &str) -> Self {
        self.host = Some(host.to_string());
        self
    }

    /// Register a `GET` handler for `pattern`.
    pub fn get<F>(self, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.add("GET", pattern, handler)
    }

    /// Serve WebSocket connections at `pattern`: an upgrade request's
    /// connection goes to `handler` with the route's parameters, anything
    /// else gets `426 Upgrade Required` (see
    /// [`WebSocket::upgrade`](crate::websocket::WebSocket::upgrade)).
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// use rust_web_server::router::Router;
    /// use rust_web_server::websocket::Message;
    ///
    /// let router = Router::new().websocket("/rooms/:room", |mut ws, params| {
    ///     let room = params.get("room").unwrap_or("lobby").to_string();
    ///     while let Ok(Some(Message::Text(text))) = ws.recv() {
    ///         let _ = ws.send_text(&format!("{room}: {text}"));
    ///     }
    /// });
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn websocket<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(crate::websocket::WebSocketConnection, PathParams) + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        self.get(pattern, move |req, params, _| {
            let (handler, params) = (Arc::clone(&handler), params.clone());
            crate::websocket::WebSocket::upgrade(req, move |ws| handler(ws, params))
        })
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

    #[doc(hidden)]
    pub fn add<F>(self, method: &str, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        let router = self.add_unchecked(method, pattern, handler);
        if let Some(conflict) = router.last_route_conflict() {
            conflict.warn_once();
        }
        router
    }

    /// [`add`](Self::add) without the conflict warning, for routers that
    /// are only matched (`AppWithState::stream_body`).
    pub(crate) fn add_unchecked<F>(mut self, method: &str, pattern: &str, handler: F) -> Self
    where F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static {
        self.routes.push(Route {
            method: method.to_string(),
            segments: matcher::parse_pattern(pattern),
            handler: Arc::new(handler),
        });
        self
    }

    /// Register a typed handler (18.28.0): a function with extractor
    /// arguments annotated `#[get("/path")]` & co., whose method and pattern
    /// come from the attribute. A router has no state, so `State<S>` and
    /// `&S` arguments aren't available here — use
    /// [`AppWithState::route`](crate::state::AppWithState::route).
    ///
    /// ```rust,ignore
    /// #[get("/users/{id}")]
    /// fn user(Path(id): Path<u64>) -> String { format!("user {id}") }
    ///
    /// let router = Router::new().route(user);
    /// ```
    pub fn route<H: crate::handler::Handler<()>>(self, handler: H) -> Self {
        let unit = Arc::new(());
        self.add(H::METHOD, H::PATTERN, move |request, params, connection| {
            handler.handle(request, params, connection, &unit)
        })
    }

    /// Routes that never run (18.28.0): registered after a route with the
    /// same method whose pattern is equivalent (`/users/:id` and
    /// `/users/{uid}`) or matches every path they do (`/users/:id` before
    /// `/users/me`). Routes are tried in order, so the earlier one always
    /// wins. Each is also printed once to stderr when it is registered.
    pub fn route_conflicts(&self) -> Vec<RouteConflict> {
        conflicts(self.routes.iter().map(|r| (r.method.as_str(), r.segments.as_slice())))
    }

    fn last_route_conflict(&self) -> Option<RouteConflict> {
        let (last, earlier) = self.routes.split_last()?;
        conflict_with(earlier.iter().map(|r| (r.method.as_str(), r.segments.as_slice())), &last.method, &last.segments)
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
    /// request's host (`Host` header, else SNI) matches.
    pub fn handle(&self, request: &Request, connection: &ConnectionInfo) -> Option<Response> {
        let (route, params) = self.find(request, connection)?;
        Some((route.handler)(request, &params, connection))
    }

    /// `true` if [`handle`](Self::handle) would run a handler for `request`
    /// (method, path and host all match), without running it.
    pub fn matches(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        self.find(request, connection).is_some()
    }

    fn find(&self, request: &Request, connection: &ConnectionInfo) -> Option<(&Route, PathParams)> {
        if let Some(required_host) = &self.host {
            if !host_matches(required_host, request_host(request, connection)) {
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
                return Some((route, PathParams::from_map(params)));
            }
        }
        None
    }
}
