//! Convenience re-exports for writing handlers and running the server.
//!
//! A single glob import covers the types you need in almost every handler:
//!
//! ```rust,no_run
//! use rust_web_server::prelude::*;
//!
//! fn hello(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
//!     Response::get_response(
//!         STATUS_CODE_REASON_PHRASE.n200_ok,
//!         None,
//!         Some(vec![Range::get_content_range(
//!             b"Hello, world!".to_vec(),
//!             MimeType::TEXT_PLAIN.to_string(),
//!         )]),
//!     )
//! }
//!
//! fn main() {
//!     let app = App::with_state(()).get("/hello", hello);
//!     let (listener, pool) = Server::setup().unwrap();
//!     Server::run(listener, pool, app);
//! }
//! ```

pub use crate::app::App;
pub use crate::core::New;
pub use crate::mime_type::MimeType;
pub use crate::range::Range;
pub use crate::request::Request;
pub use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
pub use crate::router::PathParams;
pub use crate::routes;
pub use crate::server::ConnectionInfo;
#[cfg(not(target_arch = "wasm32"))]
pub use crate::server::Server;
