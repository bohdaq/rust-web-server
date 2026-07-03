//! Forward-auth middleware — delegates the allow/deny decision for every
//! request to an external HTTP service (Traefik's `forwardAuth`, nginx's
//! `auth_request`), so a policy engine (OPA, Casbin) or a centralized auth
//! service can gate requests without embedding that logic in rws.
//!
//! No new dependency: makes a plain `GET` via the existing outbound
//! [`crate::http_client::Client`].
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::auth::forward::ForwardAuthLayer;
//!
//! let app = App::new()
//!     .wrap(ForwardAuthLayer::new("http://auth.internal/verify")
//!         .copy_header("X-User-Id")
//!         .copy_header("X-Roles")
//!         .timeout_ms(2000));
//! ```
//!
//! # How it works
//!
//! 1. Every incoming request header is copied onto a `GET` request sent to
//!    the configured auth service URL.
//! 2. **2xx response** — the request is allowed through. Any header named in
//!    [`.copy_header(...)`](ForwardAuthLayer::copy_header) that's present on
//!    the *auth service's* response replaces (not appends to — see below) the
//!    same-named header on the forwarded request. This is how an auth
//!    service that resolves a session cookie to a user ID hands that ID to
//!    your handler as a plain header.
//! 3. **Any other status** — the auth service's response is returned to the
//!    client **verbatim**: status code, all headers (except hop-by-hop and
//!    body-framing ones), and body. This preserves `WWW-Authenticate`
//!    challenges, `Location` redirects for OAuth flows, and custom error
//!    bodies without rws needing to understand any of them.
//! 4. **Auth service unreachable** (connection refused, timeout, DNS
//!    failure) — the request is rejected with `502 Bad Gateway`. This fails
//!    *closed*: an unreachable auth service is not the same as "access
//!    granted."
//!
//! Copied headers **replace** any same-named header the client already sent,
//! rather than being appended alongside it — otherwise a client could send
//! its own `X-User-Id` and have it coexist with (and potentially shadow) the
//! auth service's verified value, depending on which duplicate a downstream
//! `get_header` call happens to match first.

#[cfg(test)]
mod tests;

use crate::application::Application;
use crate::core::New;
use crate::header::Header;
use crate::http_client::Client;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Headers never copied verbatim onto rws's own response: hop-by-hop headers
/// (RFC 7230 §6.1) and the two headers whose values are derived from
/// `content_range_list` during response generation, not set directly.
const EXCLUDED_PASSTHROUGH_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "content-type",
    "content-length",
];

/// Middleware that delegates the allow/deny decision for every request to
/// an external HTTP service. See the [module docs](self) for the full flow.
pub struct ForwardAuthLayer {
    auth_url: String,
    copy_headers: Vec<String>,
    timeout_ms: u64,
}

impl ForwardAuthLayer {
    /// Create a layer that calls `auth_url` (a full `http://`/`https://` URL,
    /// e.g. `"http://auth.internal/verify"`) for every request.
    ///
    /// Default timeout: 5000ms. HTTPS URLs require the `http-client` or
    /// `http2` feature (same requirement as the outbound HTTP client itself).
    pub fn new(auth_url: impl Into<String>) -> Self {
        ForwardAuthLayer { auth_url: auth_url.into(), copy_headers: Vec::new(), timeout_ms: 5000 }
    }

    /// Copy `name` from the auth service's response onto the forwarded
    /// request when the auth service allows the request (2xx). Call
    /// multiple times to copy multiple headers. Headers not present on the
    /// auth service's response are left untouched.
    pub fn copy_header(mut self, name: impl Into<String>) -> Self {
        self.copy_headers.push(name.into());
        self
    }

    /// Override the auth service call timeout (default: 5000ms).
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
}

impl Middleware for ForwardAuthLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        // max_redirects(0): a 3xx from the auth service (e.g. an OAuth
        // login redirect) must reach passthrough_response() as-is, not be
        // silently followed and replaced with whatever it points to.
        let client = Client::new().timeout_ms(self.timeout_ms).max_redirects(0);
        let mut builder = client.get(&self.auth_url);
        for h in &request.headers {
            builder = builder.header(&h.name, &h.value);
        }

        let auth_response = match builder.send() {
            Ok(r) => r,
            Err(_) => return Ok(auth_service_unreachable()),
        };

        if !auth_response.is_success() {
            return Ok(passthrough_response(&auth_response));
        }

        let mut forwarded = request.clone();
        for name in &self.copy_headers {
            if let Some(value) = auth_response.header(name) {
                forwarded.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                forwarded.headers.push(Header { name: name.clone(), value: value.to_string() });
            }
        }

        next.execute(&forwarded, connection)
    }
}

fn auth_service_unreachable() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        b"502 Bad Gateway: auth service unreachable".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

fn passthrough_response(auth_response: &crate::http_client::Response) -> Response {
    let status = auth_response.status();
    let mut r = Response::new();
    r.status_code = status as i16;
    r.reason_phrase = reason_phrase_for_status(status);

    for (name, value) in auth_response.headers() {
        if EXCLUDED_PASSTHROUGH_HEADERS.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        r.headers.push(Header { name: name.clone(), value: value.clone() });
    }

    let body = auth_response.bytes();
    if !body.is_empty() {
        let content_type = auth_response
            .header(Header::_CONTENT_TYPE)
            .unwrap_or(MimeType::TEXT_PLAIN)
            .to_string();
        r.content_range_list = vec![Range::get_content_range(body.to_vec(), content_type)];
    }

    r
}

fn reason_phrase_for_status(status: u16) -> String {
    Response::status_code_reason_phrase_list()
        .into_iter()
        .find(|s| *s.status_code == status as i16)
        .map(|s| s.reason_phrase.to_string())
        .unwrap_or_default()
}
