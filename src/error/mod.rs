#[cfg(test)]
mod tests;

use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// Implemented by any type that can be turned into an HTTP [`Response`].
///
/// Implement this on your application error enum so handlers can return
/// `Result<Response, MyError>` and the framework maps the error to the
/// correct HTTP status automatically.
///
/// [`Response`] itself implements `IntoResponse` as the identity conversion.
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

/// A built-in typed error that maps common failure cases to standard HTTP
/// status codes. Use it directly or as a model for your own error type.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::error::{AppError, IntoResponse};
/// use rust_web_server::response::Response;
///
/// fn find_user(id: u64) -> Result<Response, AppError> {
///     if id == 0 {
///         return Err(AppError::NotFound("user not found".to_string()));
///     }
///     Err(AppError::Internal("db error".to_string()))
/// }
///
/// // In your controller:
/// // let response = find_user(id).unwrap_or_else(|e| e.into_response());
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum AppError {
    /// 400 Bad Request — malformed input.
    BadRequest(String),
    /// 401 Unauthorized — authentication is required.
    Unauthorized,
    /// 403 Forbidden — authenticated but not permitted.
    Forbidden,
    /// 404 Not Found — the requested resource does not exist.
    NotFound(String),
    /// 409 Conflict — the request conflicts with current state.
    Conflict(String),
    /// 422 Unprocessable Entity — input is syntactically valid but semantically wrong.
    UnprocessableEntity(String),
    /// 429 Too Many Requests — client has exceeded the rate limit.
    TooManyRequests,
    /// 500 Internal Server Error — unexpected server-side failure.
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body_str) = match &self {
            AppError::BadRequest(msg)           => (STATUS_CODE_REASON_PHRASE.n400_bad_request, msg.as_str()),
            AppError::Unauthorized              => (STATUS_CODE_REASON_PHRASE.n401_unauthorized, "Unauthorized"),
            AppError::Forbidden                 => (STATUS_CODE_REASON_PHRASE.n403_forbidden, "Forbidden"),
            AppError::NotFound(msg)             => (STATUS_CODE_REASON_PHRASE.n404_not_found, msg.as_str()),
            AppError::Conflict(msg)             => (STATUS_CODE_REASON_PHRASE.n409_conflict, msg.as_str()),
            AppError::UnprocessableEntity(msg)  => (STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, msg.as_str()),
            AppError::TooManyRequests           => (STATUS_CODE_REASON_PHRASE.n429_too_many_requests, "Too Many Requests"),
            AppError::Internal(msg)             => (STATUS_CODE_REASON_PHRASE.n500_internal_server_error, msg.as_str()),
        };

        let dummy = Request {
            method: "GET".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![],
            body: vec![],
        };
        let header_list = Header::get_header_list(&dummy);
        let body = body_str.as_bytes().to_vec();
        let content_range = Range::get_content_range(body, MimeType::TEXT_PLAIN.to_string());
        Response::get_response(status, Some(header_list), Some(vec![content_range]))
    }
}
