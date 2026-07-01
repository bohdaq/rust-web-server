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

`App` (in `src/app/mod.rs`) implements the `Application` trait (`src/application/mod.rs`). `Application::execute` returns `Result<Response, String>`. `App::execute` walks a hardcoded list of `if Controller::is_matching(...)` checks and calls the first matching controller's `process()`. Controllers are checked in declaration order.

The `Controller` trait (`src/controller/mod.rs`) has two methods:
- `is_matching(request, connection) -> bool`
- `process(request, response, connection) -> Response`

Two ways to add routes:
1. **Controller pattern** — create a module under `src/app/controller/`, implement `Controller`, register it in `App::execute`.
2. **Router** — build a `Router` inside `Application::execute` and call `router.handle(request, connection)`. Prefer this for new code when you need named path parameters or don't want boilerplate controllers.

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
- `ConnectionInfo` (`src/server/mod.rs`) — client/server IP+port, request allocation size, and `sni_hostname: Option<String>` (SNI hostname from TLS handshake, `None` for plain HTTP), passed into every controller

### HTTP version constants

`src/http/mod.rs` defines `VERSION` (HTTP/0.9 through HTTP/3.0) and `HTTP::version_list()` which is used by `Request::parse_method_and_request_uri_and_http_version_string` to validate the version token on every incoming request.

### Dynamic router

`src/router/mod.rs` provides `Router` — a fluent, path-based router with named parameters and wildcards. Register handlers with `.get()`, `.post()`, `.put()`, `.patch()`, `.delete()`, then call `router.handle(request, connection)` from inside `Application::execute`. Pattern syntax: `:name` for a named segment, `*name` for a trailing wildcard. `PathParams::get(name)` retrieves extracted values.

Call `.with_host("example.com")` before registering routes to restrict a `Router` to requests whose SNI hostname (TLS) or `Host` header (plain HTTP) matches that value — the foundation for virtual-host routing.

### Typed extractors

`src/extract/mod.rs` defines the `FromRequest` trait and built-in extractors:
- `Body` — raw bytes (never fails)
- `BodyText` — UTF-8 body (400 on invalid UTF-8)
- `Query` — parsed query string as `HashMap<String, String>`
- `RequestHeaders` — all request headers with case-insensitive `get(name)`

### Typed errors

`src/error/mod.rs` defines:
- `IntoResponse` trait — implement on your error enum to map it to a `Response`; `Response` itself is the identity implementation
- `AppError` enum — covers `BadRequest`, `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `UnprocessableEntity`, `TooManyRequests`, `Internal`; all implement `IntoResponse`

### Rate limiting

`src/rate_limit/mod.rs` provides `RateLimiter` — a thread-safe sliding-window limiter keyed by a string (typically the client IP). Call `limiter.check(key)` → `true` if within budget, `false` to return 429. `rate_limit::global()` returns a process-wide singleton configured via `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` (default 1000) and `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS` (default 60).

### Test client

`src/test_client/mod.rs` provides `TestClient<A>` — dispatches requests directly through an `Application` without opening a TCP socket. Use it in unit and integration tests:

```rust
let client = TestClient::new(App::new());
let res = client.get("/healthz").send();
assert_eq!(200, res.status());
```

### Graceful shutdown

HTTP/1.1 (`http1` feature, `ctrlc` dep): a `SIGINT`/`SIGTERM` handler sets an `AtomicBool`; the accept loop exits and the `ThreadPool` drains in-flight requests before the process exits.

HTTP/2 + HTTP/3 (async features): `tokio::signal` handles `SIGINT` and `SIGTERM`; each `run_tls`/`run_quic` loop selects on the signal and closes the endpoint cleanly.

### No async (default `http1` feature)

The server uses `std::net::TcpListener` and a hand-rolled `ThreadPool` (`src/thread_pool/mod.rs`). Each connection is handled synchronously on a worker thread.

### HTTP/2 feature (`--features http2`)

When built with the `http2` feature, the binary uses a `tokio` runtime and serves TLS via `rustls` (aws-lc-rs crypto backend). ALPN negotiation selects HTTP/2 (`h2` crate) or HTTP/1.1 per connection automatically. New modules:

- `src/tls/mod.rs` — `SniCertResolver` implements `rustls::server::ResolvesServerCert`; `create_tls_acceptor_from_vhosts(vhosts, default_cert, default_key)` builds a multi-domain `TlsAcceptor` that picks the right cert per SNI hostname at handshake time. `create_tls_acceptor(cert, key)` is a backward-compat wrapper for single-cert deployments. When `RWS_CONFIG_TLS_CLIENT_CA_FILE` is set, `load_client_verifier()` builds a `WebPkiClientVerifier` that enforces mTLS on both HTTPS and QUIC listeners.
- `src/virtual_host/mod.rs` — `VirtualHostConfig { domain, cert_file, key_file }` carries per-domain cert configuration.
- `src/h2_handler/mod.rs` — translates `h2::RecvStream` requests into `crate::request::Request`, calls `app.execute()`, translates `Response` back into H2 frames. The `Application` and `Controller` traits are untouched.
- `src/server/mod.rs::Server::run_tls()` — async accept loop; extracts SNI hostname after the TLS handshake (`tls_stream.get_ref().1.server_name()`), routes by ALPN to `h2_handler::handle_connection` or `Server::process_h1_tls`, populates `ConnectionInfo::sni_hostname`.
- HTTP/1.1 TLS responses include `Alt-Svc: h3=":PORT"` (or `h2` when http3 feature is absent) to advertise protocol availability.
- Forbidden HTTP/2 headers (`connection`, `keep-alive`, `transfer-encoding`, `upgrade`, `proxy-connection`, `te`) are stripped from responses before sending.
- SIGHUP reloads all virtual host certs alongside the default cert without restarting.

### HTTP/3 feature (`--features http3`, default)

When built with the `http3` feature (which implies `http2`), a second listener starts over UDP using QUIC (`quinn` crate). HTTP/3 uses `h3` + `h3-quinn`. New additions:

- `src/tls/mod.rs::create_quinn_server_config_from_vhosts()` — builds a multi-domain `quinn::ServerConfig` via the same `SniCertResolver`; `create_quinn_server_config()` is the single-cert wrapper.
- `src/h3_handler/mod.rs` — accepts QUIC connections, extracts SNI from `conn.handshake_data()?.downcast::<HandshakeData>()`, resolves H3 streams via `RequestResolver::resolve_request()`, calls `app.execute()`, sends H3 responses.
- `src/server/mod.rs::Server::run_quic()` — binds a UDP endpoint on the same port as TCP; skipped silently if no cert is configured.
- `main()` runs `Server::run_tls` and `Server::run_quic` concurrently via `tokio::join!`.

### Proxy modules

- `src/proxy/mod.rs` — `ReverseProxy` (HTTP/1.1 reverse proxy middleware); `H2ReverseProxy` (HTTP/2 upstream proxy, `http2` feature, uses `tokio::task::block_in_place` to bridge sync middleware into the tokio runtime); `GrpcProxy` (wraps `H2ReverseProxy`, filters on `Content-Type: application/grpc*`). `Backend::parse()` strips `h2://` and `http://` prefixes.
- `src/tcp_proxy/mod.rs` — `TcpProxy` standalone L4 TCP proxy; `bind(addr)` blocks and accepts connections; `relay(client)` spawns two threads doing `std::io::copy` for bidirectional byte relay; round-robin backend selection via `AtomicUsize`.
- `src/udp_proxy/mod.rs` — `UdpProxy` standalone UDP datagram proxy; per-datagram thread model; ephemeral socket per datagram connects to the backend; `set_read_timeout()` controls reply wait; round-robin backend selection.
- `src/ws_proxy/mod.rs` — `WsProxy` standalone WebSocket proxy; reads the HTTP upgrade request from the client, connects to the backend, exchanges upgrade handshake, then relays raw bytes bidirectionally in a two-thread loop.

### Rewrite middleware

- `src/rewrite/mod.rs` — `RewriteLayer` implements `Middleware`; clones `Request` and applies `RequestRule` variants (header set/remove, URI set/strip-prefix/add-prefix) before dispatch, then applies `ResponseRule` variants (header set/remove, status override, body byte find-and-replace) on the way back. Private `replace_bytes()` does linear non-overlapping scan.
