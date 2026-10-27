//! HTTPRoute filters applied by the router: header modifiers (request,
//! response, per backend), `RequestRedirect`, `URLRewrite` and `CORS`.
//! Pure functions over the request/response, so each is tested on its own.

use crate::core::New;
use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

use super::router::prefix_matches;
use super::types::*;

/// `set` replaces every value of a header, `add` appends one more,
/// `remove` drops it — names compared case-insensitively.
pub fn modify_headers(headers: &mut Vec<Header>, filter: &HttpHeaderFilter) {
    for name in &filter.remove {
        headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
    }
    for header in &filter.set {
        headers.retain(|h| !h.name.eq_ignore_ascii_case(&header.name));
        headers.push(Header::new(header.name.clone(), header.value.clone()));
    }
    for header in &filter.add {
        headers.push(Header::new(header.name.clone(), header.value.clone()));
    }
}

/// The path after a `ReplaceFullPath` / `ReplacePrefixMatch` modifier.
/// `matched` is the rule match that selected the route: a prefix
/// replacement swaps its `PathPrefix` value (whole segments) for the
/// replacement, so `/prefix/one` → `/one` maps `/prefix/one/two` to `/one/two`.
pub fn modified_path(path: &str, matched: &HttpRouteMatch, modifier: &HttpPathModifier) -> String {
    match modifier.modifier_type.as_str() {
        "ReplaceFullPath" => modifier.replace_full_path.clone().unwrap_or_else(|| "/".to_string()),
        "ReplacePrefixMatch" => {
            let prefix = matched.path.as_ref().and_then(|p| p.value.as_deref()).unwrap_or("/");
            if !prefix_matches(prefix, path) {
                return path.to_string();
            }
            // Whole segments matched, so the rest is empty or starts with `/`.
            let rest = &path[prefix.trim_end_matches('/').len()..];
            let replacement = modifier.replace_prefix_match.as_deref().unwrap_or("/").trim_end_matches('/');
            let joined = format!("{replacement}{rest}");
            match joined.as_str() {
                "" => "/".to_string(),
                j if !j.starts_with('/') => format!("/{j}"),
                _ => joined,
            }
        }
        _ => path.to_string(),
    }
}

/// Splits `request_uri` into the path and the `?query` (with its `?`).
pub fn split_target(uri: &str) -> (&str, &str) {
    match uri.find('?') {
        Some(i) => (&uri[..i], &uri[i..]),
        None => (uri, ""),
    }
}

/// The well-known port of a scheme.
fn default_port(scheme: &str) -> u16 {
    if scheme.eq_ignore_ascii_case("https") { 443 } else { 80 }
}

/// The redirect response. Scheme defaults to the listener's, host to the
/// request's, port to the scheme's well-known port when the scheme is set
/// and otherwise to the listener's; a well-known port isn't written.
pub fn redirect(request: &Request, host: &str, listener_https: bool, listener_port: u16, matched: &HttpRouteMatch, filter: &HttpRequestRedirectFilter) -> Response {
    let scheme = filter.scheme.clone().unwrap_or_else(|| if listener_https { "https" } else { "http" }.to_string());
    let hostname = filter.hostname.clone().unwrap_or_else(|| host.to_string());
    let port = match (filter.port, &filter.scheme) {
        (Some(port), _) => port as u16,
        (None, Some(scheme)) => default_port(scheme),
        (None, None) => listener_port,
    };
    let (path, query) = split_target(&request.request_uri);
    let path = filter.path.as_ref().map(|m| modified_path(path, matched, m)).unwrap_or_else(|| path.to_string());
    let authority = if port == default_port(&scheme) { hostname } else { format!("{hostname}:{port}") };
    let code = match filter.status_code.unwrap_or(302) {
        301 => STATUS_CODE_REASON_PHRASE.n301_moved_permanently,
        303 => STATUS_CODE_REASON_PHRASE.n303_see_other,
        307 => STATUS_CODE_REASON_PHRASE.n307_temporary_redirect,
        308 => STATUS_CODE_REASON_PHRASE.n308_permanent_redirect,
        _ => STATUS_CODE_REASON_PHRASE.n302_found,
    };
    let mut response = status(code);
    response.headers.push(Header::new("Location", format!("{scheme}://{authority}{path}{query}")));
    response
}

pub(crate) fn status(code: &'static crate::response::StatusCodeReasonPhrase) -> Response {
    let mut response = Response::new();
    response.status_code = *code.status_code;
    response.reason_phrase = code.reason_phrase.to_string();
    response
}

/// Whether an `allowOrigins` entry admits `origin`: `*`, exact
/// (case-insensitive), or a `*` in the host matching one or more labels.
fn origin_allowed(pattern: &str, origin: &str) -> bool {
    if pattern == "*" || pattern.eq_ignore_ascii_case(origin) {
        return true;
    }
    match pattern.split_once('*') {
        Some((before, after)) => {
            origin.len() > before.len() + after.len()
                && origin[..before.len()].eq_ignore_ascii_case(before)
                && origin[origin.len() - after.len()..].eq_ignore_ascii_case(after)
        }
        None => false,
    }
}

/// What the CORS filter does with a request.
#[derive(Debug, PartialEq)]
pub enum Cors {
    /// Not a cross-origin request, or one from an origin that isn't allowed:
    /// pass it on and add nothing.
    Pass,
    /// A preflight: answer it here with these headers (none when the origin
    /// isn't allowed).
    Preflight(Vec<Header>),
    /// An actual request: forward it and add these to the response.
    Actual(Vec<Header>),
}

pub fn cors(request: &Request, filter: &HttpCorsFilter) -> Cors {
    let Some(origin) = request.header("origin").map(|h| h.value.to_string()) else { return Cors::Pass };
    let preflight = request.method == "OPTIONS" && request.header("access-control-request-method").is_some();
    let allowed = filter.allow_origins.iter().any(|p| origin_allowed(p, &origin));
    if !allowed {
        return if preflight { Cors::Preflight(Vec::new()) } else { Cors::Pass };
    }
    let credentials = filter.allow_credentials == Some(true);
    // With credentials a wildcard can't be echoed literally.
    let wildcard = !credentials && filter.allow_origins.iter().any(|p| p == "*");
    let mut headers = vec![Header::new("Access-Control-Allow-Origin", if wildcard { "*".to_string() } else { origin })];
    if !wildcard {
        headers.push(Header::new("Vary", "Origin"));
    }
    if credentials {
        headers.push(Header::new("Access-Control-Allow-Credentials", "true"));
    }
    if !preflight {
        if !filter.expose_headers.is_empty() {
            headers.push(Header::new("Access-Control-Expose-Headers", filter.expose_headers.join(", ")));
        }
        return Cors::Actual(headers);
    }
    let requested = |name: &str| request.header(name).map(|h| h.value.to_string()).unwrap_or_default();
    let methods = if filter.allow_methods.iter().any(|m| m == "*") {
        if credentials { requested("access-control-request-method") } else { "*".to_string() }
    } else {
        filter.allow_methods.join(", ")
    };
    if !methods.is_empty() {
        headers.push(Header::new("Access-Control-Allow-Methods", methods));
    }
    let allow_headers = if filter.allow_headers.iter().any(|h| h == "*") {
        if credentials { requested("access-control-request-headers") } else { "*".to_string() }
    } else {
        filter.allow_headers.join(", ")
    };
    if !allow_headers.is_empty() {
        headers.push(Header::new("Access-Control-Allow-Headers", allow_headers));
    }
    if !filter.expose_headers.is_empty() {
        headers.push(Header::new("Access-Control-Expose-Headers", filter.expose_headers.join(", ")));
    }
    let max_age = if filter.max_age > 0 { filter.max_age } else { 5 };
    headers.push(Header::new("Access-Control-Max-Age", max_age.to_string()));
    Cors::Preflight(headers)
}

#[cfg(test)]
mod tests;
