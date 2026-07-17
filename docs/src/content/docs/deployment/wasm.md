---
title: WASM Guest Component
description: Run rust-web-server's App/Router/middleware inside a WASM runtime (Wasmtime, Spin, Fastly Compute) via the wasi:http/proxy world.
---

`rws-wasm-shim` is a separate workspace package that runs the same `App`/`Router`/middleware logic you use natively — but hosted inside a WASM component instead of a `TcpListener`. The host (Wasmtime, Spin, Fastly Compute) owns the listening socket and TLS termination; it calls the guest once per request through the standard [`wasi:http/proxy`](https://github.com/WebAssembly/wasi-http) world.

This is the "Foundation + Phase 1 + most of Phase 2" slice of the project tracked in [`spec/WASM_SHIM.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/WASM_SHIM.md) in the repository — read that file for the full multi-phase plan and every verification detail. Everything on this page was tested against a real `wasmtime serve` process, not just compiled.

## Why not just compile the whole server to WASM?

`rws`'s normal accept loop uses `std::thread` (`ThreadPool`) and `rustls`/`aws-lc-rs` for TLS — neither compiles for `wasm32-wasip2`. Rather than trying to port sockets and TLS into the sandbox, the guest component reuses the existing seam every controller and middleware layer already goes through:

```rust
pub trait Application {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String>;
}
```

`Request` and `Response` are plain data structs with no socket handle anywhere in them, so the same `App::execute()` call that runs natively behind a `TcpListener` also runs unchanged behind a `wasi:http` incoming request.

## Building and running

```bash
rustup target add wasm32-wasip2   # Rust 1.82+ — higher than this crate's own 1.75 MSRV
cd rws-wasm-shim
cargo build --release --target wasm32-wasip2
wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm
```

In another shell:

```bash
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/
```

The build output (`rws_wasm_shim.wasm`) is already a valid WASM component on this target — no separate `wasm-tools component new` step is needed.

## What runs inside the guest

Only the parts of `rust-web-server` that don't touch a socket or spawn a thread are compiled for `wasm32-wasip2` — everything else is behind `#[cfg(not(target_arch = "wasm32"))]`. This list grew substantially past the initial "core only" slice — outbound HTTP now works, which unblocks everything built on top of it:

| Available in the guest | Native-only (excluded for `wasm32`) |
|---|---|
| `App`, `Router`, `Controller`, extractors | `Server`, `ThreadPool` |
| In-memory middleware (CORS, CSRF, rewrite, IP filter, request ID, in-memory rate limit/cache) | `ReverseProxy`, `H2ReverseProxy`, `GrpcProxy`, `TcpProxy`, `UdpProxy`, `WsProxy` |
| `Response::json`/`::text`, static-file serving (streamed for large files too) | `WebSocket`, `mailer` (raw SMTP — no `wasi:http` equivalent for arbitrary TCP protocols) |
| `http_client::Client` — **both HTTP and HTTPS**, via a `wasi:http` `outgoing-handler` backend (TLS terminated by the host, not `rustls`) | `otel`, `log`, `Scheduler` (background jobs), `redis_protocol`, `circuit_breaker`, `canary`, `service_discovery`'s DNS/etcd/Consul/Docker sources, `ingress`, `timeout` |
| `auth` (`BasicAuthLayer`, `JwtLayer`, `ForwardAuthLayer`), `auth-asymmetric`, `csrf`, `crypto`, `webhook`, `rewrite-regex` | `model` (async ORM — needs real DB wire protocols) |
| `sso`, `sso-server`, `sso-saml`, `secrets`, `storage-s3`, `storage-azure` — all work because they only ever used `http_client::Client`, no code changes needed | |

## Two real limitations (confirmed, not theoretical)

Both of these were empirically verified against a real `wasmtime serve` process, not inferred from the spec:

1. **Stateful middleware does not survive across requests.** `wasmtime serve` instantiates a **fresh guest per request** — confirmed by making an adapter call a request counter five times per invocation and watching it read back exactly `5` on every single request, never `10` or `15`. `RateLimiter`, `CacheLayer`, in-memory sessions, and metrics counters all silently reset every request under this host. Other hosts (Fastly Compute, Spin) are widely understood to behave the same way for the same isolation reasons, though only `wasmtime serve` was directly tested here. There is no in-crate fix for this — it needs either a host-provided KV binding (the `wasi-keyvalue` proposal, still pre-1.0) or accepting that stateful middleware requires a specific instance-reuse-guaranteeing host.
2. **Filesystem access needs an explicit host grant.** Unlike outbound HTTP (part of the `wasi:http/proxy` world's base contract), filesystem access is opt-in — `wasmtime serve` requires `--dir HOST_DIR::.` to expose any directory to the guest. Without it, static-file controllers silently fall back to `rws`'s built-in embedded default pages for every path, with no error — easy to miss in testing. With `--dir` granted, real files (including a 9 MB file exercised through the streaming code path) serve correctly.
3. **No real client IP or TLS SNI hostname.** The host already accepted the TCP connection and terminated TLS before ever invoking the guest, and the `wasi:http` incoming-request type has no field for either. `ConnectionInfo` gets a placeholder address and `sni_hostname: None`.

## Dependency notes

- The static-file/directory-listing controllers that ship with `App` depend on the [`file-ext`](https://crates.io/crates/file-ext) crate for path handling. `file-ext` 12.1.0 added `wasm32` support (path separator, temp folder, and a `remove_dir_all`-based recursive delete in place of shelling out to `rm`, which has no equivalent in a WASI guest) — an older `file-ext` version will fail to compile for this target.
- `rustls`'s own `aws-lc-rs` crypto-backend feature compiles C/asm that doesn't build for `wasm32-wasip2`. The root `Cargo.toml` moves `rustls`/`webpki-roots` into a `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table so enabling `http-client` (or `sso`/`secrets`/`storage-s3`/`storage-azure`, which depend on it) on `wasm32` never pulls that broken dependency in — the wasm32 build gets its TLS from the host instead, via the `outgoing-handler` interface.

## What's still open

- An automated CI job that builds the component and drives a `wasmtime serve` smoke test in scripted form (the manual verification above, scripted).
- Verification against hosts other than `wasmtime serve` (Spin, Fastly Compute) — architecturally expected to behave the same way, but untested.
- A documented, supported answer for stateful middleware under per-request instantiation (a host-provided KV binding, most likely) — currently just a documented limitation, not a workaround.

See `spec/WASM_SHIM.md` in the repository for the complete, itemized history of what was built and verified.
