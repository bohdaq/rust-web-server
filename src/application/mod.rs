//! Application: the type a server runs for every request.
//!
//! An [`Application`] gets each parsed [`Request`](crate::request::Request) and its
//! [`ConnectionInfo`](crate::server::ConnectionInfo) and returns a
//! [`Response`](crate::response::Response) (or an error message). The server runs
//! it on many threads, so it must be `Clone + Send + 'static`.
//!
//! The built-in [`App`](crate::app::App) is one: it starts from a `501` response
//! with the default headers and walks a list of
//! [`Controller`](crate::controller::Controller)s, each checking whether it
//! matches and, if so, filling in the response. Most code uses `App`,
//! [`Router`](crate::router::Router) or `App::with_state` instead of implementing
//! this trait directly; see `src/application/example/` for a hand-written one.

use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

#[cfg(test)]
mod example;
#[cfg(test)]
mod tests;

/// Dispatch trait that wires controllers into a request-handling loop.
///
/// Implement this to define which controllers run and in what order.
/// Pass the implementation to [`Server::run`] or [`Server::run_tls`].
///
/// The built-in [`App`](crate::app::App) implementation covers static files,
/// favicons, forms, and file uploads. Embed it or replace it entirely.
pub trait Application {
    /// Receives a parsed request and returns a fully-built response.
    /// Walk your controller list with `is_matching` / `process` and return the first match.
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String>;

    /// `true` when executing `request` is quick and never waits on anything
    /// but the local filesystem — health checks, metrics, small static files.
    /// The async servers (HTTP/1.1, HTTP/2, HTTP/3) then call [`execute`]
    /// directly on the event loop, as nginx does, instead of handing the
    /// request to tokio's blocking pool: that handoff (a shared lock and two
    /// cross-thread wake-ups) costs more than serving a small file.
    ///
    /// Default `false`, and it must stay `false` for anything that can wait:
    /// a database query, an upstream request, a lock held across I/O. Run on
    /// the event loop, those would stall every other connection on that
    /// worker. Wrappers answer `true` only when every layer agrees.
    ///
    /// [`execute`]: Application::execute
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        false
    }

    /// An asynchronous version of [`execute`] for this request, when the
    /// application has one: the async servers await it on the connection's
    /// task instead of tying up a blocking-pool thread while it waits (the
    /// config-driven proxy uses this to wait for a backend without holding a
    /// thread). `None` (the default) means "use `execute`". Wrappers return
    /// `Some` only when they add no synchronous work of their own.
    ///
    /// [`execute`]: Application::execute
    #[cfg(feature = "http2")]
    fn execute_async<'a>(&'a self, _request: &'a Request, _connection: &'a ConnectionInfo) -> Option<AsyncResponse<'a>> {
        None
    }

    /// `true` to receive this request's body as a stream ([`Body::Stream`])
    /// instead of read into memory first: [`execute`] then runs while the
    /// body is still arriving, on a blocking-pool thread, and reads it with
    /// `std::io::Read` — a multi-gigabyte upload in a few chunks of memory.
    /// Asked after the head is read, so `request.body` is empty here;
    /// decide from the method, path and headers.
    ///
    /// Default `false`: the body is read first, as always. Async HTTP/1.1
    /// and HTTP/2 honour it; the HTTP/1.1-only build and HTTP/3 read the
    /// body first regardless. Wrappers answer `true` only when every layer
    /// leaves the body alone ([`Middleware::accepts_streamed_body`]).
    ///
    /// [`Body::Stream`]: crate::request::Body::Stream
    /// [`execute`]: Application::execute
    /// [`Middleware::accepts_streamed_body`]: crate::middleware::Middleware::accepts_streamed_body
    fn streams_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        false
    }
}

/// The future [`Application::execute_async`] returns.
#[cfg(feature = "http2")]
pub type AsyncResponse<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, String>> + Send + 'a>>;