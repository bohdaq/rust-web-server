#[cfg(test)]
mod example;

use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Core routing and handling trait. Implement this to add a route to the server.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::controller::Controller;
/// use rust_web_server::request::{METHOD, Request};
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
/// use rust_web_server::range::Range;
/// use rust_web_server::mime_type::MimeType;
/// use rust_web_server::server::ConnectionInfo;
///
/// pub struct HelloController;
///
/// impl Controller for HelloController {
///     fn is_matching(request: &Request, _: &ConnectionInfo) -> bool {
///         request.method == METHOD.get && request.request_uri == "/hello"
///     }
///
///     fn process(_: &Request, mut response: Response, _: &ConnectionInfo) -> Response {
///         response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///         response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///         response.content_range_list = vec![
///             Range::get_content_range(b"Hello!".to_vec(), MimeType::TEXT_PLAIN.to_string())
///         ];
///         response
///     }
/// }
/// ```
pub trait Controller {
    /// Returns `true` if this controller should handle the given request.
    /// Called in declaration order; the first match wins.
    fn is_matching(request: &Request, connection: &ConnectionInfo) -> bool;

    /// Produces the response. Receives the partially-built `response` (already populated
    /// with standard headers from [`Header::get_header_list`]) and must return it populated
    /// with a status code and body.
    fn process(request: &Request, response: Response, connection: &ConnectionInfo) -> Response;
}