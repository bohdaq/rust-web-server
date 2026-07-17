---
title: WASM Guest Component
description: Run rust-web-server's App/Router/middleware inside a WASM runtime (Wasmtime, Spin, Fastly Compute) via the wasi:http/proxy world.
---

`rws-wasm-shim` is a separate workspace package that runs the same `App`/`Router`/middleware logic you use natively — but hosted inside a WASM component instead of a `TcpListener`. The host (Wasmtime, Spin, Fastly Compute) owns the listening socket and TLS termination; it calls the guest once per request through the standard [`wasi:http/proxy`](https://github.com/WebAssembly/wasi-http) world.

This is the "Foundation + Phase 1" slice of the project tracked in [`spec/WASM_SHIM.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/WASM_SHIM.md) in the repository — read that file for the full multi-phase plan, including what's deliberately out of scope.

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

Only the parts of `rust-web-server` that don't touch a socket or spawn a thread are compiled for `wasm32-wasip2` — everything else is behind `#[cfg(not(target_arch = "wasm32"))]`:

| Available in the guest | Native-only (excluded for `wasm32`) |
|---|---|
| `App`, `Router`, `Controller`, extractors | `Server`, `ThreadPool` |
| In-memory middleware (CORS, CSRF, rewrite, IP filter, request ID, in-memory rate limit/cache) | `ReverseProxy`, `H2ReverseProxy`, `GrpcProxy`, `TcpProxy`, `UdpProxy`, `WsProxy` |
| `Response::json`/`::text`, static-file serving | `WebSocket`, `http_client` (native TCP/TLS backend) |
| | `otel`, `log`, `Scheduler`, `redis_protocol`, `circuit_breaker`, `canary`, `service_discovery`, `ingress`, `timeout` |

## Two real limitations

These are properties of the `wasi:http/proxy` guest model itself, not bugs in the adapter:

1. **No real client IP or TLS SNI hostname.** The host already accepted the TCP connection and terminated TLS before ever invoking the guest, and the `wasi:http` incoming-request type has no field for either. `ConnectionInfo` gets a placeholder address and `sni_hostname: None`.
2. **Stateful middleware may not persist across requests.** Many WASI-HTTP hosts (Fastly Compute, Spin) instantiate a fresh guest **per request** for isolation. Anything relying on process-wide state — `RateLimiter`'s sliding window, `CacheLayer`, in-memory sessions — silently stops working unless your specific host guarantees instance reuse (`wasmtime serve` can pool instances; that isn't a portable guarantee across hosts). Verify this against your actual deployment target before relying on it.

## Dependency note

The static-file/directory-listing controllers that ship with `App` depend on the [`file-ext`](https://crates.io/crates/file-ext) crate for path handling. `file-ext` 12.1.0 added `wasm32` support (path separator, temp folder, and a `remove_dir_all`-based recursive delete in place of shelling out to `rm`, which has no equivalent in a WASI guest) — an older `file-ext` version will fail to compile for this target.

## What's not wired up yet

- Large/streaming response bodies (`Response::stream_file`/`stream_pipe`) — buffered only for now.
- Outbound HTTP from inside the guest (`http_client`, and therefore `sso`, `secrets`, `storage-s3`/`storage-azure`) needs a `wasi:http` `outgoing-handler` backend that doesn't exist yet.

Both are tracked as Phase 2 items in `spec/WASM_SHIM.md`.
