//! Named routes and URL building (18.35.0, #94): `.name("post")` after a
//! route, then [`url_for`]`("post", &[("id", "7")])` → `/posts/7`.
//!
//! Names are process-wide, like the route table an app builds once at
//! start: registering a name again replaces it (the last app built wins).

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::matcher::{parse_pattern, Segment};

fn registry() -> &'static RwLock<HashMap<String, String>> {
    static NAMES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    NAMES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Give the route `pattern` the name `name`, for [`url_for`]. The app
/// builders call it from their `.name(…)`; call it directly for routes
/// registered some other way.
pub fn name_route(name: &str, pattern: &str) {
    if let Ok(mut names) = registry().write() {
        names.insert(name.to_string(), pattern.to_string());
    }
}

/// The pattern registered under `name`, as it was given (`/posts/{id}`).
pub fn named_pattern(name: &str) -> Option<String> {
    registry().read().ok()?.get(name).cloned()
}

/// Every named route as `(name, pattern)`, sorted by name.
pub fn named_routes() -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = match registry().read() {
        Ok(names) => names.iter().map(|(n, p)| (n.clone(), p.clone())).collect(),
        Err(_) => Vec::new(),
    };
    all.sort();
    all
}

/// The path of the route named `name` with its parameters filled in from
/// `params`; parameters the pattern doesn't have become the query string,
/// in the order given. Values are percent-encoded (a wildcard keeps its
/// `/`s). `None` when no route has that name or a parameter of the
/// pattern is missing.
///
/// ```rust
/// # extern crate rws_server as rust_web_server;
/// use rust_web_server::router::{name_route, url_for};
///
/// name_route("doc_post", "/posts/{id}/comments");
/// assert_eq!(Some("/posts/7/comments?page=2".to_string()), url_for("doc_post", &[("id", "7"), ("page", "2")]));
/// assert_eq!(None, url_for("doc_post", &[]));
/// ```
pub fn url_for(name: &str, params: &[(&str, &str)]) -> Option<String> {
    build_path(&named_pattern(name)?, params)
}

/// [`url_for`] on a pattern rather than a name.
pub fn build_path(pattern: &str, params: &[(&str, &str)]) -> Option<String> {
    let segments = parse_pattern(pattern);
    let mut used = Vec::new();
    let mut path = String::new();
    for segment in &segments {
        path.push('/');
        match segment {
            Segment::Literal(text) => path.push_str(text),
            Segment::Param(name) => {
                let value = params.iter().find(|(k, _)| k == name)?.1;
                if value.is_empty() {
                    return None;
                }
                path.push_str(&encode(value, false));
                used.push(name.as_str());
            }
            Segment::Wildcard(name) => {
                let value = params.iter().find(|(k, _)| k == name)?.1;
                path.push_str(&encode(value.trim_start_matches('/'), true));
                used.push(name.as_str());
            }
        }
    }
    if path.is_empty() {
        path.push('/');
    }
    let query: Vec<String> = params
        .iter()
        .filter(|(k, _)| !used.contains(k))
        .map(|(k, v)| format!("{}={}", encode(k, false), encode(v, false)))
        .collect();
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    Some(path)
}

/// Percent-encode everything but RFC 3986 unreserved characters (and `/`
/// when `keep_slash`).
pub(crate) fn encode(value: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') || (keep_slash && byte == b'/');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
