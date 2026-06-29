//! # rust-web-server
//!
//! A static file web server and HTTP toolkit written in Rust.
//! Supports HTTP/3 (QUIC), HTTP/2, and HTTP/1.1.
//!
//! ## Use as a library
//!
//! Add to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rust-web-server = "17"
//! ```
//!
//! ## Quick start: add a custom route
//!
//! ```rust,no_run
//! use rust_web_server::controller::Controller;
//! use rust_web_server::request::{METHOD, Request};
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//! use rust_web_server::range::Range;
//! use rust_web_server::mime_type::MimeType;
//! use rust_web_server::server::ConnectionInfo;
//!
//! pub struct PingController;
//!
//! impl Controller for PingController {
//!     fn is_matching(request: &Request, _: &ConnectionInfo) -> bool {
//!         request.method == METHOD.get && request.request_uri == "/ping"
//!     }
//!
//!     fn process(_: &Request, mut response: Response, _: &ConnectionInfo) -> Response {
//!         response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         response.content_range_list = vec![
//!             Range::get_content_range(b"pong".to_vec(), MimeType::TEXT_PLAIN.to_string())
//!         ];
//!         response
//!     }
//! }
//! ```
//!
//! See [DEVELOPER.md](https://github.com/bohdaq/rust-web-server/blob/main/DEVELOPER.md)
//! for the full building blocks reference and use case examples.

pub mod app;
pub mod compression;
pub mod cookie;
pub mod application;
pub mod body;
pub mod client_hint;
pub mod controller;
pub mod core;
pub mod cors;
pub mod entry_point;
pub mod ext;
pub mod header;
pub mod http;
pub mod json;
pub mod language;
pub mod log;
pub mod mime_type;
pub mod null;
pub mod range;
pub mod request;
pub mod response;
pub mod server;
pub mod symbol;
pub mod thread_pool;
pub mod url;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod tls;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod h2_handler;

#[cfg(feature = "http3")]
#[doc(hidden)]
pub mod h3_handler;
