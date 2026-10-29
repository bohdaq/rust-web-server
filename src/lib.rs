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
pub use rws_macros::{delete, get, patch, post, put, route, Config, FromRequest, JsonSchema, Validate};
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
#[cfg(not(target_arch = "wasm32"))]
pub mod config_watch;
pub mod config_check;
/// RSA public keys and RS256 / rsa-sha256 signature verification (no `rsa` crate).
#[cfg(any(feature = "auth-asymmetric", feature = "sso"))]
pub mod rsa_key;
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
pub mod schema;
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
/// DNS wire format, shared by the SRV service discovery and ACME's DNS-01.
pub(crate) mod dns;
pub mod config_binding;
pub mod di;
#[cfg(not(target_arch = "wasm32"))]
pub mod proxy_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod llm;
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
#[cfg(feature = "tower")]
pub mod tower;
#[cfg(feature = "gateway-api")]
pub mod gateway_api;
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
pub mod h2_server;

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

/// A rustls client-config builder with this crate's crypto provider
/// (aws-lc-rs) named explicitly. `rustls::ClientConfig::builder()` uses the
/// process-wide default provider and panics when none is installed and more
/// than one is compiled in — which happens as soon as any dependency enables
/// rustls's `ring` backend (e.g. sqlx). Every outbound TLS client in the
/// crate starts here.
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
pub(crate) fn tls_client_builder() -> rustls::ConfigBuilder<rustls::ClientConfig, rustls::WantsVerifier> {
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("aws-lc-rs supports rustls's default protocol versions")
}

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

        /// The same in the working directory (the static-file root), for
        /// tests that need a file the server would serve.
        pub fn new_in_cwd(name: &str) -> Self {
            TempPath(std::env::current_dir().expect("working directory").join(format!("rws_test_{}_{}", std::process::id(), name)))
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
            // A symlink is removed itself, never followed (a test may link
            // to a system directory).
            let is_link = std::fs::symlink_metadata(&self.0).is_ok_and(|m| m.file_type().is_symlink());
            if is_link {
                let _ = std::fs::remove_file(&self.0);
            } else if self.0.is_dir() {
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
    /// Proxy route-matching normalization of a request target (#50): any
    /// input, and normalizing twice changes nothing.
    pub fn proxy_normalize_target(data: &[u8]) {
        let Ok(target) = std::str::from_utf8(data) else { return };
        let once = crate::proxy_config::normalize_request_target(target).into_owned();
        let twice = crate::proxy_config::normalize_request_target(&once).into_owned();
        assert_eq!(once, twice, "normalization is not idempotent for {:?}", target);
    }

    /// Static-file request-path decoding (#55). Must never panic; the
    /// decoded path never carries a `..` segment past the web-root check.
    pub fn static_path_decode(data: &[u8]) {
        let Ok(path) = std::str::from_utf8(data) else { return };
        if let Ok(fs_path) = crate::app::controller::static_resource::static_fs_path(path) {
            let relative = &fs_path[crate::app::controller::static_resource::working_directory().len()..];
            assert!(!relative.split(['/', '\\']).any(|segment| segment == ".."), "{:?} -> {:?}", path, fs_path);
        }
    }

    /// `Accept-Encoding` negotiation (#7): any header value; the choice is
    /// always one of the offered codings, and a coding the value refuses
    /// (its first `q=` parameter is 0) is never chosen.
    pub fn accept_encoding(data: &[u8]) {
        let Ok(value) = std::str::from_utf8(data) else { return };
        let request = crate::request::Request {
            method: "GET".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![crate::header::Header::new(crate::header::Header::_ACCEPT_ENCODING, value.to_string())],
            body: Default::default(),
        };
        for preference in [crate::compression::DYNAMIC_PREFERENCE, crate::compression::STATIC_PREFERENCE] {
            if let Some(chosen) = crate::compression::negotiate(&request, preference) {
                assert!(preference.contains(&chosen));
                // Refused: the coding's first `q=` parameter is exactly 0.
                let refused = value.split(',').any(|c| {
                    let mut parts = c.split(';');
                    parts.next().unwrap_or("").trim().eq_ignore_ascii_case(chosen.token())
                        && parts
                            .map(str::trim)
                            .find(|p| p.as_bytes().get(..2).is_some_and(|b| b.eq_ignore_ascii_case(b"q=")))
                            .is_some_and(|q| q[2..].trim().parse::<f32>() == Ok(0.0))
                });
                let listed_once = value.split(',').filter(|c| c.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case(chosen.token())).count() == 1;
                assert!(!(refused && listed_once), "{value:?} refused {chosen:?}");
            }
        }
    }

    /// Provider responses as the LLM proxy reads them (#41): usage from a
    /// JSON body or an event stream fed in pieces, and chunked framing.
    /// Must never panic.
    pub fn llm_usage(data: &[u8]) {
        for content_type in ["application/json", "text/event-stream"] {
            let mut parser = crate::llm::UsageParser::new(content_type);
            for piece in data.chunks(7) {
                parser.feed(piece);
            }
            let _ = parser.finish();
        }
        let mut decoder = crate::llm::usage::ChunkDecoder::default();
        let mut out = Vec::new();
        let _ = decoder.decode(data, &mut out);
    }

    /// An upstream MCP server's reply as the gateway reads it (#62): a JSON
    /// body or an event stream, then the response's tools. Must never panic.
    pub fn mcp_gateway_reply(data: &[u8]) {
        for content_type in ["application/json", "text/event-stream"] {
            if let Ok(result) = crate::mcp::gateway::parse_reply(content_type, data, 1) {
                let _ = crate::mcp::gateway::fuzzing::tools(&result);
            }
        }
    }

    /// A WebSocket frame from a client (#8): any bytes; a frame never holds
    /// more payload than the limit allows. Must never panic.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn websocket_frame(data: &[u8]) {
        let mut cursor = std::io::Cursor::new(data);
        while let Ok(frame) = crate::websocket::WebSocket::read_frame_limited(&mut cursor, 4096) {
            let len = match &frame {
                crate::websocket::Frame::Text(t) => t.len(),
                crate::websocket::Frame::Binary(b) | crate::websocket::Frame::Ping(b) | crate::websocket::Frame::Pong(b) => b.len(),
                crate::websocket::Frame::Continuation { data, .. } => data.len(),
                crate::websocket::Frame::Close(_, reason) => reason.len() + 2,
            };
            assert!(len <= 4096 + 2);
        }
    }

    /// A DNS response read for SRV records (#8). Must never panic.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dns_srv_response(data: &[u8]) {
        let id = data.get(..2).map_or(0, |b| u16::from_be_bytes([b[0], b[1]]));
        let _ = crate::service_discovery::dns_srv::parse_response(data, id);
    }

    /// A DNS message: the shared parser (SRV discovery, ACME DNS-01), TXT
    /// values and, with `acme`, a DNS UPDATE reply. Must never panic.
    /// A certificate's subjectAltNames, as a BackendTLSPolicy checks a
    /// backend's (untrusted) certificate. Must never panic.
    pub fn x509_subject_alt_names(data: &[u8]) {
        let _ = crate::core::x509::subject_alt_names(data);
    }

    pub fn dns_message(data: &[u8]) {
        if let Ok(message) = crate::dns::parse(data) {
            for record in message.answers.iter().chain(&message.authority).chain(&message.additional) {
                let _ = crate::dns::txt_value(&data[record.rdata.clone()]);
            }
        }
        #[cfg(feature = "acme")]
        let _ = crate::acme::dns01::check_update_reply(data, 0);
    }

    /// A Redis (RESP) reply (#8). Must never panic.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn redis_reply(data: &[u8]) {
        let _ = crate::redis_protocol::read_reply(&mut std::io::Cursor::new(data));
    }

    /// A `Cookie` request header (#8): every parsed cookie has a name.
    pub fn cookie_header(data: &[u8]) {
        let Ok(value) = std::str::from_utf8(data) else { return };
        let jar = crate::cookie::CookieJar::parse(value);
        assert!(jar.cookies.iter().all(|c| !c.name.is_empty()));
    }

    /// A W3C `baggage` header (#25): members survive a round trip through
    /// the header. Must never panic.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn otel_baggage(data: &[u8]) {
        let Ok(value) = std::str::from_utf8(data) else { return };
        let baggage = crate::otel::Baggage::parse(value);
        assert!(baggage.len() <= crate::otel::baggage::MAX_MEMBERS);
        let again = crate::otel::Baggage::parse(&baggage.to_header());
        for (key, value) in baggage.iter() {
            if let Some(round_tripped) = again.get(key) {
                assert_eq!(value, round_tripped);
            }
        }
    }

    /// What ACME renewal parses from the CA (#1): certificate DER (validity,
    /// serial, Authority Key Identifier), renewalInfo windows (RFC 3339) and
    /// directory profile names. Must never panic.
    #[cfg(feature = "acme")]
    pub fn acme_renewal(data: &[u8]) {
        let _ = crate::acme::fuzzing::cert_info_from_der(data);
        if let Ok(text) = std::str::from_utf8(data) {
            let _ = crate::acme::fuzzing::parse_suggested_window(text);
            let _ = crate::acme::fuzzing::parse_rfc3339(text);
            let _ = crate::acme::fuzzing::json_object_keys(text, "profiles");
        }
    }

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
        for header in &request.headers {
            let _ = crate::http::date::parse(&header.value);
        }
        let _ = framing::validate_head(&pending[..head_len], &request);
        let _ = framing::keep_alive(&request);
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


/// Compiles the README's Rust example as a doctest, so it can't drift from the API.
#[cfg(all(doctest, feature = "http3"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
