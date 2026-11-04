//! The proxy and operations layer of rust-web-server: reverse proxy, config-driven routes, load balancing, L4 and WebSocket proxies, the admin API, cluster sync, fleet pull, WAF, LLM and A2A gateways, FastCGI, traffic insights.
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
pub use rws_server::{app, application, authz, blocklist, cache, config_reload, core, disk_guard, early_hints, entry_point, error, ext, feature, header, http, http_client, ip_filter, maintenance, mcp, metrics, middleware, range, rate_limit, request, request_log, response, rewrite, router, server, server_config, service_discovery, session, sse, state, url};
#[cfg(feature = "auth")]
#[doc(hidden)]
pub use rws_server::auth;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::circuit_breaker;
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use rws_server::crypto_provider;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::log;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::otel;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::redis_protocol;
#[cfg(feature = "sso")]
#[doc(hidden)]
pub use rws_server::sso;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
#[doc(hidden)]
pub use rws_server::model;
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
#[doc(hidden)]
pub use rws_server::systemd;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::timeout;
#[cfg(feature = "http2")]
#[doc(hidden)]
pub use rws_server::tls;
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use rws_server::tls_client_builder;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use rws_server::websocket;
#[cfg(feature = "acme")]
#[doc(hidden)]
pub use rws_acme::acme;
#[cfg(feature = "storage-s3")]
#[doc(hidden)]
pub use rws_storage::storage;
#[cfg(feature = "wasm-plugins")]
#[doc(hidden)]
pub use rws_wasm_plugin::wasm_plugin;

#[cfg(not(target_arch = "wasm32"))]
pub mod proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod proxy_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod admin;
/// `app.dev_mcp()`: an MCP endpoint for a coding agent (18.28.0).
#[cfg(not(target_arch = "wasm32"))]
pub mod dev_mcp;
/// Runtime state shared across replicas through Redis (18.22.0).
#[cfg(not(target_arch = "wasm32"))]
pub mod cluster;
/// Fleet config pull: signed configs from HTTPS, S3 or git (18.23.0).
#[cfg(all(feature = "http2", not(target_arch = "wasm32")))]
pub mod fleet;
pub mod config_check;
#[cfg(not(target_arch = "wasm32"))]
pub mod llm;
#[cfg(not(target_arch = "wasm32"))]
pub mod tcp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod udp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod ws_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod canary;
#[cfg(not(target_arch = "wasm32"))]
pub mod ingress;
/// Web application firewall: rws's own rules (18.22.0).
pub mod waf;
pub mod a2a;
/// FastCGI client: PHP through php-fpm (18.21.0).
#[cfg(not(target_arch = "wasm32"))]
pub mod fastcgi;
pub mod traffic;

#[cfg(test)]
pub(crate) mod test_env;
// Lower modules only the tests use.
#[cfg(test)]
pub(crate) use rws_server::test_client;
#[cfg(all(test, feature = "http2"))]
pub(crate) use rws_server::h2_server;
#[cfg(all(test, feature = "http3"))]
pub(crate) use rws_server::h3_handler;
