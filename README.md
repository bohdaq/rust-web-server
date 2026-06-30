# rws

Static file web server and HTTP toolkit written in Rust. Supports HTTP/3, HTTP/2, and HTTP/1.1. HTTP/3 and HTTP/2 require a TLS certificate; without one the server falls back to plain HTTP/1.1 automatically.

Use it as a ready-to-run binary **or** pull it in as a library crate to get battle-tested building blocks — request/response parsing, routing, headers, MIME detection, body parsing, JSON, logging — without taking on a full async framework.

## Install

```bash
cargo install rust-web-server
```

This installs the `rws` binary with HTTP/3, HTTP/2, and TLS support included.

## Run

### Plain HTTP/1.1

```bash
rws
```

Starts on `http://127.0.0.1:7878` by default. Place your files in the working directory and open the URL in a browser.

### HTTPS + HTTP/2 + HTTP/3

Generate a self-signed certificate for local development:
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

Start the server with the certificate:
```bash
rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```

Open `https://127.0.0.1:7878` in a browser. The server listens on the same port for both TCP (HTTP/1.1 and HTTP/2 via ALPN) and UDP (HTTP/3 via QUIC). HTTP/2 and HTTP/3 are negotiated automatically — no extra configuration needed.

For a public domain, obtain a certificate from [Let's Encrypt](https://letsencrypt.org/).

### Custom address and port

```bash
rws --ip=0.0.0.0 --port=443 --tls-cert-file=cert.pem --tls-key-file=key.pem
```

See [CONFIGURE](CONFIGURE.md) for all configuration options (env vars, config file, command-line flags).

## Build from source

```bash
git clone https://github.com/bohdaq/rust-web-server.git
cd rust-web-server
cargo build --release
```

The binary is at `target/release/rws`.

To build with HTTP/2 only (no QUIC/HTTP/3):
```bash
cargo build --release --no-default-features --features http2
```

To build HTTP/1.1 only (smallest binary, no TLS):
```bash
cargo build --release --no-default-features --features http1
```

## Features

- HTTP/3 over QUIC (UDP) — negotiated via `Alt-Svc`
- HTTP/2 with ALPN negotiation alongside HTTP/1.1 on the same TCP port
- TLS via [rustls](https://github.com/rustls/rustls) (aws-lc-rs backend, no OpenSSL)
- HTTP/1.1 keep-alive — persistent connections; `Connection: close` or idle timeout ends the session
- Response compression — automatic gzip for text types when client sends `Accept-Encoding: gzip`
- Large file streaming — chunked transfer for files > 8 MB; no full-file buffering
- HTTP → HTTPS redirect — set `RWS_CONFIG_HTTP_REDIRECT_PORT` to redirect a plain-HTTP port
- Cookie handling — `CookieJar` parses the `Cookie` header; `SetCookie` builder creates `Set-Cookie` values
- CORS — allowed for all origins by default, fully configurable
- HTTP Range Requests — partial file serving and multi-range responses
- HTTP Client Hints
- ETag and 304 Not Modified — conditional requests skip body transfer on cache hit
- Security headers — `Strict-Transport-Security` (HTTPS only), `Content-Security-Policy` (configurable via `RWS_CONFIG_CSP`), `Referrer-Policy`, `Permissions-Policy`, `X-Content-Type-Options`, `X-Frame-Options`
- WebAssembly MIME type — `.wasm` files served as `application/wasm`
- Combined Log Format (CLF) — access log compatible with GoAccess and AWStats; set `RWS_CONFIG_LOG_FORMAT=json` for structured JSON logs
- Graceful shutdown — Ctrl+C and SIGTERM stop the server cleanly (async/TLS paths); `/readyz` returns `503` during drain
- Kubernetes-ready — health probes (`GET /healthz` liveness, `GET /readyz` readiness), Prometheus metrics (`GET /metrics`), `0.0.0.0` default bind, Dockerfile included
- Dynamic routing — standalone `Router` with `:param` and `*wildcard` path matching
- Typed errors — `IntoResponse` trait and built-in `AppError` mapping to HTTP status codes
- Typed request extractors — `FromRequest` trait; built-in `Body`, `BodyText`, `Query`, `RequestHeaders`
- Per-IP rate limiting — sliding-window `RateLimiter`; configurable via env vars
- In-process test client — `TestClient` dispatches requests without a TCP socket
- WebSocket support — RFC 6455 handshake, frame encode/decode, SHA-1 + base64 built in, no extra dependency
- Shared application state — `App::with_state(S)` shares `Arc<S>` across state-aware route handlers
- Middleware pipeline — `App::new().wrap(layer)` stacks composable `Middleware` layers; built-in `RateLimitLayer` included
- Async handlers — `App::with_async_state(S)` gives route handlers an `async fn` signature (`http2` feature, tokio-backed)
- Server-Sent Events — `Sse` builder produces a buffered `text/event-stream` response with correct headers
- Graceful shutdown — Ctrl+C and SIGTERM drain in-flight connections on all server paths
- 30-second read timeout per request on plain HTTP/1.1 connections
- Symlink resolution
- `.html` extension inference — `/page` serves `page.html`; `/dir` serves `dir/index.html`
- Custom 404 page — place a `404.html` in the working directory to override the default

## Use as a library

Add the crate to `Cargo.toml`:

```toml
[dependencies]
rust-web-server = "17"
```

Implement a controller and plug it into the server in a few lines:

```rust
use rust_web_server::controller::Controller;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::ConnectionInfo;

pub struct PingController;

impl Controller for PingController {
    fn is_matching(request: &Request, _: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/ping"
    }

    fn process(_: &Request, mut response: Response, _: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(b"pong".to_vec(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    }
}
```

See [DEVELOPER](DEVELOPER.md) for the full building blocks reference and 21 use case examples covering JSON responses, query parameters, form and file upload parsing, redirects, typed errors, typed extractors, rate limiting, testing, WebSocket connections, shared state, and middleware.

## AI adoption

This framework is designed to be an AI first class citizen — AI coding assistants (Claude, Cursor, Copilot) generate correct, idiomatic, compiling code on the first try.

See [AI_ADOPTION.md](AI_ADOPTION.md) for the full strategy: using the server as an AI API backend, adding SSE streaming for token-by-token output, implementing an MCP tool server, and the steps to make the framework maximally discoverable by AI tools (`llms.txt`, Cargo examples, ergonomic helpers, system prompt file).

## Further reading

- [CONFIGURE](CONFIGURE.md) — all configuration options
- [FAQ](FAQ.md) — common problems and solutions
- [DEVELOPER](DEVELOPER.md) — building blocks, use cases, building, and testing
- [src/README.md](src/README.md) — module-level documentation
- [AI_ADOPTION.md](AI_ADOPTION.md) — AI adoption strategy and roadmap

## License

MIT
