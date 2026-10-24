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
}