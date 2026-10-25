# Contributing to rws

Thanks for helping. Bug reports, docs fixes and code are all welcome. Issues
labelled [`good first issue`](https://github.com/bohdaq/rust-web-server/labels/good%20first%20issue)
are small and self-contained, and say which files to look at. For questions and
ideas, use [Discussions](https://github.com/bohdaq/rust-web-server/discussions).

Security problems: please don't open a public issue — see [SECURITY.md](SECURITY.md).

## Build and test

You need Rust 1.88 or newer.

```bash
cargo build                                            # default: HTTP/1.1 + HTTP/2 + HTTP/3 + TLS
cargo build --no-default-features --features http1     # HTTP/1.1 only, no async runtime
cargo test                                             # all tests (about 30 s)
cargo test --lib proxy::                               # one module's tests
cargo run -- --check                                   # validate ./rws.config.toml
```

The prebuilt binaries are built with `--features "acme auth"`.

CI also runs, on every push:

| Check | Run it locally |
|---|---|
| Fuzzing of every parser that sees untrusted input | `fuzz/run.sh 30` (needs nightly and `cargo-fuzz`) |
| HTTP/2 conformance (h2spec) | `ci/e2e/h2spec.sh target/debug/rws <h2spec>` |
| ACME against Pebble | `ci/e2e/acme_pebble.sh …` (see the script header) |
| 1000 idle clients | `ci/e2e/slow_clients.sh target/debug/rws` |
| `--check` and SIGHUP reload | `ci/e2e/config_reload.sh target/debug/rws` |

## Ground rules for a change

- **Tests.** Put them in the module's `tests.rs` (or a `#[cfg(test)]` block):
  the happy path plus at least one failure case. `TestClient::new(app)` runs
  requests through an app without a socket.
- **Environment variables are shared between tests.** Tests run in parallel in
  one process. A test that sets `RWS_CONFIG_*` variables — directly or through
  `bootstrap()`, `override_environment_variables_from_config()` or
  `config_reload::reload()` — must start with
  `let _g = crate::test_env::lock();`. Tests of CORS/CSP headers can avoid
  that with `App::with_config(ServerConfig { … })`.
- **Docs ship with the code.** A user-visible change updates the matching
  page under `docs/src/content/docs/` (a new public type also gets a row in
  `reference/api.md` and rustdoc), adds a `CHANGELOG.md` entry, and
  regenerates `llms.txt` with `python3 ci/gen_llms.py`; `python3
  ci/check_docs.py` checks links and settings. `CLAUDE.md` describes the
  architecture in more depth than this file.
- **No new HTTP dependencies.** HTTP/1.1 parsing, routing, JSON, WebSocket and
  the ACME client are written in-crate on purpose; HTTP/2 and HTTP/3 use `h2`
  and `quinn`, TLS uses `rustls`. Ask in an issue before adding a dependency.
- **Keep the MSRV.** Don't use language or library features newer than Rust 1.88.

## Pull requests

1. Fork, branch from `main`, make the change with its tests and docs.
2. `cargo test` passes, and `cargo build` has no new warnings.
3. Open the PR describing what changed and why. Link the issue if there is one.

Small, focused PRs get reviewed fastest. If you're planning something large,
open an issue or a discussion first so we can agree on the approach.
