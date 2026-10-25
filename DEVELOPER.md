[Read Me](README.md) > Developer

# Developer Guide

How to build, test and release `rws`, and where documentation goes. For using
the crate (routing, middleware, proxy, database, deployment, ...) read the
user documentation:

- Site: **https://rws8.tech** (rebuilt on each release)
- In the repo: [`docs/src/content/docs/`](docs/src/content/docs/) (Starlight
  markdown, browsable on GitHub)
- API: rustdoc on [docs.rs/rust-web-server](https://docs.rs/rust-web-server);
  the type index is [API Reference](docs/src/content/docs/reference/api.md)

See also [CONTRIBUTING.md](CONTRIBUTING.md) (ground rules for a PR) and
[CLAUDE.md](CLAUDE.md) (architecture in depth).

You need [Rust](https://www.rust-lang.org/tools/install) 1.88 or later (the
MSRV; CI checks it in the `msrv` job).

```bash
rustup update
```

## Run

```bash
cargo run
```

Starts with HTTP/3, HTTP/2 and TLS compiled in (the default). Without a
certificate it falls back to plain HTTP/1.1.

```bash
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem   # HTTPS + HTTP/2 + HTTP/3
cargo run --no-default-features --features http1                # HTTP/1.1 only, no TLS
cargo run -- --check                                            # validate ./rws.config.toml
```

## Test

```bash
cargo test                                                    # everything (about 30 s)
cargo test --lib proxy::                                      # one module
cargo test --lib client_hint::tests::hint_list -- --exact     # one test
```

Feature-gated code is tested with that feature on, for example
`cargo test --no-default-features --features http2,macros,model-sqlite -- model`.

Rules for tests:

- Put tests in the module's `tests.rs` (or a `#[cfg(test)]` block): the happy
  path plus at least one failure case. `TestClient::new(app)` runs requests
  through an app without a socket.
- **Shared env lock.** Tests run in parallel in one process and `RWS_CONFIG_*`
  variables are process-wide. A test that writes them, directly or through
  `bootstrap()`, `override_environment_variables_from_config()`,
  `config_reload::reload()`, `CommandLineArgument::_parse()` /
  `set_environment_variable()`, or that stores `metrics::SERVER_READY`, must
  start with `let _g = crate::test_env::lock();` (a named binding, held for
  the whole test). A test that sets a variable directly also calls
  `server_config::invalidate()`. CORS/CSP header tests can skip the lock by
  using `App::with_config(ServerConfig { .. })`. Details: CLAUDE.md,
  "Shared-state lock".
- Tests that write files use `crate::test_env::TempPath::new(name)`, which
  deletes the file on drop.
- A new parser of untrusted input gets a case in `src/stack_depth_tests.rs`
  and a fuzz target in `fuzz/`.

### End-to-end and fuzzing

CI runs these on every push. Run them after touching `acme`, `proxy`,
`server`, `h2_handler`, `config_reload` or response parsing.

| Check | Command |
|---|---|
| ACME against Pebble (build with `--features "acme auth"`) | `ci/e2e/acme_pebble.sh target/debug/rws <pebble-binary> <pebble-source-dir>` |
| HTTP/2 conformance, must be 0 failed / 0 skipped | `ci/e2e/h2spec.sh target/debug/rws <h2spec-binary>` |
| 1000 idle clients (Slowloris) | `ci/e2e/slow_clients.sh target/debug/rws` |
| `--check` exit codes and SIGHUP reload | `ci/e2e/config_reload.sh target/debug/rws` |
| Fuzz every untrusted-input parser (nightly + cargo-fuzz) | `fuzz/run.sh 60 4` (see [fuzz/README.md](fuzz/README.md)) |

Pebble: `gh release download v2.10.1 --repo letsencrypt/pebble` plus
`gh api repos/letsencrypt/pebble/tarball/v2.10.1` (its `test/` config and
certs). h2spec: `gh release download v2.6.0 --repo summerwind/h2spec`. Each
script's header describes what it checks.

`bench/run.sh` compares throughput against Caddy and nginx (also the manual
`Benchmark` workflow). Judge a performance change with `RWS_BASE=<binary>`
(workflow input `base_ref`): both builds in one run, on the same machine.

## Build

```bash
cargo build --release                                              # default: HTTP/3 + HTTP/2 + TLS
cargo build --release --no-default-features --features http2       # HTTP/2 + TLS, no QUIC
cargo build --release --no-default-features --features http1       # HTTP/1.1 only, smallest
cargo build --release --features "acme auth"                       # what the prebuilt binaries use
```

### Feature matrix

| Feature | Enables | Notes |
|---|---|---|
| `http3` (default) | HTTP/3 over QUIC | implies `http2` |
| `http2` | tokio runtime, TLS (rustls), HTTP/2, async handlers, `AsyncClient` | |
| `http1` | synchronous thread-pool server, no async runtime | use with `--no-default-features` |
| `http-client` | HTTPS for the outbound `Client` | plain HTTP needs no feature; `http2` also enables HTTPS |
| `acme` | automatic certificates (Let's Encrypt) | implies `http2` |
| `serde` | `Json<T>` extractor/response, `Response::json::<T>()` on the client | |
| `auth` | `BasicAuthLayer`, `JwtLayer` (HS256), `ForwardAuthLayer` | |
| `auth-asymmetric` | RS256 / ES256 in `JwtLayer` | implies `auth` |
| `macros` | `#[derive(...)]` macros from `rws-macros` | |
| `tera` | `TeraEngine` templates | |
| `model-sqlite` / `model-postgres` / `model-mysql` | `DbPool`, `#[derive(Model)]`, migrations | each implies `http2`; combinable |
| `crypto` | password hashing, tokens, encrypted cookies | |
| `csrf` | `CsrfLayer` | |
| `mailer` | SMTP `Mailer` | STARTTLS/SMTPS also need `http-client` or `http2` |
| `sso` | OAuth2 / OIDC client (`OidcAuth`) | implies `http-client` |
| `sso-server` | `AuthServer`: rws as an OAuth 2.0 authorization server | implies `sso`, `auth` |
| `sso-saml` | `SamlSp`: SAML 2.0 service provider | implies `sso` |
| `jobs` | in-memory `JobQueue` | `PersistentJobQueue` also needs a `model-*` feature |
| `storage-local` / `storage-s3` / `storage-azure` | file / object storage backends | S3 and Azure imply `http-client` |
| `openapi` | `/openapi.json` and `/docs` | |
| `webhook` | GitHub / Shopify / Stripe signature checks | |
| `rewrite-regex` | regex URI rewriting in `RewriteLayer` | |
| `secrets` | `vault://`, `aws-sm://`, `azkv://` references | implies `http-client` |

`Cargo.toml` `[features]` is the source of truth; the user-facing list is
[Feature flags](docs/src/content/docs/getting-started/features.md).

### WASM guest

`rws-wasm-shim` builds a `wasi:http/proxy` component for Wasmtime, Spin or
Fastly Compute (see [WASM](docs/src/content/docs/deployment/wasm.md)):

```bash
rustup target add wasm32-wasip2
cd rws-wasm-shim
cargo build --release --target wasm32-wasip2
wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm
```

## Release

See [RELEASE.md](RELEASE.md). The release workflow (`.github/workflows/release.yml`)
builds the binaries and the container image (`Dockerfile.release`, distroless:
startup code must not shell out or assume a writable working directory) and
smoke-tests the image before pushing it.

## Project layout

| Path | Contents |
|---|---|
| `src/` | the crate; one directory per module, module docs in `//!` at the top of `mod.rs` |
| `src/main.rs` | the `rws` binary |
| `src/app/` | `App` and the built-in controllers |
| `src/server/` | listeners, HTTP/1.1 framing, TLS accept loops |
| `src/h2_handler/`, `src/h3_handler/` | HTTP/2 and HTTP/3 |
| `src/proxy/`, `src/proxy_config/` | reverse proxy and the config-driven proxy |
| `src/entry_point/` | configuration loading (CLI, env, `rws.config.toml`) |
| `rws-macros/` | proc macros (`macros` feature) |
| `rws-wasm-shim/` | WASM guest component |
| `docs/` | Astro/Starlight documentation site (rws8.tech) |
| `ci/e2e/` | end-to-end scripts |
| `fuzz/` | cargo-fuzz targets |
| `bench/` | benchmark harness |
| `helm/rws/` | Helm chart |
| `spec/` | design notes and the roadmap |

## Where docs go

Each fact lives in one place:

| What | Where |
|---|---|
| How to use a feature: guides, examples, configuration, deployment | a page under [`docs/src/content/docs/`](docs/src/content/docs/), shown on https://rws8.tech; new pages also go in the sidebar in `docs/astro.config.mjs` |
| Public API (types, methods, parameters) | rustdoc: `///` on items, `//!` for module overviews; add new public types to [`reference/api.md`](docs/src/content/docs/reference/api.md) |
| What changed in a release | [CHANGELOG.md](CHANGELOG.md) |
| Architecture and internals | [CLAUDE.md](CLAUDE.md) and the [Architecture](docs/src/content/docs/reference/architecture.md) page |

Docs site links are root-relative (`/features/sessions/`). Keep examples
compiling against the current API; check names and feature gates in `src/`.
