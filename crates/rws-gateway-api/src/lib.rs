//! The Kubernetes Gateway API controller of rust-web-server (kube).
//!
//! Internal to rust-web-server, released with it at the same version: use
//! these through the `rust_web_server` paths, which stay stable; this
//! crate's own layout may change in any release. See
//! `spec/WORKSPACE_SPLIT.md`.
#![cfg_attr(docsrs, feature(doc_cfg))]

// Derive output (rws-macros) names `::rust_web_server::…`; in this crate's
// tests that is this crate.
#[cfg(test)]
extern crate self as rust_web_server;

// The layers below, under the paths the moved code uses (`crate::request`, …);
// public (hidden) so doctests can name them as `rust_web_server::…`.
#[allow(unused_imports)]
#[doc(hidden)]
pub use rws_server::{application, core, error, header, http_client, range, request, response, server, url};
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use rws_server::crypto_provider;
#[cfg(feature = "http2")]
#[doc(hidden)]
pub use rws_server::tls;
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use rws_server::tls_client_builder;
#[allow(unused_imports)]
#[doc(hidden)]
pub use rws_proxy::{proxy};

pub mod gateway_api;

#[cfg(test)]
pub(crate) mod test_env;
