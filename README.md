# rws

Static file web server written in Rust. Supports HTTP/3, HTTP/2, and HTTP/1.1. HTTP/3 and HTTP/2 require a TLS certificate; without one the server falls back to plain HTTP/1.1 automatically.

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
- CORS — allowed for all origins by default, fully configurable
- HTTP Range Requests — partial file serving and multi-range responses
- HTTP Client Hints
- ETag and 304 Not Modified — conditional requests skip body transfer on cache hit
- Security headers — `Strict-Transport-Security` (HTTPS only), `Content-Security-Policy` (configurable via `RWS_CONFIG_CSP`), `Referrer-Policy`, `Permissions-Policy`, `X-Content-Type-Options`, `X-Frame-Options`
- WebAssembly MIME type — `.wasm` files served as `application/wasm`
- Combined Log Format (CLF) — access log compatible with GoAccess and AWStats
- Graceful shutdown — Ctrl+C stops the server cleanly (async/TLS paths)
- 30-second read timeout on plain HTTP/1.1 connections
- Symlink resolution
- `.html` extension inference — `/page` serves `page.html`; `/dir` serves `dir/index.html`
- Custom 404 page — place a `404.html` in the working directory to override the default

## Further reading

- [CONFIGURE](CONFIGURE.md) — all configuration options
- [FAQ](FAQ.md) — common problems and solutions
- [DEVELOPER](DEVELOPER.md) — building, testing, and contributing
- [src/README.md](src/README.md) — module-level documentation

## License

MIT
