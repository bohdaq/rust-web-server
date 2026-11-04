//! ACME for rust-web-server: certificates from Let's Encrypt and other ACME CAs (HTTP-01, TLS-ALPN-01, DNS-01, on-demand, shared storage).
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
pub use rws_server::{core, crypto_provider, dns, entry_point, http, http_client, metrics, redis_protocol, server, tls, tls_client_builder};

pub mod acme;

#[cfg(test)]
pub(crate) mod test_env;
