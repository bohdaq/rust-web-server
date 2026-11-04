//! The lowest layer of [rust-web-server](https://crates.io/crates/rust-web-server):
//! string symbols, small helpers, base64 / SHA-256 / X.509 helpers, MIME
//! types, URL parsing, HTTP versions and dates, and DNS wire format.
//!
//! Internal to rust-web-server, released with it at the same version: use
//! these through the `rust_web_server` paths (`rust_web_server::core`,
//! `rust_web_server::mime_type`, …), which stay stable; this crate's own
//! layout may change in any release. See `spec/WORKSPACE_SPLIT.md`.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod core;
pub mod ext;
pub mod http;
pub mod mime_type;
pub mod null;
pub mod symbol;
pub mod url;
/// DNS wire format, shared by the SRV service discovery and ACME's DNS-01.
#[doc(hidden)]
pub mod dns;
