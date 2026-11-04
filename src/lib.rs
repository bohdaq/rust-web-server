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
//! rust-web-server = "18"
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
//!
//! ## Features
//!
//! On docs.rs every item is shown, labelled with the features it needs. The
//! presets `web`, `api`, `proxy` and `full` turn on common sets; see
//! [Feature flags](https://rws8.tech/getting-started/installation/#feature-flags).
// docs.rs sets `--cfg docsrs` ([package.metadata.docs.rs]): nightly rustdoc
// then labels each item with the `cfg` it is compiled under.
#![cfg_attr(docsrs, feature(doc_cfg))]

// Allows `::rust_web_server::…` paths to resolve from within this crate's own
// tests, which is required by proc-macro derive output that uses that prefix.
extern crate self as rust_web_server;

pub use rws_server::app;
#[cfg(feature = "auth")]
pub use rws_server::auth;

#[cfg(feature = "macros")]
pub use rws_macros::{delete, get, patch, post, put, route, Config, FromPath, FromRequest, JsonSchema, Validate};
#[cfg(all(feature = "macros", any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
pub use rws_macros::Model;
#[cfg(feature = "http2")]
pub use rws_server::async_state;
#[cfg(feature = "http2")]
pub(crate) use rws_server::async_bridge;
pub use rws_server::session;
// Socket-coupled — see spec/WASM_SHIM.md for what a wasm32-wasip2 build
// excludes and why (no OS threads, no raw TCP/TLS in a WASI guest).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rws_server::redis_protocol;
pub use rws_server::sse;
pub use rws_http::compression;
pub use rws_server::cookie;
pub use rws_http::error;
pub use rws_http::extract;
#[cfg(all(test, feature = "macros"))]
mod extract_derive_tests;
#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod server_tests;
#[cfg(test)]
mod feature_tests;
pub mod i18n;
pub use rws_proxy::a2a;
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
pub use rws_server::systemd;
pub use rws_server::ip_filter;
pub mod macros;
pub use rws_server::blocklist;
pub use rws_server::cache;
pub use rws_server::early_hints;
pub use rws_server::disk_guard;
pub use rws_proxy::traffic;
pub use rws_server::authz;
pub use rws_server::config_reload;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::config_watch;
pub use rws_proxy::config_check;
/// RSA public keys and RS256 / rsa-sha256 signature verification (no `rsa` crate).
#[cfg(any(feature = "auth-asymmetric", feature = "sso"))]
pub use rws_server::rsa_key;
pub use rws_http::server_config;
pub use rws_server::feature;
pub use rws_server::maintenance;
pub use rws_server::metrics;
pub use rws_server::mcp;
pub use rws_server::request_log;
pub use rws_server::request_id;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::otel;
#[cfg(feature = "acme")]
pub use rws_acme::acme;
pub use rws_server::middleware;
pub use rws_server::rate_limit;
pub use rws_server::router;
pub use rws_server::handler;
pub use rws_server::schema;
pub use rws_server::state;
pub use rws_server::test_client;
pub use rws_server::application;
/// `rws.app.toml` and the conventional app layout (18.28.0).
pub use rws_http::app_manifest;
pub use rws_http::body;
pub use rws_http::client_hint;
pub use rws_server::controller;
pub use rws_base::core;
pub use rws_http::cors;
pub use rws_http::entry_point;
pub use rws_base::ext;
pub use rws_http::header;
pub use rws_base::http;
pub use rws_http::json;
pub use rws_server::language;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::log;
pub use rws_base::mime_type;
pub use rws_base::null;
pub use rws_http::range;
pub use rws_http::request;
pub use rws_http::response;
pub use rws_server::server;
pub use rws_base::symbol;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::thread_pool;
pub use rws_base::url;
pub use rws_server::pagination;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::proxy;
pub use rws_server::rewrite;
pub use rws_server::scheduler;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::tcp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::udp_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::ws_proxy;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::canary;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::circuit_breaker;
// `BackendPool`/`DiscoverySource` (DNS/etcd/Consul/Docker) are native-only —
// gated internally in service_discovery/mod.rs, not at this declaration —
// because `json_lite` (a plain JSON parser, no sockets) must stay available
// for `secrets`'s Vault/AWS-SM/Key-Vault backends on every target.
pub use rws_server::service_discovery;
/// DNS wire format, shared by the SRV service discovery and ACME's DNS-01.
pub(crate) use rws_base::dns;
pub use rws_server::config_binding;
pub use rws_server::di;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::proxy_config;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::llm;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::admin;
/// `app.dev_mcp()`: an MCP endpoint for a coding agent working on the app (18.28.0).
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::dev_mcp;
/// `rws gen model|route|migration|test`: code generators for the
/// conventional layout (18.29.0).
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::generator;
/// Fleet config pull: signed configs from HTTPS, S3 or git (18.23.0).
#[cfg(all(feature = "http2", not(target_arch = "wasm32")))]
pub use rws_proxy::fleet;
/// Web application firewall: rws's own rules (18.22.0).
pub use rws_proxy::waf;
/// Runtime state shared across replicas through Redis (18.22.0).
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::cluster;
/// FastCGI client: PHP through php-fpm (18.21.0).
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::fastcgi;
/// `rws ctl` / `rwsctl`: the admin API from the command line (18.20.0).
#[cfg(not(target_arch = "wasm32"))]
pub mod ctl;
#[cfg(feature = "wasm-plugins")]
pub use rws_wasm_plugin::wasm_plugin;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_proxy::ingress;
#[cfg(feature = "tera")]
pub mod template;
pub use rws_server::validate;
pub use rws_http::virtual_host;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub use rws_server::model;
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::websocket;
// Has both a native (TcpStream/rustls) and a wasm32 (wasi:http outgoing-handler)
// backend behind Client::send() — see the #[cfg(target_arch = "wasm32")]
// `send_once` in this module. spec/WASM_SHIM.md Phase 2 item 7.
pub use rws_server::http_client;
#[cfg(feature = "crypto")]
pub use rws_server::crypto;
#[cfg(feature = "csrf")]
pub use rws_server::csrf;
#[cfg(feature = "sso")]
pub use rws_server::sso;
// SMTP is a raw-TCP protocol with no wasi:http equivalent (unlike
// http_client, which the host can proxy via outgoing-handler) — genuinely
// can't work in a wasi:http guest, not just ungated. spec/WASM_SHIM.md.
#[cfg(all(feature = "mailer", not(target_arch = "wasm32")))]
pub mod mailer;
#[cfg(feature = "jobs")]
pub mod jobs;
// Also compiled under `secrets` — `secrets::aws_secrets_manager`/`azure_key_vault`
// reuse `StorageError` and the SigV4/Managed-Identity credential helpers from here.
#[cfg(any(feature = "storage-local", feature = "storage-s3", feature = "storage-azure", feature = "storage-gcs"))]
pub use rws_storage::storage;
// `secrets` alone needs only what rws-server's slim `storage` has.
#[cfg(all(feature = "secrets", not(any(feature = "storage-local", feature = "storage-s3", feature = "storage-azure", feature = "storage-gcs"))))]
pub use rws_server::storage;
#[cfg(feature = "openapi")]
pub use rws_server::openapi;
#[cfg(feature = "tower")]
pub use rws_tower::tower;
#[cfg(feature = "gateway-api")]
pub use rws_gateway_api::gateway_api;
#[cfg(feature = "webhook")]
pub mod webhook;
#[cfg(feature = "secrets")]
pub use rws_server::secrets;
// Runs wrapped work on a background OS thread to bound wait time — needs
// `std::thread::spawn`, unavailable on wasm32-wasip2. See spec/WASM_SHIM.md.
#[cfg(not(target_arch = "wasm32"))]
pub use rws_server::timeout;
pub mod prelude;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub use rws_server::tls;

#[cfg(feature = "http2")]
#[doc(hidden)]
pub use rws_server::h2_server;

#[cfg(feature = "http3")]
#[doc(hidden)]
pub use rws_server::h3_handler;

/// Shared infrastructure for tests that write process-wide environment variables.
///
/// Tests that call `override_environment_variables_from_config` or
/// `CommandLineArgument::set_environment_variable` must hold this lock for
/// their entire duration so they don't race with other tests reading the
/// same variables.
#[cfg(test)]
mod stack_depth_tests;

// The rustls crypto provider and client builder live in rws-server (its TLS
// code uses them); `FIPS` too.
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
pub use rws_server::crypto_provider;
pub use rws_server::FIPS;
#[cfg(all(any(feature = "http-client", feature = "http2"), not(target_arch = "wasm32")))]
pub(crate) use rws_server::tls_client_builder;

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
    /// A fleet signature file (`rws-fleet-sig v2 <serial> <sig>` or the
    /// 18.23 form) checked against a body (18.24.0). Must never panic or
    /// accept: no input is a valid signature for the fixed key.
    #[cfg(all(feature = "http2", not(target_arch = "wasm32")))]
    pub fn fleet_signature(data: &[u8]) {
        let _ = crate::fleet::parse_signature(data);
        let (body, sig) = data.split_at(data.len() / 2);
        assert!(crate::fleet::verify(body, sig, &[7u8; 32]).is_err(), "a fuzzed signature verified");
    }

    /// The shared base64 decoder (18.25.0), every alphabet and option:
    /// never panics, and whatever decodes re-encodes to the same bytes.
    pub fn base64_decode(data: &[u8]) {
        use crate::core::base64::{Base64, Decoder};
        for d in [Decoder::STANDARD, Decoder::URL_SAFE, Decoder::EITHER.skip_whitespace(), Decoder::EITHER.skip_invalid()] {
            if let Ok(bytes) = d.decode(data) {
                let again = Decoder::STANDARD.decode(Base64::encode(&bytes).unwrap()).unwrap();
                assert_eq!(bytes, again, "base64 round trip changed the bytes");
            }
        }
    }

    /// An RFC 7512 `pkcs11:` URI (18.24.0). Must never panic.
    #[cfg(feature = "pkcs11")]
    pub fn pkcs11_uri(data: &[u8]) {
        if let Ok(text) = std::str::from_utf8(data) {
            let _ = crate::tls::pkcs11::Pkcs11Uri::parse(text);
            let _ = crate::entry_point::shown("RWS_CONFIG_TLS_KEY_FILE", text);
        }
    }

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

    /// OpenAPI documents to MCP tools (#66): any bytes. Must never panic;
    /// every tool's schemas are JSON and its requests build or err cleanly.
    pub fn openapi_to_mcp(data: &[u8]) {
        let Ok(spec) = std::str::from_utf8(data) else { return };
        let options = crate::mcp::openapi_tools::ToolOptions::default();
        let Ok(found) = crate::mcp::openapi_tools::tools(spec, &options) else { return };
        for tool in found {
            assert!(crate::service_discovery::json_lite::parse(&tool.input_schema).is_ok());
            assert!(tool.name.len() <= 64 && tool.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'), "{}", tool.name);
            let _ = tool.request(r#"{"id":"1","body":{"a":[1,2]},"q":["x","y"]}"#);
        }
    }

    /// An A2A JSON-RPC request body (#65): any bytes. Must never panic, and
    /// the answer is always a JSON-RPC response (or a 413).
    pub fn a2a_request(data: &[u8]) {
        use crate::application::Application;
        thread_local! {
            static AGENT: crate::a2a::A2aServer = crate::a2a::A2aServer::new(crate::a2a::AgentCard::new("fuzz", "fuzz", "1"))
                .on_message(|ctx, message| match message.text().len() % 3 {
                    0 => crate::a2a::A2aReply::Message(crate::a2a::Message::agent_text(message.text())),
                    1 => crate::a2a::A2aReply::Task(ctx.input_required("more?")),
                    _ => crate::a2a::A2aReply::Task(ctx.complete(vec![crate::a2a::Artifact::text("a", message.text())])),
                });
        }
        let request = crate::request::Request {
            method: "POST".to_string(),
            request_uri: "/a2a".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![crate::header::Header::new("A2A-Version", "1.0".to_string())],
            body: data.to_vec().into(),
        };
        let connection = crate::server::ConnectionInfo::default();
        let response = AGENT.with(|agent| agent.execute(&request, &connection)).expect("A2A never errs");
        if response.status_code == 200 {
            let body = String::from_utf8(response.content_range_list[0].body.clone()).expect("UTF-8");
            assert!(crate::service_discovery::json_lite::parse(&body).is_ok(), "not JSON: {body}");
        }
    }

    /// `Accept-Language` parsing and negotiation (#33): any header value;
    /// at most `MAX_LANGUAGE_RANGES` ranges, and the language chosen is
    /// always one of those offered.
    pub fn accept_language(data: &[u8]) {
        let Ok(value) = std::str::from_utf8(data) else { return };
        let preferences = crate::i18n::AcceptLanguage::parse(value);
        assert!(preferences.0.len() <= crate::i18n::MAX_LANGUAGE_RANGES);
        let available = ["en", "en-GB", "uk", "pt-BR"];
        if let Some(chosen) = preferences.negotiate(&available) {
            assert!(available.contains(&chosen));
        }
    }

    /// i18n catalog text (#33): any input; whatever parses translates
    /// without panicking, plurals and placeholders included.
    pub fn i18n_catalog(data: &[u8]) {
        let Ok(text) = std::str::from_utf8(data) else { return };
        if let Ok(catalogs) = crate::i18n::Catalogs::new("en").add("en", text) {
            let t = catalogs.for_language("en");
            for line in text.lines() {
                let key = line.split('=').next().unwrap_or("").trim();
                let _ = t.t_with(key, &[("name", "x")]);
                let _ = t.plural(key.trim_end_matches(".one").trim_end_matches(".other"), 3);
            }
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

    /// What a WebAssembly plugin returns, as the host reads it (#40).
    /// Must never panic.
    #[cfg(feature = "wasm-plugins")]
    pub fn wasm_plugin_output(data: &[u8]) {
        let _ = crate::wasm_plugin::parse_request_output(data);
        let _ = crate::wasm_plugin::parse_response_output(data);
    }

    /// A disk-cache entry file as the cache reads it back (#39): the header
    /// line and key, then the stored response. Must never panic.
    pub fn cache_entry(data: &[u8]) {
        if let Some((_, _, _, offset)) = crate::cache::disk::parse_entry_header(data) {
            let _ = crate::response::Response::parse_upstream(&data[offset..], crate::response::REGENERATED_OR_HOP_BY_HOP);
        }
    }

    /// A request as the traffic classifier sees it: the first line is the
    /// User-Agent, the rest the path; the address comes from the first byte.
    pub fn traffic_classify(data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let (ua, path) = text.split_once('\n').unwrap_or((&text, "/"));
        let ip = std::net::IpAddr::from([data.first().copied().unwrap_or(1), 2, 3, 4]);
        let class = crate::traffic::classify(ip, Some(ua), path);
        let _ = (class.group(), class.kind.label());
    }

    /// Authorization text as the admin API and config file read it (#32):
    /// a tuple, a subject, and a relation definition.
    pub fn authz_parse(data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let _ = crate::authz::Tuple::parse(&text);
        let _ = crate::authz::Subject::parse(&text);
        if let Ok(terms) = crate::authz::rebac::parse_expression(&text) {
            let _ = terms.len();
            let _ = crate::authz::Schema::new().relation("t", "r", &text).and_then(|s| s.validate());
        }
    }

    /// An `Available-Dictionary` header value, and dictionary-compressed
    /// bodies through both decoders (#38).
    pub fn available_dictionary(data: &[u8]) {
        use crate::compression::dictionary::{decode, parse_available_dictionary, DictionaryEncoding};
        let _ = parse_available_dictionary(&String::from_utf8_lossy(data));
        for encoding in [DictionaryEncoding::Dcz, DictionaryEncoding::Dcb] {
            let _ = decode(encoding, data, b"dictionary", 1 << 16);
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
