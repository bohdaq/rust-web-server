[Read Me](README.md) > [Developer](DEVELOPER.md) > Release

# Release Info

Make sure you have [Rust](https://www.rust-lang.org/tools/install) 1.88 or later installed.

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

The release workflow publishes both crates when the tag is pushed: `rws-macros`
first (only if its `rws-macros/Cargo.toml` version isn't on crates.io yet —
bump it whenever the macros change), then `rust-web-server`. It needs the
`CARGO_REGISTRY_TOKEN` repository secret (a crates.io token with publish rights
for both crates); a version already on crates.io is skipped, so re-runs are safe.

## Publish a GitHub Release

Tag the commit with the exact `Cargo.toml` version (no `v` prefix) and push the tag:

```bash
git tag 17.110.0
git push origin 17.110.0
```

`.github/workflows/release.yml` checks that the tag matches `Cargo.toml`, then:

1. publishes a GitHub Release whose notes list every `feat`/`fix`/`perf` commit since the previous tag, with commits marked breaking (`type!:` / `type(scope)!:`) repeated in a "Breaking changes" section at the top;
2. builds `rws` for the targets below and attaches `rws-<version>-<target>.tar.gz` (`.zip` on Windows) plus a `.sha256` for each — the names `[package.metadata.binstall]` in `Cargo.toml` points `cargo binstall` at;
3. smoke-tests the image, then pushes `<version>`, `<major>` and `latest` (linux/amd64 + linux/arm64) to `ghcr.io/bohdaq/rws` and, when the `DOCKERHUB_USERNAME` variable and `DOCKERHUB_TOKEN` secret are set, to Docker Hub `bohdaq/rws` (its overview is synced from `.github/DOCKERHUB.md` by `dockerhub-overview.yml` whenever that file changes), built from those binaries via `Dockerfile.release`;
4. rewrites `Formula/rws.rb` in [bohdaq/homebrew-tap](https://github.com/bohdaq/homebrew-tap), authenticated with the `HOMEBREW_TAP_DEPLOY_KEY` repository secret (a write-enabled deploy key on the tap repo).

A patch for an older line (e.g. `17.113.1` after `18.0.0`) is released the same way, from a tag on that line's branch: the workflow sees that it isn't the highest version tag, so the GitHub Release isn't marked Latest, the image is pushed as `<version>` and `<major>` but not `latest`, and the Homebrew formula is left on the newest version.

To rebuild an existing tag after fixing the workflow, run it manually (Actions → Release → Run workflow) with that tag; every step is idempotent.

## Supported architectures

Built automatically by the release workflow, each with a SHA-256 checksum:

1. ARM 64-bit Apple: **aarch64-apple-darwin**
1. x86 64-bit Apple: **x86_64-apple-darwin**
1. x86 64-bit Linux: **x86_64-unknown-linux-musl** (static, any distro)
1. ARM 64-bit Linux: **aarch64-unknown-linux-musl** (static, any distro)
1. x86 64-bit Windows: **x86_64-pc-windows-msvc**

See [other supported platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).
