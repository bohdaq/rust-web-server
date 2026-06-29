# rws

Static file web server written in Rust. Serves HTTP/1.1 on a synchronous thread pool. Build with `--features http2` to enable TLS and HTTP/2.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.75 or later

## Build

HTTP/1.1 only:
```bash
cargo build --release
```

With HTTPS and HTTP/2 support:
```bash
cargo build --release --features http2
```

## Run

### HTTP/1.1

```bash
./target/release/rws
```

Server starts on `http://127.0.0.1:7878` by default. Place your files in the working directory and open the URL in a browser.

### HTTPS + HTTP/2

Generate a self-signed certificate for local development:
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

Start the server:
```bash
./target/release/rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```

Open `https://127.0.0.1:7878` in a browser. The server negotiates HTTP/2 or HTTP/1.1 automatically via ALPN on the same port.

### Custom port and address

```bash
./target/release/rws --ip=0.0.0.0 --port=443 --tls-cert-file=cert.pem --tls-key-file=key.pem
```

See [CONFIGURE](CONFIGURE.md) for all configuration options (env vars, config file, command-line flags).

## Features

- HTTP/2 with ALPN negotiation alongside HTTP/1.1 on the same port
- TLS via [rustls](https://github.com/rustls/rustls) (aws-lc-rs backend, no OpenSSL)
- CORS — allowed for all origins by default, fully configurable
- HTTP Range Requests — partial file serving and multi-range responses
- HTTP Client Hints
- `X-Content-Type-Options: nosniff` and `X-Frame-Options` headers
- Symlink resolution
- `.html` extension inference — `/page` serves `page.html`; `/dir` serves `dir/index.html`
- No caching headers — files are always served fresh
- Request/response logging to stdout

## Further reading

- [CONFIGURE](CONFIGURE.md) — all configuration options
- [FAQ](FAQ.md) — common problems and solutions
- [DEVELOPER](DEVELOPER.md) — building, testing, and contributing
- [src/README.md](src/README.md) — module-level documentation
