# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build (http3 is the default — includes HTTP/3, HTTP/2, and TLS)
cargo build

# Build HTTP/2 + TLS only (no QUIC/HTTP/3)
cargo build --no-default-features --features http2

# Build HTTP/1.1-only (no TLS, lightest binary)
cargo build --no-default-features --features http1

# Run (falls back to plain HTTP/1.1 if no cert is configured)
cargo run

# Run with HTTPS + HTTP/2 + HTTP/3 active
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem

# Run all tests
cargo test

# Run a single test (replace the test path with the one you want)
cargo test --package rust-web-server --bin rws client_hint::tests::client_hints_header -- --exact
```

MSRV is 1.75. The `--ignore-rust-version` flag from older docs is no longer needed.

## Architecture

The server is a synchronous, thread-pool-based HTTP/1.1 server built with no async runtime and zero third-party HTTP dependencies. Everything — HTTP parsing, JSON, CORS, MIME types, range requests — is implemented from scratch in this repo.

### Request lifecycle

`main.rs` → `Server::setup()` binds the TCP listener → `Server::run()` accepts connections → dispatches each to the `ThreadPool` → `Server::process()` reads bytes, calls `Request::parse()`, then `app.execute()` → `Response::generate_response()` writes bytes back.

### Routing / controller dispatch

`App` (in `src/app/mod.rs`) implements the `Application` trait. It walks a hardcoded list of `if Controller::is_matching(...)` checks and calls the first matching controller's `process()`. There is no router table — controllers are checked in declaration order.

The `Controller` trait (`src/controller/mod.rs`) has two methods:
- `is_matching(request, connection) -> bool`
- `process(request, response, connection) -> Response`

To add a route: create a new module under `src/app/controller/`, implement `Controller`, and add the `is_matching`/`process` call to `App::execute` in `src/app/mod.rs`.

### Configuration

Configuration is layered (lowest → highest priority):
1. Defaults hardcoded in `src/entry_point/mod.rs` (`Config::*_DEFAULT_VALUE`)
2. System environment variables
3. `rws.config.toml` in the working directory
4. Command-line args (`rws.command_line`)

All config is read at startup into process environment variables (`RWS_CONFIG_*`) and then accessed globally via `env::var(...)`. There is no config struct passed around at runtime.

### Key types

- `Request` (`src/request/mod.rs`) — method, request_uri, http_version, headers, body (raw bytes)
- `Response` (`src/response/mod.rs`) — status code, headers, body as `Vec<ContentRange>`
- `Header` (`src/header/mod.rs`) — name/value pair; constants for all standard header names
- `ConnectionInfo` (`src/server/mod.rs`) — client/server IP+port + request allocation size, passed into every controller

### HTTP version constants

`src/http/mod.rs` defines `VERSION` (HTTP/0.9 through HTTP/3.0) and `HTTP::version_list()` which is used by `Request::parse_method_and_request_uri_and_http_version_string` to validate the version token on every incoming request.

### No async (default `http1` feature)

The server uses `std::net::TcpListener` and a hand-rolled `ThreadPool` (`src/thread_pool/mod.rs`). Each connection is handled synchronously on a worker thread.

### HTTP/2 feature (`--features http2`)

When built with the `http2` feature, the binary uses a `tokio` runtime and serves TLS via `rustls` (aws-lc-rs crypto backend). ALPN negotiation selects HTTP/2 (`h2` crate) or HTTP/1.1 per connection automatically. New modules:

- `src/tls/mod.rs` — builds the `TlsAcceptor` from PEM cert/key files; set `RWS_CONFIG_TLS_CERT_FILE` and `RWS_CONFIG_TLS_KEY_FILE` env vars or in `rws.config.toml`.
- `src/h2_handler/mod.rs` — translates `h2::RecvStream` requests into `crate::request::Request`, calls `app.execute()`, translates `Response` back into H2 frames. The `Application` and `Controller` traits are untouched.
- `src/server/mod.rs::Server::run_tls()` — async accept loop; routes each TLS connection by ALPN to either `h2_handler::handle_connection` or `Server::process_h1_tls`.
- HTTP/1.1 TLS responses include `Alt-Svc: h3=":PORT"` (or `h2` when http3 feature is absent) to advertise protocol availability.
- Forbidden HTTP/2 headers (`connection`, `keep-alive`, `transfer-encoding`, `upgrade`, `proxy-connection`, `te`) are stripped from responses before sending.

### HTTP/3 feature (`--features http3`, default)

When built with the `http3` feature (which implies `http2`), a second listener starts over UDP using QUIC (`quinn` crate). HTTP/3 uses `h3` + `h3-quinn`. New additions:

- `src/tls/mod.rs::create_quinn_server_config()` — builds a `quinn::ServerConfig` from the same PEM cert/key, with ALPN set to `h3`.
- `src/h3_handler/mod.rs` — accepts QUIC connections, resolves H3 streams via `RequestResolver::resolve_request()`, calls `app.execute()`, sends H3 responses.
- `src/server/mod.rs::Server::run_quic()` — binds a UDP endpoint on the same port as TCP; skipped silently if no cert is configured.
- `main()` runs `Server::run_tls` and `Server::run_quic` concurrently via `tokio::join!`.
