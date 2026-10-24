//! # rust-web-server
//!
//! An HTTP/1.1, HTTP/2, and HTTP/3 (QUIC) server, reverse proxy, and
//! application framework in one crate. HTTP/1.1 parsing, routing, middleware,
//! JSON, WebSocket, and SSE are implemented in-crate; HTTP/2 and HTTP/3
//! framing use the `h2` and `quinn`/`h3` crates, and TLS uses `rustls`.
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
//! ## Quick start
//!
//! ```rust,no_run
//! use rust_web_server::prelude::*;
//!
//! fn hello(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
//!     Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "Hello, world!")
//! }
//!
//! fn main() {
//!     let app = App::with_state(()).get("/hello", hello);
//!     let (listener, pool) = Server::setup().unwrap();
//!     Server::run(listener, pool, app);
//! }
//! ```
//!
//! See [DEVELOPER.md](https://github.com/bohdaq/rust-web-server/blob/main/DEVELOPER.md)
//! for the full building blocks reference and use case examples.

// Allows `::rust_web_server::…` paths to resolve from within this crate's own
// tests, which is required by proc-macro derive output that uses that prefix.
extern crate self as rust_web_server;

pub mod app;
#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "macros")]
pub use rws_macros::{delete, get, patch, post, put, route, Config, FromRequest, Validate};
#[cfg(all(feature = "macros", any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
pub use rws_macros::Model;
#[cfg(feature = "http2")]
pub mod async_state;
#[cfg(feature = "http2")]
pub(crate) mod async_bridge;
pub mod session;
// Socket-coupled — see spec/WASM_SHIM.md for what a wasm32-wasip2 build
// excludes and why (no OS threads, no raw TCP/TLS in a WASI guest).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod redis_protocol;
pub mod sse;
pub mod compression;
pub mod cookie;
pub mod error;
pub mod extract;
pub mod ip_filter;
pub mod macros;
pub mod blocklist;
pub mod cache;
pub mod config_reload;
pub mod server_config;
pub mod feature;
pub mod maintenance;
pub mod metrics;
pub mod mcp;
pub mod request_log;
pub mod request_id;
#[cfg(not(target_arch = "wasm32"))]
pub mod otel;
#[cfg(feature = "acme")]
pub mod acme;
pub mod middleware;
pub mod rate_limit;
pub mod router;
pub mod state;
pub mod test_client;
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
#[cfg(not(target_arch = "wasm32"))]
pub mod log;
pub mod mime_type;
pub mod null;
pub mod range;
pub mod request;
pub mod response;
pub mod server;
pub mod symbol;
#[cfg(not(target_arch = "wasm32"))]
pub mod thread_pool;
pub mod url;
pub mod pagination;
#[cfg(not(target_arch = "wasm32"))]
pub mod proxy;
pub mod rewrite;
pub mod scheduler;
#[cfg(not(target_arch = "wasm32"))]
pub mod tcp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod udp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod ws_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod canary;
#[cfg(not(target_arch = "wasm32"))]
pub mod circuit_breaker;
// `BackendPool`/`DiscoverySource` (DNS/etcd/Consul/Docker) are native-only —
// gated internally in service_discovery/mod.rs, not at this declaration —
// because `json_lite` (a plain JSON parser, no sockets) must stay available
// for `secrets`'s Vault/AWS-SM/Key-Vault backends on every target.
pub mod service_discovery;
pub mod config_binding;
pub mod di;
#[cfg(not(target_arch = "wasm32"))]
pub mod proxy_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod ingress;
#[cfg(feature = "tera")]
pub mod template;
pub mod validate;
pub mod virtual_host;
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
// SMTP is a raw-TCP protocol with no wasi:http equivalent (unlike
// http_client, which the host can proxy via outgoing-handler) — genuinely
// can't work in a wasi:http guest, not just ungated. spec/WASM_SHIM.md.
#[cfg(all(feature = "mailer", not(target_arch = "wasm32")))]
pub mod mailer;
#[cfg(feature = "jobs")]
pub mod jobs;
// Also compiled under `secrets` — `secrets::aws_secrets_manager`/`azure_key_vault`
// reuse `StorageError` and the SigV4/Managed-Identity credential helpers from here.
#[cfg(any(feature = "storage-local", feature = "storage-s3", feature = "storage-azure", feature = "secrets"))]
pub mod storage;
#[cfg(feature = "openapi")]
pub mod openapi;
#[cfg(feature = "webhook")]
pub mod webhook;
#[cfg(feature = "secrets")]
pub mod secrets;
// Runs wrapped work on a background OS thread to bound wait time — needs
// `std::thread::spawn`, unavailable on wasm32-wasip2. See spec/WASM_SHIM.md.
#[cfg(not(target_arch = "wasm32"))]
pub mod timeout;
pub mod prelude;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod tls;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub mod h2_handler;

#[cfg(feature = "http3")]
#[doc(hidden)]
pub mod h3_handler;

/// Shared infrastructure for tests that write process-wide environment variables.
///
/// Tests that call `override_environment_variables_from_config` or
/// `CommandLineArgument::set_environment_variable` must hold this lock for
/// their entire duration so they don't race with other tests reading the
/// same variables.
#[cfg(test)]
mod stack_depth_tests;

#[cfg(test)]
pub mod test_env {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    pub fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// A per-process path under the OS temp dir that is deleted (file or
    /// directory) when dropped — so a test cleans up even when an assertion
    /// fails before its last line.
    pub struct TempPath(std::path::PathBuf);

    impl TempPath {
        pub fn new(name: &str) -> Self {
            TempPath(std::env::temp_dir().join(format!("rws_test_{}_{}", std::process::id(), name)))
        }

        pub fn as_str(&self) -> &str {
            self.0.to_str().expect("temp dir path is UTF-8")
        }
    }

    impl std::ops::Deref for TempPath {
        type Target = std::path::Path;
        fn deref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            if self.0.is_dir() {
                let _ = std::fs::remove_dir_all(&self.0);
            } else {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
}

/// Entry points for `fuzz/` targets that reach crate-private parsers.
/// Compiled only under `--cfg fuzzing`, which `cargo fuzz` sets — never part
/// of the public API.
#[cfg(fuzzing)]
#[doc(hidden)]
pub mod fuzz_api {
    /// The SAML XML parser (`sso::saml::xml`), fed IdP-supplied documents.
    #[cfg(feature = "sso-saml")]
    pub fn saml_xml_parse(xml: &str) {
        let _ = crate::sso::saml::xml_parse_for_fuzzing(xml);
    }

    /// HTTP/1.1 request framing (`server::framing`) over a raw connection
    /// buffer: head detection, body framing, and the chunked body decoder,
    /// fed in two pieces split at the first byte to mimic separate reads.
    pub fn http1_framing(data: &[u8]) {
        use crate::server::framing::{self, BodyFraming, ChunkedBody, HeadScan};
        let mut pending = data.to_vec();
        framing::skip_leading_empty_lines(&mut pending);
        let HeadScan::Complete(head_len) = framing::scan_head(&pending) else { return };
        assert!(head_len <= pending.len() && head_len <= framing::MAX_HEAD_SIZE);
        let Ok(request) = crate::request::Request::parse(&pending[..head_len]) else { return };
        let body = &pending[head_len..];
        if let Ok(BodyFraming::Chunked) = framing::body_framing(&request) {
            let split = data.first().map_or(0, |&b| b as usize).min(body.len());
            let mut decoder = ChunkedBody::new(1 << 20);
            let mut buffer = body[..split].to_vec();
            for next in [&body[split..], &[][..]] {
                match decoder.feed(&buffer) {
                    Ok((consumed, _)) => {
                        assert!(consumed <= buffer.len());
                        buffer.drain(..consumed);
                    }
                    Err(_) => return,
                }
                buffer.extend_from_slice(next);
            }
            assert!(decoder.body.len() <= 1 << 20);
        }
    }
}

