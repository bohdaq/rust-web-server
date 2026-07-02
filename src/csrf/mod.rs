//! CSRF protection middleware using the double-submit cookie pattern.
//!
//! Enabled by the `csrf` Cargo feature:
//!
//! ```toml
//! rust-web-server = { version = "17", features = ["csrf"] }
//! ```
//!
//! # Usage
//!
//! Add [`CsrfLayer`] to the middleware stack. It validates the CSRF token on
//! every mutating request (`POST`, `PUT`, `PATCH`, `DELETE`) and passes safe
//! methods through unconditionally.
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::csrf::CsrfLayer;
//!
//! let app = App::new().wrap(CsrfLayer::new());
//! ```
//!
//! # Embedding the token in HTML forms
//!
//! In a `GET` handler, call [`CsrfToken::from_request`] to obtain the current
//! token and embed it in the form:
//!
//! ```rust,no_run
//! use rust_web_server::csrf::CsrfToken;
//! use rust_web_server::request::Request;
//! use rust_web_server::response::Response;
//! use rust_web_server::server::ConnectionInfo;
//!
//! fn show_form(req: &Request, _conn: &ConnectionInfo) -> Response {
//!     let token = CsrfToken::from_request(req)
//!         .map(|t| t.value().to_string())
//!         .unwrap_or_default();
//!     let html = format!(
//!         r#"<form method="POST" action="/submit">
//!   <input type="hidden" name="_csrf" value="{token}">
//!   <button type="submit">Submit</button>
//! </form>"#
//!     );
//!     Response::new()  // build your response with `html` body
//! }
//! ```
//!
//! For AJAX, include the token in the `X-CSRF-Token` request header. The cookie
//! is not `HttpOnly` by default so that JavaScript can read it via
//! `document.cookie`; call [`.http_only(true)`](CsrfLayer::http_only) to
//! restrict to HTML-form workflows only.
//!
//! # How it works
//!
//! 1. On safe methods (`GET`/`HEAD`/`OPTIONS`): [`CsrfLayer`] reads the
//!    existing `_csrf` cookie or generates a 32-byte random token. The token is
//!    injected into the request (as an internal header) so [`CsrfToken::from_request`]
//!    can return it to the handler. The cookie is (re-)set on the response.
//!
//! 2. On mutating methods: the layer reads the cookie value and the submitted
//!    value (from the `X-CSRF-Token` header or the `_csrf` form field). It
//!    compares them in **constant time** and returns `403 Forbidden` if they do
//!    not match.

#[cfg(test)]
mod tests;

use rand_core::{OsRng, RngCore};

use crate::application::Application;
use crate::error::{AppError, IntoResponse};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Internal header name used to pass the current CSRF token from
/// [`CsrfLayer`] to [`CsrfToken::from_request`] without a separate store.
const INJECTED_HEADER: &str = "X-Rws-Csrf-Token";

/// CSRF protection middleware.
///
/// Add to any application with `.wrap(CsrfLayer::new())`.  Safe HTTP methods
/// (`GET`, `HEAD`, `OPTIONS`, `TRACE`) always pass through; mutating methods
/// are validated and rejected with `403 Forbidden` on mismatch.
///
/// # Defaults
///
/// | Setting | Default |
/// |---|---|
/// | Cookie name | `_csrf` |
/// | Form field name | `_csrf` |
/// | Validated header | `X-CSRF-Token` |
/// | `SameSite` | `Strict` |
/// | `HttpOnly` | `false` (JS-readable for AJAX workflows) |
/// | `Secure` | `false` (enable in production behind HTTPS) |
pub struct CsrfLayer {
    cookie_name: String,
    field_name: String,
    header_name: String,
    http_only: bool,
    secure: bool,
    same_site: String,
}

impl Default for CsrfLayer {
    fn default() -> Self {
        CsrfLayer {
            cookie_name: "_csrf".to_string(),
            field_name: "_csrf".to_string(),
            header_name: "X-CSRF-Token".to_string(),
            http_only: false,
            secure: false,
            same_site: "Strict".to_string(),
        }
    }
}

impl CsrfLayer {
    /// Create a [`CsrfLayer`] with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the cookie name (default: `"_csrf"`).
    pub fn cookie_name(mut self, name: &str) -> Self {
        self.cookie_name = name.to_string();
        self
    }

    /// Override the form field name (default: `"_csrf"`).
    pub fn field_name(mut self, name: &str) -> Self {
        self.field_name = name.to_string();
        self
    }

    /// Override the request header name that carries the token for AJAX
    /// (default: `"X-CSRF-Token"`).
    pub fn header_name(mut self, name: &str) -> Self {
        self.header_name = name.to_string();
        self
    }

    /// Set the `HttpOnly` flag on the CSRF cookie (default: `false`).
    ///
    /// Enable this if you only serve traditional HTML forms and never need
    /// JavaScript to read the token. When `true`, AJAX clients cannot access
    /// the token via `document.cookie` and must rely on a server-rendered
    /// token in the HTML.
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Set the `Secure` flag on the CSRF cookie (default: `false`).
    ///
    /// Enable in production to prevent the cookie from being sent over
    /// plain HTTP.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    fn generate_token(&self) -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn get_cookie_token(&self, req: &Request) -> Option<String> {
        get_cookie(req, &self.cookie_name)
    }

    fn get_submitted_token(&self, req: &Request) -> Option<String> {
        for h in &req.headers {
            if h.name.eq_ignore_ascii_case(&self.header_name) {
                let val = h.value.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        let is_form = req.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("content-type")
                && h.value
                    .to_lowercase()
                    .contains("application/x-www-form-urlencoded")
        });
        if is_form {
            return get_form_field(&req.body, &self.field_name);
        }
        None
    }

    fn cookie_header_value(&self, token: &str) -> String {
        let mut s = format!(
            "{}={}; Path=/; SameSite={}",
            self.cookie_name, token, self.same_site
        );
        if self.http_only {
            s.push_str("; HttpOnly");
        }
        if self.secure {
            s.push_str("; Secure");
        }
        s
    }
}

impl Middleware for CsrfLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        if is_safe_method(&request.method) {
            let token = self
                .get_cookie_token(request)
                .unwrap_or_else(|| self.generate_token());

            // Inject so CsrfToken::from_request can retrieve it inside the handler.
            let mut req = request.clone();
            req.headers.push(Header {
                name: INJECTED_HEADER.to_string(),
                value: token.clone(),
            });

            let mut response = next.execute(&req, connection)?;
            response.headers.push(Header {
                name: "Set-Cookie".to_string(),
                value: self.cookie_header_value(&token),
            });
            Ok(response)
        } else {
            let cookie_token = match self.get_cookie_token(request) {
                Some(t) => t,
                None => return Ok(AppError::Forbidden.into_response()),
            };
            let submitted_token = match self.get_submitted_token(request) {
                Some(t) => t,
                None => return Ok(AppError::Forbidden.into_response()),
            };
            if !ct_eq(cookie_token.as_bytes(), submitted_token.as_bytes()) {
                return Ok(AppError::Forbidden.into_response());
            }
            next.execute(request, connection)
        }
    }
}

/// The CSRF token for the current request.
///
/// Obtain via [`CsrfToken::from_request`] inside a `GET` handler (after
/// [`CsrfLayer`] has run) and embed the value in your HTML form.
///
/// ```rust,no_run
/// use rust_web_server::csrf::CsrfToken;
/// use rust_web_server::request::Request;
///
/// fn page(req: &Request) -> String {
///     let token = CsrfToken::from_request(req)
///         .map(|t| t.value().to_string())
///         .unwrap_or_default();
///     format!(r#"<input type="hidden" name="_csrf" value="{token}">"#)
/// }
/// ```
pub struct CsrfToken(String);

impl CsrfToken {
    /// Return the token string.
    pub fn value(&self) -> &str {
        &self.0
    }

    /// Extract the current CSRF token from a request.
    ///
    /// Returns `Some` only when [`CsrfLayer`] is in the middleware stack and
    /// the request was already processed by it. Returns `None` otherwise.
    pub fn from_request(req: &Request) -> Option<Self> {
        for h in &req.headers {
            if h.name.eq_ignore_ascii_case(INJECTED_HEADER) {
                return Some(CsrfToken(h.value.clone()));
            }
        }
        None
    }
}

impl std::fmt::Display for CsrfToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn is_safe_method(method: &str) -> bool {
    matches!(method, "GET" | "HEAD" | "OPTIONS" | "TRACE")
}

fn get_cookie(req: &Request, name: &str) -> Option<String> {
    for h in &req.headers {
        if h.name.eq_ignore_ascii_case("cookie") {
            for part in h.value.split(';') {
                let part = part.trim();
                if let Some(pos) = part.find('=') {
                    let k = part[..pos].trim();
                    if k.eq_ignore_ascii_case(name) {
                        return Some(part[pos + 1..].trim().to_string());
                    }
                }
            }
        }
    }
    None
}

fn get_form_field(body: &[u8], field: &str) -> Option<String> {
    let s = std::str::from_utf8(body).ok()?;
    for pair in s.split('&') {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?.trim();
        if k == field {
            return Some(parts.next().unwrap_or("").to_string());
        }
    }
    None
}

/// Constant-time byte-slice equality to prevent timing attacks.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
