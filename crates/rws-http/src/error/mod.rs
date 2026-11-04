#[cfg(test)]
mod tests;

pub mod problem;
pub use problem::problem;

use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};

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

/// `Ok` and `Err` each become their own response, so a handler can return
/// `Result<Json<Post>, AppError>` (18.28.0).
impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(error) => error.into_response(),
        }
    }
}

/// `200 OK`, `text/plain` (18.28.0).
impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &self)
    }
}

/// `200 OK`, `text/plain` (18.28.0).
impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, self)
    }
}

/// The inner response with its status replaced:
/// `(STATUS_CODE_REASON_PHRASE.n201_created, Json(post))` (18.28.0).
impl<T: IntoResponse> IntoResponse for (&'static StatusCodeReasonPhrase, T) {
    fn into_response(self) -> Response {
        let (status, inner) = self;
        let mut response = inner.into_response();
        response.status_code = *status.status_code;
        response.reason_phrase = status.reason_phrase.to_string();
        response
    }
}

/// A built-in typed error that maps common failure cases to standard HTTP
/// status codes. Use it directly or as a model for your own error type.
///
/// # Example
///
/// ```rust,no_run
/// # extern crate rws_http as rust_web_server;
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
    /// 413 Payload Too Large — the request body exceeds a configured limit.
    PayloadTooLarge(String),
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
            AppError::PayloadTooLarge(msg)       => (STATUS_CODE_REASON_PHRASE.n413_payload_too_large, msg.as_str()),
            AppError::Internal(msg)             => (STATUS_CODE_REASON_PHRASE.n500_internal_server_error, msg.as_str()),
        };

        // Problem details (18.17.0); `error_format = "text"` keeps the bare
        // message as the body.
        let detail = match &self {
            AppError::Unauthorized | AppError::Forbidden | AppError::TooManyRequests => "",
            _ => body_str,
        };
        let dummy = Request {
            method: "GET".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![],
            body: Default::default(),
        };
        problem::with_headers(status, detail, body_str, Some(Header::get_header_list(&dummy)))
    }
}
