---
title: Installation
description: How to install rust-web-server as a library crate, as a standalone binary, or from source — including feature flags and MSRV.
---

## Requirements

**Minimum Supported Rust Version (MSRV): 1.88**

Install or update Rust via [rustup](https://rustup.rs/):

```bash
rustup update stable
rustc --version   # should print 1.88.0 or newer
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

Every release ships prebuilt binaries, so no Rust toolchain is needed:

| Method | Command | Platforms |
|---|---|---|
| Homebrew | `brew install bohdaq/tap/rws` | macOS (Apple Silicon, Intel), Linux (x86_64, arm64) |
| cargo-binstall | `cargo binstall rust-web-server` | everything below, without compiling |
| Docker | `docker run -p 7878:7878 -v "$PWD:/srv" bohdaq/rws` (also `ghcr.io/bohdaq/rws`) | linux/amd64, linux/arm64 |
| Download | [GitHub Releases](https://github.com/bohdaq/rust-web-server/releases/latest) | `x86_64`/`aarch64-unknown-linux-musl`, `x86_64`/`aarch64-apple-darwin`, `x86_64-pc-windows-msvc` |
| From source | `cargo install rust-web-server` | anywhere Rust 1.88+ runs |

Linux binaries are statically linked against musl, so they run on any distribution regardless of glibc version. Each archive has a matching `.sha256` file.

```bash
rws --version
rws --help      # every option, with its environment-variable equivalent
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
| `auth` | `BasicAuthLayer`, `JwtLayer` (HS256), `ForwardAuthLayer`, JWT helpers | `hmac`, `sha2` |
| `auth-asymmetric` | RS256 / ES256 in `JwtLayer` (in-crate RSA verification) | `auth`, `p256`, `rand_core` |
| `macros` | `#[derive(Model)]`, `#[derive(FromRequest)]`, `#[derive(Validate)]`, `#[derive(Config)]`, `#[get]`/`#[post]`/… route attributes | `rws-macros` |
| `acme` | Automatic TLS certificate provisioning (ACME/Let's Encrypt) | `http2`, `rcgen`, `aws-lc-rs`, … |
| `tera` | Tera template engine integration | `tera`, `serde`, `serde_json` |
| `model-sqlite` | Async ORM backend for SQLite | `http2`, `sqlx` (sqlite) |
| `model-postgres` | Async ORM backend for PostgreSQL | `http2`, `sqlx` (postgres) |
| `model-mysql` | Async ORM backend for MySQL / MariaDB | `http2`, `sqlx` (mysql) |
| `crypto` | Argon2id password hashing, signed (HMAC) and encrypted (AES-GCM) cookies | `argon2`, `rand_core`, `hmac`, `sha2`, `aes-gcm` |
| `csrf` | CSRF token generation and validation | `rand_core` |
| `mailer` | SMTP client (STARTTLS/SMTPS also need `http-client` or `http2`) | — |
| `sso` | OAuth 2.0 / OIDC client (RS256/ES256 `id_token` verification, outbound HTTPS) | `p256`, `sha2`, `rand_core`, `serde`, `serde_json`, `http-client` |
| `sso-server` | `AuthServer`: rws as an OAuth 2.0 authorization server (HS256 tokens) | `sso`, `auth` |
| `sso-saml` | `SamlSp`: SAML 2.0 Service Provider | `sso` |
| `jobs` | In-memory `JobQueue` (`PersistentJobQueue` also needs a `model-*` feature) | — |
| `storage-local` | `LocalStorage` | — |
| `storage-s3` | `S3Storage` (AWS SigV4, no SDK) | `hmac`, `sha2`, `http-client` |
| `storage-azure` | `AzureBlobStorage` (Shared Key / Managed Identity, no SDK) | `hmac`, `sha2`, `http-client` |
| `openapi` | `GET /openapi.json` + `GET /docs` from registered routes | — |
| `webhook` | `verify_webhook_signature` (GitHub, Shopify, Stripe) | `hmac`, `sha2` |
| `rewrite-regex` | `RewriteLayer::request_uri_regex_rewrite` | `regex` |
| `secrets` | `secrets::resolve()` for `vault://`, `aws-sm://`, `azkv://` references | `hmac`, `sha2`, `http-client` |

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

## What's built in, and what isn't

HTTP/1.1 parsing, CORS, MIME types, range requests, WebSocket, SSE, and routing are implemented inside this crate. The transitive dependency tree has no `hyper`, `tower`, or `actix-net`.

The rest of the protocol stack uses established crates rather than reimplementing them: HTTP/2 framing uses `h2`, HTTP/3 and QUIC use `h3` and `quinn`, and TLS uses `rustls`. `tokio-util` appears transitively in the default build through `h2`.
