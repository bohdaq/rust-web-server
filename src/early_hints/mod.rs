//! `103 Early Hints` (RFC 8297, 18.12.0, #38): `Link` headers a client can
//! act on — preload a stylesheet, preconnect to an origin — while the
//! response is still being made.
//!
//! A handler or middleware calls [`send`] at any point before it returns;
//! the connection writes a `103` head straight away and the final response
//! after it. [`EarlyHintsLayer`] sends a fixed list for every request it
//! sees:
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::early_hints::EarlyHintsLayer;
//!
//! let app = App::new().wrap(EarlyHintsLayer::new([
//!     "</app.css>; rel=preload; as=style",
//!     "<https://cdn.example.com>; rel=preconnect",
//! ]));
//! ```
//!
//! Sent on HTTP/2 and HTTP/3; on HTTP/1.1 only with
//! `RWS_CONFIG_EARLY_HINTS_HTTP1=true` (browsers use them only over h2/h3,
//! and some old HTTP/1.1 clients mishandle extra `1xx` responses), never on
//! HTTP/1.0. The `http1`-only build writes them just before the final
//! response. Where nothing can send them — [`TestClient`](crate::test_client),
//! an app called directly — [`send`] does nothing and returns `false`.
//! At most [`MAX_PER_REQUEST`] per request; a value with CR, LF or NUL is
//! refused.

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// `103` heads one request may send.
pub const MAX_PER_REQUEST: usize = 4;

/// Where one request's hints go: the connection that serves it.
pub(crate) struct Sink {
    deliver: Box<dyn Fn(Vec<String>) + Send + Sync>,
    sent: AtomicUsize,
}

impl Sink {
    pub(crate) fn new(deliver: impl Fn(Vec<String>) + Send + Sync + 'static) -> Arc<Sink> {
        Arc::new(Sink { deliver: Box::new(deliver), sent: AtomicUsize::new(0) })
    }

    /// Start counting again (a sink reused for the next request).
    pub(crate) fn reset(&self) {
        self.sent.store(0, Ordering::Relaxed);
    }

    fn send(&self, links: Vec<String>) -> bool {
        if self.sent.fetch_add(1, Ordering::Relaxed) >= MAX_PER_REQUEST {
            return false;
        }
        (self.deliver)(links);
        true
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Sink>>> = const { RefCell::new(None) };
}

#[cfg(feature = "http2")]
tokio::task_local! {
    static TASK: Option<Arc<Sink>>;
}

/// Run `f` with `sink` as this thread's destination for [`send`] (the
/// previous one restored afterwards, also on panic).
pub(crate) fn with_sink<R>(sink: Option<Arc<Sink>>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Arc<Sink>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            CURRENT.with(|c| *c.borrow_mut() = previous);
        }
    }
    let previous = CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), sink));
    let _restore = Restore(previous);
    f()
}

/// Run `future` with `sink` as its destination for [`send`] (async
/// handlers, the async forwarder).
#[cfg(feature = "http2")]
pub(crate) async fn scope<F: std::future::Future>(sink: Option<Arc<Sink>>, future: F) -> F::Output {
    TASK.scope(sink, future).await
}

fn current() -> Option<Arc<Sink>> {
    #[cfg(feature = "http2")]
    if let Ok(Some(sink)) = TASK.try_with(|s| s.clone()) {
        return Some(sink);
    }
    CURRENT.with(|c| c.borrow().clone())
}

/// Whether [`send`] would reach a client from here.
pub fn available() -> bool {
    current().is_some()
}

/// Send a `103 Early Hints` with these `Link` header values now, before the
/// final response. `false` when there is nowhere to send it (see the module
/// docs), the request already sent [`MAX_PER_REQUEST`], or a value is empty
/// or holds CR, LF or NUL.
pub fn send<S: AsRef<str>>(links: &[S]) -> bool {
    if links.is_empty() || !links.iter().all(|l| valid(l.as_ref())) {
        return false;
    }
    match current() {
        Some(sink) => sink.send(links.iter().map(|l| l.as_ref().to_string()).collect()),
        None => false,
    }
}

/// A usable `Link` value: not empty, no CR, LF or NUL.
pub fn valid(link: &str) -> bool {
    !link.trim().is_empty() && !link.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0))
}

/// Whether this HTTP/1.x request may get `103`s: HTTP/1.1 with
/// `early_hints_http1` on.
pub(crate) fn http1_wanted(request: &Request) -> bool {
    request.http_version == "HTTP/1.1" && crate::server_config::ServerConfig::current().early_hints_http1
}

/// The HTTP/1.1 `103` head for `links`.
pub(crate) fn http1_head(links: &[String]) -> Vec<u8> {
    let mut head = b"HTTP/1.1 103 Early Hints\r\n".to_vec();
    for link in links {
        head.extend_from_slice(b"Link: ");
        head.extend_from_slice(link.as_bytes());
        head.extend_from_slice(b"\r\n");
    }
    head.extend_from_slice(b"\r\n");
    head
}

/// Middleware that sends the same `Link` hints for every request, then
/// hands it on.
#[derive(Clone, Debug)]
pub struct EarlyHintsLayer {
    links: Arc<Vec<String>>,
}

impl EarlyHintsLayer {
    /// `links` are `Link` header values, e.g.
    /// `</app.css>; rel=preload; as=style`. Invalid ones are dropped.
    pub fn new<I, S>(links: I) -> EarlyHintsLayer
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        EarlyHintsLayer { links: Arc::new(links.into_iter().map(Into::into).filter(|l: &String| valid(l)).collect()) }
    }

    pub fn links(&self) -> &[String] {
        &self.links
    }
}

impl Middleware for EarlyHintsLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        if !self.links.is_empty() {
            send(&self.links);
        }
        next.execute(request, connection)
    }

    /// Queueing a hint never blocks.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
