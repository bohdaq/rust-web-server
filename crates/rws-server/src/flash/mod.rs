//! Flash messages and redirects (18.35.0, #94): a message set on one
//! response — usually a redirect after a form post — and shown on the next
//! page, once.
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::error::IntoResponse;
//! use rust_web_server::extract::FromRequest;
//! use rust_web_server::flash::{Flash, FlashLayer, Redirect};
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//! use rust_web_server::test_client::TestClient;
//!
//! let app = App::with_state(())
//!     .post("/posts", |_, _, _, _| Redirect::to("/posts").notice("Post saved").into_response())
//!     .get("/posts", |request, _, _, _| {
//!         let flash = Flash::from_request(request).unwrap();
//!         Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, flash.notice().unwrap_or(""))
//!     })
//!     .wrap(FlashLayer::new());
//!
//! let client = TestClient::new(app);
//! let saved = client.post("/posts").send();
//! assert_eq!(303, saved.status());
//! ```
//!
//! The messages travel in the `rws_flash` cookie (`HttpOnly`,
//! `SameSite=Lax`, `Path=/`), percent-encoded: readable and changeable by
//! the browser, so never put anything in a flash that the page trusts —
//! templates escape it like any other text. [`FlashLayer`] removes the
//! cookie from the browser after the request that carried it, unless the
//! response sets a new flash. A flash cookie is capped at
//! [`MAX_COOKIE_BYTES`]; messages past it are dropped.

#[cfg(test)]
mod tests;

use crate::application::Application;
use crate::cookie::{CookieJar, SetCookie};
use crate::error::IntoResponse;
use crate::extract::FromRequest;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::names::encode;
use crate::server::ConnectionInfo;

/// The cookie flash messages travel in.
pub const COOKIE_NAME: &str = "rws_flash";

/// The largest flash cookie value written; later messages are dropped.
pub const MAX_COOKIE_BYTES: usize = 3800;

/// One message: its kind (`notice`, `alert`, or your own) and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessage {
    pub kind: String,
    pub text: String,
}

/// The flash messages a request carried. A handler argument
/// (`FromRequest`, never fails); empty when there are none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flash {
    messages: Vec<FlashMessage>,
}

impl Flash {
    /// The messages in `request`'s flash cookie.
    pub fn read(request: &Request) -> Flash {
        let Some(header) = request.header("cookie") else {
            return Flash::default();
        };
        match CookieJar::parse(&header.value).get(COOKIE_NAME) {
            Some(cookie) => Flash { messages: decode_messages(&cookie.value) },
            None => Flash::default(),
        }
    }

    /// Every message, in the order they were set.
    pub fn messages(&self) -> &[FlashMessage] {
        &self.messages
    }

    /// The first message of `kind`.
    pub fn get(&self, kind: &str) -> Option<&str> {
        self.messages.iter().find(|m| m.kind == kind).map(|m| m.text.as_str())
    }

    /// The first `notice` message.
    pub fn notice(&self) -> Option<&str> {
        self.get("notice")
    }

    /// The first `alert` message.
    pub fn alert(&self) -> Option<&str> {
        self.get("alert")
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl FromRequest for Flash {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Ok(Flash::read(request))
    }
}

/// Add a flash message to `response` (after any it already sets).
pub fn set(response: &mut Response, kind: &str, text: &str) {
    let mut messages = Vec::new();
    response.headers.retain(|h| {
        let ours = h.name.eq_ignore_ascii_case(Header::_SET_COOKIE) && h.value.starts_with(&format!("{COOKIE_NAME}="));
        if ours {
            let value = h.value[COOKIE_NAME.len() + 1..].split(';').next().unwrap_or("");
            messages = decode_messages(value);
        }
        !ours
    });
    messages.push(FlashMessage { kind: kind.to_string(), text: text.to_string() });
    response.headers.push(Header::new(Header::_SET_COOKIE, cookie(&encode_messages(&messages))));
}

fn cookie(value: &str) -> String {
    SetCookie::new(COOKIE_NAME, value).path("/").http_only().same_site("Lax").build()
}

fn clear_cookie() -> String {
    SetCookie::new(COOKIE_NAME, "").path("/").http_only().same_site("Lax").max_age(0).build()
}

/// `kind=text&kind=text`, percent-encoded, cut at [`MAX_COOKIE_BYTES`].
fn encode_messages(messages: &[FlashMessage]) -> String {
    let mut out = String::new();
    for message in messages {
        let pair = format!("{}={}", encode(&message.kind, false), encode(&message.text, false));
        if out.len() + pair.len() + 1 > MAX_COOKIE_BYTES {
            break;
        }
        if !out.is_empty() {
            out.push('&');
        }
        out.push_str(&pair);
    }
    out
}

fn decode_messages(value: &str) -> Vec<FlashMessage> {
    value
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (kind, text) = pair.split_once('=')?;
            Some(FlashMessage { kind: percent_decode(kind)?, text: percent_decode(text)? })
        })
        .collect()
}

/// `%XX` decoding (`+` stays `+`); `None` for a bad escape or non-UTF-8.
pub(crate) fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Removes the flash cookie after the request that carried it, unless the
/// response sets a new flash. Wrap the app in it once: `app.wrap(FlashLayer::new())`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlashLayer;

impl FlashLayer {
    pub fn new() -> Self {
        FlashLayer
    }
}

fn carries_flash(request: &Request) -> bool {
    request.header("cookie").is_some_and(|h| CookieJar::parse(&h.value).get(COOKIE_NAME).is_some())
}

impl Middleware for FlashLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let mut response = next.execute(request, connection)?;
        if carries_flash(request) {
            let sets_flash = response.headers.iter().any(|h| h.name.eq_ignore_ascii_case(Header::_SET_COOKIE) && h.value.starts_with(&format!("{COOKIE_NAME}=")));
            if !sets_flash {
                response.headers.push(Header::new(Header::_SET_COOKIE, clear_cookie()));
            }
        }
        Ok(response)
    }

    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    /// Requests without a flash cookie don't need it: async handlers stay async.
    fn is_passthrough(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        !carries_flash(request)
    }

    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn layer_name(&self) -> &'static str {
        "FlashLayer"
    }
}

/// A redirect, optionally with flash messages: `303 See Other` by default
/// (the method after a form post becomes `GET`).
#[derive(Debug, Clone)]
pub struct Redirect {
    location: String,
    status: &'static crate::response::StatusCodeReasonPhrase,
    flash: Vec<FlashMessage>,
}

impl Redirect {
    /// `303 See Other` to `location` (CR and LF are removed from it).
    pub fn to(location: &str) -> Self {
        Redirect { location: location.replace(['\r', '\n'], ""), status: STATUS_CODE_REASON_PHRASE.n303_see_other, flash: Vec::new() }
    }

    /// `302 Found` to `location`.
    pub fn found(location: &str) -> Self {
        Redirect { status: STATUS_CODE_REASON_PHRASE.n302_found, ..Redirect::to(location) }
    }

    /// `301 Moved Permanently` to `location`.
    pub fn permanent(location: &str) -> Self {
        Redirect { status: STATUS_CODE_REASON_PHRASE.n301_moved_permanently, ..Redirect::to(location) }
    }

    /// To the route named `name` ([`url_for`](crate::router::url_for));
    /// `/` when there's no such route.
    pub fn to_route(name: &str, params: &[(&str, &str)]) -> Self {
        Redirect::to(&crate::router::url_for(name, params).unwrap_or_else(|| "/".to_string()))
    }

    /// Add a flash message of `kind`.
    pub fn flash(mut self, kind: &str, text: &str) -> Self {
        self.flash.push(FlashMessage { kind: kind.to_string(), text: text.to_string() });
        self
    }

    /// Add a `notice` flash message.
    pub fn notice(self, text: &str) -> Self {
        self.flash("notice", text)
    }

    /// Add an `alert` flash message.
    pub fn alert(self, text: &str) -> Self {
        self.flash("alert", text)
    }

    /// Where it redirects.
    pub fn location(&self) -> &str {
        &self.location
    }
}

impl IntoResponse for Redirect {
    fn into_response(self) -> Response {
        let mut response = Response::get_response(self.status, None, None);
        response.headers.push(Header::new(Header::_LOCATION, self.location));
        for message in &self.flash {
            set(&mut response, &message.kind, &message.text);
        }
        response
    }
}
