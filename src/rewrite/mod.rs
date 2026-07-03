//! Request and response rewriting middleware.
//!
//! [`RewriteLayer`] is a [`Middleware`] that transforms requests before they
//! reach handlers and responses before they leave the server. Build one with the
//! fluent builder API and add it to any [`crate::middleware::WithMiddleware`] stack.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::rewrite::RewriteLayer;
//!
//! let app = App::new()
//!     .wrap(RewriteLayer::new()
//!         .request_header_set("X-Env", "production")
//!         .request_uri_strip_prefix("/api/v1")
//!         .response_header_set("Cache-Control", "no-store")
//!         .response_body_replace("http://staging.internal", "https://example.com"));
//! ```
//!
//! # Regex URI rewriting (`rewrite-regex` feature)
//!
//! The prefix/set operations above cover fixed strings. When the rewrite
//! depends on part of the incoming path — versioning schemes, locale
//! prefixes, ID extraction — [`RewriteLayer::request_uri_regex_rewrite`]
//! matches the URI against a regex and rewrites it using the match's capture
//! groups, the same `rewrite` semantics as nginx: if the pattern matches
//! anywhere in the URI, the **entire** URI is replaced by the expanded
//! replacement string; otherwise the URI is left untouched.
//!
//! ```rust,no_run
//! # #[cfg(feature = "rewrite-regex")]
//! # fn example() -> Result<(), regex::Error> {
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::rewrite::RewriteLayer;
//!
//! let app = App::new()
//!     .wrap(RewriteLayer::new()
//!         .request_uri_regex_rewrite(r"^/api/v\d+/(.*)$", "/$1")?);
//! # Ok(())
//! # }
//! ```
//!
//! Requires the `rewrite-regex` feature (adds the `regex` crate) — this is
//! the one place in `rws` a third-party regex engine is worth the
//! dependency; hand-rolling one is out of scope for this crate's "no
//! third-party HTTP dependencies" philosophy, which doesn't extend to
//! general-purpose text processing.

#[cfg(test)]
mod tests;

use crate::application::Application;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

#[cfg(feature = "rewrite-regex")]
use regex::Regex;

enum RequestRule {
    SetHeader { name: String, value: String },
    RemoveHeader(String),
    SetUri(String),
    StripUriPrefix(String),
    AddUriPrefix(String),
    #[cfg(feature = "rewrite-regex")]
    RewriteUri { pattern: Regex, replacement: String },
}

enum ResponseRule {
    SetHeader { name: String, value: String },
    RemoveHeader(String),
    SetStatus { code: i16, reason: String },
    BodyReplace { from: Vec<u8>, to: Vec<u8> },
}

/// Composable request/response rewriting middleware.
///
/// Clones the incoming [`Request`], applies request rules, dispatches to the
/// next handler, then applies response rules on the returned [`Response`].
///
/// All builder methods take `self` by value and return `Self` for chaining.
pub struct RewriteLayer {
    request_rules: Vec<RequestRule>,
    response_rules: Vec<ResponseRule>,
}

impl RewriteLayer {
    pub fn new() -> Self {
        RewriteLayer { request_rules: Vec::new(), response_rules: Vec::new() }
    }

    /// Add or replace a request header (case-insensitive name match).
    pub fn request_header_set(mut self, name: &str, value: &str) -> Self {
        self.request_rules.push(RequestRule::SetHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        self
    }

    /// Remove a request header (case-insensitive).
    pub fn request_header_remove(mut self, name: &str) -> Self {
        self.request_rules.push(RequestRule::RemoveHeader(name.to_string()));
        self
    }

    /// Replace the entire request URI.
    pub fn request_uri_set(mut self, uri: &str) -> Self {
        self.request_rules.push(RequestRule::SetUri(uri.to_string()));
        self
    }

    /// Strip a path prefix from the request URI. No-op if the prefix is absent.
    /// Normalizes to `"/"` if stripping leaves an empty path.
    pub fn request_uri_strip_prefix(mut self, prefix: &str) -> Self {
        self.request_rules.push(RequestRule::StripUriPrefix(prefix.to_string()));
        self
    }

    /// Prepend a prefix to the request URI.
    pub fn request_uri_add_prefix(mut self, prefix: &str) -> Self {
        self.request_rules.push(RequestRule::AddUriPrefix(prefix.to_string()));
        self
    }

    /// Rewrite the request URI by regex, nginx `rewrite`-directive style.
    ///
    /// If `pattern` matches anywhere in the URI, the **entire** URI is
    /// replaced by `replacement` with capture-group references expanded —
    /// `$1`, `$2`, ... for numbered groups, `${name}` for named groups
    /// (`(?P<name>...)`), or `$0`/`${0}` for the whole match. If `pattern`
    /// does not match, the URI is left unchanged.
    ///
    /// Returns `Err` if `pattern` is not a valid regex. Requires the
    /// `rewrite-regex` feature.
    #[cfg(feature = "rewrite-regex")]
    pub fn request_uri_regex_rewrite(mut self, pattern: &str, replacement: &str) -> Result<Self, regex::Error> {
        let compiled = Regex::new(pattern)?;
        self.request_rules.push(RequestRule::RewriteUri {
            pattern: compiled,
            replacement: replacement.to_string(),
        });
        Ok(self)
    }

    /// Add or replace a response header (case-insensitive name match).
    pub fn response_header_set(mut self, name: &str, value: &str) -> Self {
        self.response_rules.push(ResponseRule::SetHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        self
    }

    /// Remove a response header (case-insensitive).
    pub fn response_header_remove(mut self, name: &str) -> Self {
        self.response_rules.push(ResponseRule::RemoveHeader(name.to_string()));
        self
    }

    /// Override the response status code and reason phrase.
    pub fn response_status(mut self, code: i16, reason: &str) -> Self {
        self.response_rules.push(ResponseRule::SetStatus { code, reason: reason.to_string() });
        self
    }

    /// Byte-level find-and-replace across all response body content ranges.
    pub fn response_body_replace(mut self, from: &str, to: &str) -> Self {
        self.response_rules.push(ResponseRule::BodyReplace {
            from: from.as_bytes().to_vec(),
            to: to.as_bytes().to_vec(),
        });
        self
    }
}

impl Middleware for RewriteLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let mut req = request.clone();

        for rule in &self.request_rules {
            match rule {
                RequestRule::SetHeader { name, value } => {
                    req.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                    req.headers.push(Header { name: name.clone(), value: value.clone() });
                }
                RequestRule::RemoveHeader(name) => {
                    req.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                }
                RequestRule::SetUri(uri) => {
                    req.request_uri = uri.clone();
                }
                RequestRule::StripUriPrefix(prefix) => {
                    if let Some(stripped) = req.request_uri.strip_prefix(prefix.as_str()) {
                        req.request_uri = if stripped.is_empty() || !stripped.starts_with('/') {
                            format!("/{}", stripped)
                        } else {
                            stripped.to_string()
                        };
                    }
                }
                RequestRule::AddUriPrefix(prefix) => {
                    req.request_uri = format!("{}{}", prefix, req.request_uri);
                }
                #[cfg(feature = "rewrite-regex")]
                RequestRule::RewriteUri { pattern, replacement } => {
                    if let Some(captures) = pattern.captures(&req.request_uri) {
                        let mut expanded = String::new();
                        captures.expand(replacement, &mut expanded);
                        req.request_uri = expanded;
                    }
                }
            }
        }

        let mut response = next.execute(&req, connection)?;

        for rule in &self.response_rules {
            match rule {
                ResponseRule::SetHeader { name, value } => {
                    response.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                    response.headers.push(Header { name: name.clone(), value: value.clone() });
                }
                ResponseRule::RemoveHeader(name) => {
                    response.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                }
                ResponseRule::SetStatus { code, reason } => {
                    response.status_code = *code;
                    response.reason_phrase = reason.clone();
                }
                ResponseRule::BodyReplace { from, to } => {
                    for cr in &mut response.content_range_list {
                        cr.body = replace_bytes(&cr.body, from, to);
                    }
                }
            }
        }

        Ok(response)
    }
}

fn replace_bytes(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return haystack.to_vec();
    }
    let mut result = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if haystack[i..].starts_with(needle) {
            result.extend_from_slice(replacement);
            i += needle.len();
        } else {
            result.push(haystack[i]);
            i += 1;
        }
    }
    result.extend_from_slice(&haystack[i..]);
    result
}
