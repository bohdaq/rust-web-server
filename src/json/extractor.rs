//! Serde-based JSON extractor and responder (`serde` feature).

#[cfg(test)]
mod tests;

use serde::{de::DeserializeOwned, Serialize};

use crate::core::New;
use crate::error::{AppError, IntoResponse};
use crate::extract::FromRequest;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// A JSON extractor and responder backed by `serde_json`.
///
/// Wrap a [`Deserialize`][serde::Deserialize] type to extract JSON from a
/// request body, or wrap a [`Serialize`][serde::Serialize] type to produce
/// a `200 OK application/json` response.
///
/// # Deserializing (request → typed struct)
///
/// ```rust,no_run
/// use serde::Deserialize;
/// use rust_web_server::json::Json;
/// use rust_web_server::request::Request;
///
/// #[derive(Deserialize)]
/// struct CreateUser { name: String, age: u32 }
///
/// fn handler(req: &Request) -> Result<(), rust_web_server::response::Response> {
///     let Json(payload) = Json::<CreateUser>::from_request(req)?;
///     // payload.name, payload.age
///     Ok(())
/// }
/// ```
///
/// # Serializing (typed struct → response)
///
/// ```rust,no_run
/// use serde::Serialize;
/// use rust_web_server::json::Json;
///
/// #[derive(Serialize)]
/// struct UserResponse { id: u64, name: String }
///
/// let response = Json(UserResponse { id: 1, name: "Alice".to_string() }).into_response();
/// ```
#[derive(Debug)]
pub struct Json<T>(pub T);

impl<T> std::ops::Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: DeserializeOwned> Json<T> {
    /// Deserialize `T` from the request body as JSON.
    ///
    /// Returns a `400 Bad Request` response on parse failure.
    pub fn from_request(request: &Request) -> Result<Json<T>, Response> {
        serde_json::from_slice(&request.body)
            .map(Json)
            .map_err(|e| AppError::BadRequest(e.to_string()).into_response())
    }
}

impl<T: DeserializeOwned> FromRequest for Json<T> {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Json::from_request(request)
    }
}

impl<T: Serialize> Json<T> {
    /// Serialize `T` as JSON and return a `200 OK` response with
    /// `Content-Type: application/json`.
    ///
    /// Returns `500 Internal Server Error` if serialization fails (rare for
    /// well-formed `Serialize` implementations).
    pub fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(body) => {
                let mut response = Response::new();
                response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                response.reason_phrase =
                    STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                response.content_range_list = vec![Range::get_content_range(
                    body,
                    MimeType::APPLICATION_JSON.to_string(),
                )];
                response
            }
            Err(e) => AppError::Internal(e.to_string()).into_response(),
        }
    }
}
