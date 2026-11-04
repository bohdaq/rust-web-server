//! Small helpers shared by every role.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, OnceLock};

use rust_web_server::application::Application;
use rust_web_server::header::Header;
use rust_web_server::mime_type::MimeType;
use rust_web_server::range::Range;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use rust_web_server::server::ConnectionInfo;

pub const S: &rust_web_server::response::ResponseStatusCodeReasonPhrase = &STATUS_CODE_REASON_PHRASE;

/// A non-empty environment variable.
pub fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

pub fn text(status: &StatusCodeReasonPhrase, body: &str) -> Response {
    Response::text(status, body)
}

pub fn json(status: &StatusCodeReasonPhrase, body: &serde_json::Value) -> Response {
    Response::json(status, body.to_string().into_bytes())
}

pub fn html(status: &StatusCodeReasonPhrase, body: &str) -> Response {
    let mut r = Response::with_status(status);
    r.content_range_list = vec![Range::get_content_range(body.as_bytes().to_vec(), MimeType::TEXT_HTML.to_string())];
    r
}

pub fn bytes(status: &StatusCodeReasonPhrase, body: Vec<u8>, content_type: &str) -> Response {
    let mut r = Response::with_status(status);
    r.content_range_list = vec![Range::get_content_range(body, content_type.to_string())];
    r
}

pub fn empty(status: &StatusCodeReasonPhrase) -> Response {
    Response::with_status(status)
}

/// `503` naming what is missing (a feature whose env vars are unset).
pub fn unavailable(what: &str) -> Response {
    text(S.n503_service_unavailable, &format!("unavailable: {what}\n"))
}

pub fn redirect(location: &str) -> Response {
    let mut r = Response::with_status(S.n302_found);
    r.headers.push(Header::new("Location", location.to_string()));
    r
}

/// The query string's parameters, percent-decoded (`+` is a space).
///
/// Not rws's `extract::Query`: its decoder (`URL::parse_query`, from
/// url-search-params 12) only knows a fixed list of upper-case escapes
/// (`%3C`, `%C3%A9`, `+` stay as they are), decodes `%25` before the others
/// (`%2526` becomes `&`), and cuts a value at its second `=`.
pub fn query(request: &Request) -> HashMap<String, String> {
    let q = request.request_uri.split_once('?').map(|(_, q)| q).unwrap_or("");
    let q = q.split('#').next().unwrap_or("");
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(p), String::new()),
        })
        .collect()
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push(h << 4 | l);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.as_ref())
}

pub fn path(request: &Request) -> &str {
    request.request_uri.split('?').next().unwrap_or("")
}

/// The process-wide runtime every database future runs on. sqlx connections
/// belong to the runtime that opened them, so all of them are opened here
/// (rws's `AsyncAppWithState` builds a throwaway runtime per request, which
/// would strand pooled connections on a dead driver).
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("e2e-db")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// Run `fut` on [`runtime`] and wait for it from a synchronous handler.
/// Works from the server's blocking pool, from a plain thread and from tests.
pub fn block<F, T>(fut: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    runtime().spawn(async move {
        let _ = tx.send(fut.await);
    });
    rx.recv().expect("database task panicked")
}

/// `Application` + `Clone` around any stack of apps (what `Server::run_tls`
/// needs), with a hook that runs before the stack for env-dependent `503`s.
#[derive(Clone)]
pub struct Root {
    inner: Arc<dyn Application + Send + Sync>,
    guard: Arc<dyn Fn(&Request) -> Option<Response> + Send + Sync>,
}

impl Root {
    pub fn new(inner: impl Application + Send + Sync + 'static) -> Self {
        Root { inner: Arc::new(inner), guard: Arc::new(|_| None) }
    }

    pub fn guard(mut self, guard: impl Fn(&Request) -> Option<Response> + Send + Sync + 'static) -> Self {
        self.guard = Arc::new(guard);
        self
    }
}

impl Application for Root {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if let Some(r) = (self.guard)(request) {
            return Ok(r);
        }
        self.inner.execute(request, connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding() {
        assert_eq!("<b> é", percent_decode("%3Cb%3e+%C3%A9"));
        assert_eq!("%26", percent_decode("%2526"));
        assert_eq!("100%", percent_decode("100%"));
        assert_eq!("%zz", percent_decode("%zz"));
        let r = Request {
            method: "GET".into(),
            request_uri: "/x?a=b=c&flag&e=%3D".into(),
            http_version: "HTTP/1.1".into(),
            headers: vec![],
            body: Default::default(),
        };
        let q = query(&r);
        assert_eq!("b=c", q["a"]);
        assert_eq!("", q["flag"]);
        assert_eq!("=", q["e"]);
    }
}
