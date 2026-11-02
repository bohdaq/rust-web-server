//! Request and response bodies.
//!
//! A request body is raw bytes (`Request::body: Vec<u8>`). A response body is a
//! list of [`ContentRange`](crate::range::ContentRange)s — usually one, several
//! for a `multipart/byteranges` (range) response.
//!
//! Parsers for the common request encodings:
//!
//! - [`form_urlencoded`] — `application/x-www-form-urlencoded`
//! - [`multipart_form_data`] — `multipart/form-data` (each part has headers,
//!   `Content-Disposition` required, and a body)
//! - JSON — see [`crate::json`]
//!
//! `HEAD` and `OPTIONS` requests carry no body. Examples: `src/body/example/`.

pub mod form_urlencoded;
pub mod multipart_form_data;

#[cfg(test)]
mod example;