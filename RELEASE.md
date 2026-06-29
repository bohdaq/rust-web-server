[Read Me](README.md) > [Developer](DEVELOPER.md) > Release

# Release Info

Make sure you have [Rust](https://www.rust-lang.org/tools/install) 1.75 or later installed.

## Build

Default binary (HTTP/3 + HTTP/2 + TLS):
```bash
cargo build --release
./target/release/rws --ip=0.0.0.0 --port=443 --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```

HTTP/2 + TLS only (no QUIC):
```bash
cargo build --release --no-default-features --features http2
./target/release/rws --ip=0.0.0.0 --port=443 --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```

HTTP/1.1 only (no TLS, smallest binary):
```bash
cargo build --release --no-default-features --features http1
./target/release/rws --ip=0.0.0.0 --port=8080
```

## Publish to crates.io

```bash
cargo publish
```

## Supported architectures

For each binary provide a SHA-256 checksum.

1. x86 64-bit Apple: **x86_64-apple-darwin**
1. x86 64-bit Linux: **x86_64-unknown-linux-gnu** — Debian (.deb), RPM (.rpm), Portage ebuild, Pacman
1. ARM 64-bit Linux: **aarch64-unknown-linux-gnu** — Debian (.deb)
1. x86 64-bit Windows: **x86_64-pc-windows-msvc**

See [other supported platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).
