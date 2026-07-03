#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::url::URL;

/// Types that can be extracted from a [`Request`].
///
/// Implement this trait to build reusable request-parsing logic that maps
/// cleanly to an HTTP error response on failure.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::extract::{Body, BodyText, Query, FromRequest};
/// use rust_web_server::request::Request;
///
/// // inside a Controller::process implementation
/// fn handle(request: &Request) {
///     let body = Body::from_request(request).unwrap();
///     let text = BodyText::from_request(request).unwrap();
///     let params = Query::from_request(request).unwrap();
///     let id = params.get("id").map(String::as_str).unwrap_or("");
/// }
/// ```
pub trait FromRequest: Sized {
    /// Extract `Self` from `request`, or return a ready-to-send error [`Response`].
    fn from_request(request: &Request) -> Result<Self, Response>;
}

/// Raw request body bytes.
///
/// Never fails — an empty body produces an empty `Vec`.
#[derive(Debug)]
pub struct Body(pub Vec<u8>);

impl FromRequest for Body {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Ok(Body(request.body.clone()))
    }
}

impl Body {
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Request body decoded as UTF-8 text.
///
/// Returns `400 Bad Request` if the body is not valid UTF-8.
#[derive(Debug)]
pub struct BodyText(pub String);

impl FromRequest for BodyText {
    fn from_request(request: &Request) -> Result<Self, Response> {
        String::from_utf8(request.body.clone())
            .map(BodyText)
            .map_err(|_| bad_request(request, "request body is not valid UTF-8"))
    }
}

impl BodyText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Parsed query parameters from the request URI.
///
/// Never fails — a URI with no query string produces an empty map.
#[derive(Debug)]
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::extract::{Query, FromRequest};
/// use rust_web_server::request::Request;
///
/// fn handle(request: &Request) {
///     let q = Query::from_request(request).unwrap();
///     let page = q.get("page").map(String::as_str).unwrap_or("1");
/// }
/// ```
pub struct Query(pub HashMap<String, String>);

impl FromRequest for Query {
    fn from_request(request: &Request) -> Result<Self, Response> {
        let query_str = request.request_uri
            .splitn(2, '?')
            .nth(1)
            .unwrap_or("");
        Ok(Query(URL::parse_query(query_str)))
    }
}

impl Query {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }
}

/// Clone of all request headers.
#[derive(Debug)]
pub struct RequestHeaders(pub Vec<Header>);

impl FromRequest for RequestHeaders {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Ok(RequestHeaders(request.headers.clone()))
    }
}

impl RequestHeaders {
    /// Return the value of the first header matching `name` (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.0.iter()
            .find(|h| h.name.to_lowercase() == lower)
            .map(|h| h.value.as_str())
    }
}

/// The request's correlation ID (`X-Request-Id` by default — see
/// [`crate::request_id::DEFAULT_HEADER`]), for handlers that want it without
/// reaching for [`RequestHeaders`] directly.
///
/// Never fails: if [`crate::request_id::RequestIdLayer`] isn't wrapping this
/// app (or the caller didn't send the header), `.0` is an empty string.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl FromRequest for RequestId {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Ok(RequestId(
            request
                .get_header(crate::request_id::DEFAULT_HEADER.to_string())
                .map(|h| h.value.clone())
                .unwrap_or_default(),
        ))
    }
}

impl RequestId {
    /// Returns the ID as a string slice, or `""` if none was set.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn bad_request(request: &Request, msg: &str) -> Response {
    let header_list = Header::get_header_list(request);
    let cr = Range::get_content_range(msg.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string());
    Response::get_response(
        STATUS_CODE_REASON_PHRASE.n400_bad_request,
        Some(header_list),
        Some(vec![cr]),
    )
}
