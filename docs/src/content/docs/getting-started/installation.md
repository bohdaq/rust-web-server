---
title: Installation
description: How to install rust-web-server as a library crate, as a standalone binary, or from source — including feature flags and MSRV.
---

## Requirements

**Minimum Supported Rust Version (MSRV): 1.75**

Install or update Rust via [rustup](https://rustup.rs/):

```bash
rustup update stable
rustc --version   # should print 1.75.0 or newer
```

## As a library crate

Add `rust-web-server` to your `Cargo.toml`:

```bash
cargo add rust-web-server
```

Or pin the major version manually:

```toml
[dependencies]
rust-web-server = "17"
```

The default feature set (`http3`) pulls in HTTP/3, HTTP/2, TLS, and a tokio async runtime. If you need a lighter build, opt out and select a lower feature tier (see the feature flags table below).

### Prelude

A single glob import covers the types you need in almost every handler:

```rust
use rust_web_server::prelude::*;
// Re-exports: App, Server, ConnectionInfo, Request, Response,
//             STATUS_CODE_REASON_PHRASE, Range, MimeType,
//             PathParams, New, routes!
```

## As a standalone binary

Install the `rws` binary to `~/.cargo/bin`:

```bash
cargo install rust-web-server
rws --version
```

Run it in any directory that contains static files and it will serve them on `http://localhost:7878` with no configuration file needed.

## Build from source

```bash
git clone https://github.com/bohdaq/rust-web-server.git
cd rust-web-server
cargo build --release
# binary is at: target/release/rws
```

To build a specific feature tier from source:

```bash
# HTTP/2 + TLS only (no QUIC)
cargo build --release --no-default-features --features http2

# HTTP/1.1 only — no TLS, no async runtime, smallest binary
cargo build --release --no-default-features --features http1
```

## Feature flags

| Feature | What it adds | Required deps |
|---|---|---|
| `http1` | Synchronous thread-pool server, no async runtime, no TLS | `ctrlc`, `libc` |
| `http2` | tokio runtime, TLS via rustls (aws-lc-rs), HTTP/2 via ALPN | `h2`, `rustls`, `tokio`, `tokio-rustls`, … |
| `http3` *(default)* | QUIC transport and HTTP/3 on top of `http2` | `quinn`, `h3`, `h3-quinn` |
| `http-client` | HTTPS support in the outbound `Client` | `rustls`, `webpki-roots` |
| `serde` | JSON serialization/deserialization via serde | `serde`, `serde_json` |
| `auth` | HMAC-SHA2 utilities for JWT signing and cookie signing | `hmac`, `sha2` |
| `macros` | `#[derive(Model)]` proc-macro for the ORM layer | `rws-macros` |
| `acme` | Automatic TLS certificate provisioning (ACME/Let's Encrypt) | `rcgen`, `aws-lc-rs`, … |
| `tera` | Tera template engine integration | `tera`, `serde`, `serde_json` |
| `model-sqlite` | ORM backend for SQLite (bundled libsqlite3, no system dep) | `rusqlite` |
| `model-postgres` | ORM backend for PostgreSQL | `postgres` |
| `model-mysql` | ORM backend for MySQL / MariaDB | `mysql` |
| `crypto` | Argon2id password hashing | `argon2`, `rand_core` |
| `csrf` | CSRF token generation and validation | `rand_core` |
| `sso` | OAuth 2.0 / OIDC SSO support (RSA + ECDSA signing, outbound HTTPS) | `rsa`, `p256`, `sha2`, `rand_core`, `serde`, `serde_json`, `http-client` |

:::note[Combining features]
Features compose freely. For example, to build HTTP/2 + TLS + JSON + SQLite:

```toml
rust-web-server = { version = "17", default-features = false, features = ["http2", "serde", "model-sqlite"] }
```
:::

## Approximate binary sizes

These measurements are for a release build (`cargo build --release`) of the `rws` binary with only the core transport feature active. Enabling additional features (`serde`, `tera`, ORM backends, etc.) increases the size.

| Feature tier | Approx. binary size |
|---|---|
| `http1` | ~3 MB |
| `http2` | ~8 MB |
| `http3` (default) | ~12 MB |

:::note[No OpenSSL]
TLS uses `rustls` with the `aws-lc-rs` crypto backend. There is no dependency on a system OpenSSL installation. Binaries are fully static with respect to TLS and are compatible with FIPS-validated environments.
:::

## No third-party HTTP dependencies

HTTP parsing, CORS, MIME types, range requests, WebSocket, SSE, and routing are all implemented from scratch inside this crate. The transitive dependency tree has no `hyper`, `actix-net`, or `tokio-util`.
