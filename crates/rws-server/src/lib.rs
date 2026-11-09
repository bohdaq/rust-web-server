//! The server layer of [rust-web-server](https://crates.io/crates/rust-web-server):
//! the HTTP/1.1, HTTP/2 and HTTP/3 servers, TLS, the `App` and its
//! controllers, routing, state-aware apps, middleware, MCP, OpenAPI, SSO,
//! sessions, metrics, OpenTelemetry, the HTTP client and the model layer.
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
pub use rws_base::{core, dns, ext, http, mime_type, null, symbol, url};
// Not every module is used in every feature set.
#[allow(unused_imports)]
#[doc(hidden)]
pub use rws_http::{app_manifest, body, client_hint, compression, cors, entry_point, error, extract, header, json, range, request, response, server_config, virtual_host};

pub mod app;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "http2")]
pub mod async_state;
#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod async_bridge;
pub mod session;
// Socket-coupled — see spec/WASM_SHIM.md for what a wasm32-wasip2 build
// excludes and why (no OS threads, no raw TCP/TLS in a WASI guest).
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod redis_protocol;
pub mod sse;
pub mod cookie;
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
pub mod systemd;
pub mod ip_filter;
pub mod blocklist;
pub mod cache;
pub mod early_hints;
pub mod disk_guard;
pub mod authz;
pub mod config_reload;
#[cfg(not(target_arch = "wasm32"))]
pub mod config_watch;
/// RSA public keys and RS256 / rsa-sha256 signature verification (no `rsa` crate).
#[cfg(any(feature = "auth-asymmetric", feature = "sso"))]
pub mod rsa_key;
pub mod feature;
pub mod maintenance;
pub mod metrics;
pub mod mcp;
pub mod request_log;
pub mod request_id;
#[cfg(not(target_arch = "wasm32"))]
pub mod otel;
pub mod middleware;
pub mod rate_limit;
pub mod router;
pub mod group;
pub mod flash;
#[cfg(feature = "serde")]
pub mod vite;
pub mod channels;
pub mod handler;
pub mod schema;
pub mod state;
pub mod test_client;
pub mod application;
pub mod controller;
pub mod language;
#[cfg(not(target_arch = "wasm32"))]
pub mod log;
pub mod server;
#[cfg(not(target_arch = "wasm32"))]
pub mod thread_pool;
pub mod pagination;
pub mod rewrite;
pub mod scheduler;
#[cfg(not(target_arch = "wasm32"))]
pub mod circuit_breaker;
// `BackendPool`/`DiscoverySource` (DNS/etcd/Consul/Docker) are native-only —
// gated internally in service_discovery/mod.rs, not at this declaration —
// because `json_lite` (a plain JSON parser, no sockets) must stay available
// for `secrets`'s Vault/AWS-SM/Key-Vault backends on every target.
pub mod service_discovery;
pub mod config_binding;
pub mod di;
pub mod validate;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;
// Has both a native (TcpStream/rustls) and a wasm32 (wasi:http outgoing-handler)
// backend behind Client::send() — see the #[cfg(target_arch = "wasm32")]
// `send_once` in this module. spec/WASM_SHIM.md Phase 2 item 7.
pub mod http_client;
#[cfg(feature = "crypto")]
pub mod crypto;
#[cfg(feature = "csrf")]
pub mod csrf;
#[cfg(feature = "sso")]
pub mod sso;
#[cfg(feature = "openapi")]
pub mod openapi;
#[cfg(feature = "secrets")]
pub mod secrets;
// Runs wrapped work on a background OS thread to bound wait time — needs
// `std::thread::spawn`, unavailable on wasm32-wasip2. See spec/WASM_SHIM.md.
#[cfg(not(target_arch = "wasm32"))]
pub mod timeout;
#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod tls;
#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod h2_server;
#[cfg(feature = "http3")]
#[doc(hidden)]
pub mod h3_handler;

// `StorageError` and the credential helpers `secrets` shares with the
// storage backends (rust-web-server's `storage`).
#[cfg(any(feature = "storage-local", feature = "storage-s3", feature = "storage-azure", feature = "storage-gcs", feature = "secrets"))]
pub mod storage;

/// The rustls crypto provider every TLS config uses: aws-lc-rs, or with the
/// `fips` feature its FIPS 140-3 validated module and only the algorithms
/// FIPS approves (18.23.0).
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
pub fn crypto_provider() -> rustls::crypto::CryptoProvider {
    #[cfg(feature = "fips")]
    return rustls::crypto::default_fips_provider();
    #[cfg(not(feature = "fips"))]
    rustls::crypto::aws_lc_rs::default_provider()
}

/// Whether this build uses the FIPS module (`fips` feature).
pub const FIPS: bool = cfg!(feature = "fips");

/// A rustls client-config builder with this crate's crypto provider
/// (aws-lc-rs) named explicitly. `rustls::ClientConfig::builder()` uses the
/// process-wide default provider and panics when none is installed and more
/// than one is compiled in — which happens as soon as any dependency enables
/// rustls's `ring` backend (e.g. sqlx). Every outbound TLS client in the
/// crate starts here.
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn tls_client_builder() -> rustls::ConfigBuilder<rustls::ClientConfig, rustls::WantsVerifier> {
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(crate::crypto_provider()))
        .with_safe_default_protocol_versions()
        .expect("aws-lc-rs supports rustls's default protocol versions")
}


#[cfg(test)]
pub(crate) mod test_env;

// Derives (rws-macros) for this crate's tests and doctests; their output
// names `rust_web_server::…`, which is this crate there (`extern crate self`
// above, `extern crate rws_server as rust_web_server` in doctests).
#[cfg(feature = "macros")]
#[doc(hidden)]
pub use rws_macros::{Config, FromPath, FromRequest, JsonSchema, Validate};
