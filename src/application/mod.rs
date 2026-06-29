use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

#[cfg(test)]
mod example;

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
}