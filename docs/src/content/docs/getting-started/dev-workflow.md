---
title: Auto-Rebuild on Change (cargo-watch)
description: Automatically rebuild and restart your rws app whenever source files change, so you're always running the latest code.
---

`rws` doesn't need any special support for this — it's the standard Rust dev-loop pattern, using an external file-watcher to rebuild and restart your binary. [`cargo-watch`](https://crates.io/crates/cargo-watch) is the simplest way to get it.

## Setup

```bash
cargo install cargo-watch
```

Run this from your app's project root — the one with a `Cargo.toml` that depends on `rust-web-server`, not inside the `rws` source tree itself:

```bash
cargo watch -x run
```

On every file save, `cargo-watch` kills the currently running process, rebuilds, and restarts it. You never run `cargo run` by hand again — the server you're hitting with `curl` (or a browser) is always built from the latest saved code.

## Useful variants

Fail fast on a compile error before it tries to restart the (now-stale) server:

```bash
cargo watch -x check -x run
```

Scope which paths are watched — useful if your project has directories generating unrelated file-system noise (build scripts, generated files):

```bash
cargo watch -w src -w Cargo.toml -x run
```

If you're developing against a local checkout of `rws` itself via a `path = "../rust-web-server"` dependency (rather than the crates.io release), watch both trees so library changes trigger a rebuild too:

```bash
cargo watch -w src -w ../rust-web-server/src -x run
```

## What to expect

- **Rebuild time** dominates the loop — typically a few seconds for an incremental build, longer the first time or after a `Cargo.toml` change.
- **The process fully restarts** on each change — it isn't a hot code swap. Any in-memory state (counters, caches you built yourself, in-memory `SessionStore`) is reset; state backed by `DbPool`, `RedisSessionStore`, or `DbSessionStore` survives across restarts since it lives outside the process.
- **In-flight requests during a restart** get connection-refused for the brief window between the old process exiting and the new one binding the port again. Fine for iterative development; if this is disruptive enough to matter (e.g. a frontend dev server proxying to your API while you're actively testing), see the note below.

:::tip[Want zero-downtime restarts instead?]
`Server::run()` and `Server::run_tls()` both accept an already-constructed `std::net::TcpListener` rather than always binding their own — which means the [`systemfd`](https://crates.io/crates/systemfd) + [`listenfd`](https://crates.io/crates/listenfd) pattern works with `rws` as-is: `systemfd` holds the listening socket open across restarts and hands it to each new process via `listenfd`, so there's no "address already in use" and no gap where the port stops accepting connections. This is a bigger setup than plain `cargo-watch` and only worth it if the restart gap is actually causing friction.
:::
