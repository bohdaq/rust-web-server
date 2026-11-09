//! Error pages (18.35.0, #95): what the client sees when a handler panics
//! or the app returns an error, depending on [`RwsEnv`].
//!
//! - In `dev` and `test`: the panic message, where it happened, the request
//!   and the backtrace — an HTML page for a browser, problem details with
//!   `location` and `backtrace` members for anything else.
//! - In `prod`: `500` problem details without any of it; the details go to
//!   stderr.
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::error_pages::ErrorPages;
//! use rust_web_server::response::Response;
//! use rust_web_server::test_client::TestClient;
//!
//! let app = App::with_state(())
//!     .get("/boom", |_, _, _, _| -> Response { panic!("the cache is empty") })
//!     .wrap(ErrorPages::detailed(true));   // ErrorPages::from_env() reads RWS_ENV
//! let response = TestClient::new(app).get("/boom").header("Accept", "text/html").send();
//! assert_eq!(500, response.status());
//! assert!(response.body_text().contains("the cache is empty"));
//! ```
//!
//! With details on, every request goes through the layer on the blocking
//! pool (async handlers included), so their panics can be caught; with them
//! off, requests pass straight through and the server's own panic handling
//! answers `500`.

// The tests read the JSON bodies with serde_json.
#[cfg(all(test, feature = "serde"))]
mod tests;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use crate::application::Application;
use crate::entry_point::RwsEnv;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Where the last panic happened and its backtrace, recorded by the hook
/// while a detailed `ErrorPages` exists.
struct Captured {
    location: String,
    backtrace: String,
}

static CAPTURING: AtomicBool = AtomicBool::new(false);
/// The latest panic of each thread (the newest few threads): a panic's
/// details belong to the thread that caught it, whatever else panics
/// meanwhile.
static LAST: Mutex<Vec<(std::thread::ThreadId, Captured)>> = Mutex::new(Vec::new());
const KEPT: usize = 64;

/// Chain a panic hook (once) that records the location and backtrace of a
/// panic while capturing is on, then runs the hook that was there.
fn install_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if CAPTURING.load(Ordering::Relaxed) {
                let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
                let backtrace = std::backtrace::Backtrace::force_capture().to_string();
                if let Ok(mut last) = LAST.lock() {
                    let me = std::thread::current().id();
                    last.retain(|(id, _)| *id != me);
                    if last.len() >= KEPT {
                        last.remove(0);
                    }
                    last.push((me, Captured { location, backtrace }));
                }
            }
            previous(info);
        }));
    });
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic with a non-string payload".to_string()
    }
}

/// The error-page middleware. See the [module docs](self).
#[derive(Clone, Copy, Debug)]
pub struct ErrorPages {
    details: bool,
}

impl ErrorPages {
    /// Details per `RWS_ENV`: on in `dev` and `test`, off in `prod` (and
    /// when it's unset).
    pub fn from_env() -> Self {
        Self::detailed(RwsEnv::current().shows_error_details())
    }

    /// Details on or off, whatever `RWS_ENV` says.
    pub fn detailed(details: bool) -> Self {
        if details {
            install_hook();
            CAPTURING.store(true, Ordering::Relaxed);
        }
        ErrorPages { details }
    }

    /// Whether the client sees the details.
    pub fn shows_details(&self) -> bool {
        self.details
    }

    fn answer(&self, request: &Request, message: &str, captured: Option<Captured>) -> Response {
        let (location, backtrace) = captured.map(|c| (c.location, c.backtrace)).unwrap_or_default();
        eprintln!("[rws] {} {} failed: {message}{}{}", request.method, request.request_uri, if location.is_empty() { String::new() } else { format!(" at {location}") }, if backtrace.is_empty() { String::new() } else { format!("\n{backtrace}") });
        if !self.details {
            return crate::error::problem::problem(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, "");
        }
        let wants_html = request.header("accept").is_some_and(|h| h.value.contains("text/html"));
        if wants_html {
            return page(request, message, &location, &backtrace);
        }
        let frames: Vec<&str> = backtrace.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect();
        let body = serde_free_json(message, &location, &frames);
        let content = crate::range::Range::get_content_range(body.into_bytes(), crate::error::problem::PROBLEM_JSON.to_string());
        Response::get_response(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, None, Some(vec![content]))
    }
}

/// `{"type","title","status","detail","location","backtrace":[…]}` without
/// a JSON crate.
fn serde_free_json(message: &str, location: &str, frames: &[&str]) -> String {
    fn s(text: &str) -> String {
        let mut out = String::from("\"");
        for c in text.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    let frames: Vec<String> = frames.iter().map(|f| s(f)).collect();
    format!(
        "{{\"type\":\"about:blank\",\"title\":\"Internal Server Error\",\"status\":500,\"detail\":{},\"location\":{},\"backtrace\":[{}]}}",
        s(message),
        s(location),
        frames.join(",")
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn page(request: &Request, message: &str, location: &str, backtrace: &str) -> Response {
    let headers: String = request
        .headers
        .iter()
        .map(|h| {
            let secret = matches!(h.name.to_ascii_lowercase().as_str(), "cookie" | "authorization" | "proxy-authorization" | "x-api-key");
            let value = if secret { "(hidden)" } else { h.value.as_ref() };
            format!("<tr><th>{}</th><td>{}</td></tr>", escape(&h.name), escape(value))
        })
        .collect();
    let body = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>500: {title}</title>\
         <style>body{{font:15px system-ui,sans-serif;margin:2rem;color:#222}}h1{{color:#b00020;font-size:1.4rem}}code,pre{{font:13px ui-monospace,monospace}}pre{{background:#f6f6f6;padding:1rem;overflow:auto}}th{{text-align:left;padding-right:1rem;vertical-align:top}}</style></head>\
         <body><h1>{title}</h1><p><code>{location}</code></p><p><code>{method} {uri}</code></p>\
         <h2>Backtrace</h2><pre>{backtrace}</pre><h2>Request headers</h2><table>{headers}</table>\
         <p><small>Shown because RWS_ENV is dev or test; production answers a bare 500.</small></p></body></html>",
        title = escape(message),
        location = escape(location),
        method = escape(&request.method),
        uri = escape(&request.request_uri),
        backtrace = if backtrace.is_empty() { "(none: the handler ran on another thread, or the app returned an error)".to_string() } else { escape(backtrace) },
    );
    let content = crate::range::Range::get_content_range(body.into_bytes(), crate::mime_type::MimeType::TEXT_HTML.to_string());
    let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, None, Some(vec![content]));
    response.headers.push(Header::new(Header::_CACHE_CONTROL, "no-store"));
    response
}

impl Middleware for ErrorPages {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        match catch_unwind(AssertUnwindSafe(|| next.execute(request, connection))) {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(message)) => Ok(self.answer(request, &message, None)),
            Err(payload) => {
                let me = std::thread::current().id();
                let captured = LAST.lock().ok().and_then(|mut last| {
                    let at = last.iter().position(|(id, _)| *id == me)?;
                    Some(last.remove(at).1)
                });
                Ok(self.answer(request, &panic_message(&*payload), captured))
            }
        }
    }

    fn is_passthrough(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        !self.details
    }

    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn layer_name(&self) -> &'static str {
        "ErrorPages"
    }
}
