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
}

/// The future [`Application::execute_async`] returns.
#[cfg(feature = "http2")]
pub type AsyncResponse<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, String>> + Send + 'a>>;