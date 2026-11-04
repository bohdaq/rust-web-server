//! The HTTP layer of [rust-web-server](https://crates.io/crates/rust-web-server):
//! requests, responses, headers, ranges, bodies, JSON, extractors,
//! compression, CORS, errors, and the server's settings.
//!
//! Internal to rust-web-server, released with it at the same version: use
//! these through the `rust_web_server` paths (`rust_web_server::request`,
//! `rust_web_server::response`, …), which stay stable; this crate's own
//! layout may change in any release. See `spec/WORKSPACE_SPLIT.md`.
#![cfg_attr(docsrs, feature(doc_cfg))]

// The base layer, under the paths the moved code uses (`crate::core`, …).
pub(crate) use rws_base::{core, ext, http, mime_type, null, symbol, url};

pub mod app_manifest;
pub mod body;
pub mod client_hint;
pub mod compression;
pub mod cors;
pub mod early_hints;
pub mod entry_point;
pub mod error;
pub mod extract;
pub mod header;
pub mod ip_filter;
pub mod json;
pub mod range;
pub mod request;
pub mod response;
pub mod server_config;
#[doc(hidden)]
pub mod static_path;
pub mod virtual_host;

#[cfg(test)]
#[doc(hidden)]
pub mod test_env;
