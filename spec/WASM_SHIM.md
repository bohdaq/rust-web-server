[Read Me](README.md) > WASM Shim Roadmap

# WASM / `wasm32-wasip2` Shim Roadmap

What is needed to run `rws` *inside* a WebAssembly runtime (Wasmtime, Spin, Fastly Compute, WasmEdge) as
opposed to embedding a WASM plugin *inside* `rws` (that's the separate "Wasm middleware" idea in
`IDEAS.md`, unrelated to this doc). Closes `GAPS_V3.md` §3.19 / `TODO.md`'s WebAssembly item /
`TODO_FINAL.md` #28. Rated "Very large / biggest unknown" there — this doc exists to turn that
one line into a sequenced, scoped project the same way `KAFKA_ROADMAP.md` did for Kafka.

**Status: Foundation and Phase 1 are shipped** (`rws-wasm-shim/`), verified end-to-end against a
real `wasmtime serve` process — not just a compile check. See "What actually shipped" below for
where the implementation diverged from the original sketch. Phases 2 and 3 remain open.

---

## Architecture decision — guest, not standalone server

The naive reading of "port rws to `wasm32-wasi`" — compile the existing `TcpListener` accept loop,
`ThreadPool`, and TLS stack to WASM and run it as a sandboxed standalone server — **does not work**:

- `thread_pool` needs `std::thread::spawn`, unavailable on `wasm32-wasip2` (no threads proposal
  in scope; that's a different target, `wasm32-wasip1-threads`, with its own maturity gap).
- `rustls` + `aws-lc-rs` (the TLS/crypto backend `http2`/`http3`/`acme` depend on) does not build
  for `wasm32-*` — `aws-lc-rs` links pre-built C/asm artifacts per host triple.
- `quinn`/`h3`/`h2` assume real UDP/TCP sockets and a multi-threaded tokio runtime
  (`rt-multi-thread`), neither available in a WASI guest.

The realistic and actually-useful shape is the one every real WASM-HTTP platform already uses:
**rws runs as a `wasi:http/proxy` guest component.** The host (`wasmtime serve`, Spin, Fastly
Compute, a cloud edge platform) owns the listening socket and TLS termination; per request it
calls the guest's `incoming-handler.handle(request, response-out)` export once. rws's job shrinks
to: parse nothing off the wire (the host already parsed HTTP), run routing/middleware/handler
logic, hand back a response.

This maps cleanly onto an existing seam in the codebase — confirmed by inspection, not assumed:

```rust
// src/application/mod.rs — already just data in, data out, no socket type anywhere
pub trait Application {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String>;
}
```

`Request` (`src/request/mod.rs`) and `Response` (`src/response/mod.rs`) are plain structs with
public fields — no embedded `TcpStream`/socket handle. `grep`ping the whole tree for
`std::net|TcpStream|TcpListener|std::thread|tokio::net|tokio::spawn` shows those touches are
confined to `server`, `h2_handler`, `h3_handler`, `thread_pool`, `proxy*`, `tcp_proxy`,
`udp_proxy`, `ws_proxy`, `websocket`, `ingress`, `acme`, `mailer`, `http_client`, `scheduler`,
`otel`, `log`, `redis_protocol`, `service_discovery` — every other module (`app`, `router`,
`controller`, `extract`, `cors`, `csrf`, `rate_limit`, `cache`, `di`, `rewrite`, `middleware`,
`validate`, `session`, ...) is pure request/response logic today. That's the reuse boundary this
plan builds on: the adapter translates wasi-http types into `Request`/`ConnectionInfo`, calls
`app.execute()` unchanged, translates the `Response` back.

**Target the stable WASI 0.2 `wasi:http/proxy` world, not WASI 0.3.** `wasm32-wasip2` reached
Rust Tier 2 in 1.82 (Nov 2024) and `wasmtime serve` has supported the 0.2 `proxy` world since
Wasmtime 18. WASI 0.3 (the async-native component model revision) is still RC-stage in 2026
Wasmtime builds, with reported `wit-bindgen`/`wasmtime serve` interop breakage on 0.3-generated
components — building against it now would mean chasing a moving target for no guest-side
benefit yet.

---

## Foundation — required before anything else ✅ shipped

### 1. `cfg`-gate every socket/thread-coupled module out of the wasm build — done

`server`, `thread_pool`, `proxy`, `proxy_config`, `tcp_proxy`, `udp_proxy`, `ws_proxy`,
`websocket`, `http_client` (native backend), `otel`, `log`, `redis_protocol`,
`service_discovery`, `ingress`, `canary`, `circuit_breaker`, `timeout` are now
`#[cfg(not(target_arch = "wasm32"))]` at their `pub mod` declaration in `src/lib.rs` — a target
property, not a feature choice, so it can't be misconfigured by a downstream `Cargo.toml`.
`h2_handler`/`h3_handler`/`acme`/`mailer` needed no change — already behind Cargo features
(`http2`/`http3`/`acme`/`mailer`) that the wasm build simply doesn't enable.

Module-level gating alone wasn't enough — three modules mix pure logic with a socket-backed
sub-feature and needed splitting at the item level instead of the whole file:

- `scheduler` — `pub mod cron;` (pure date/time math) stays on every target because
  `app::controller::static_resource` uses it unconditionally for `Last-Modified` formatting;
  only the thread-spawning `Scheduler`/`Task`/`TaskKind` struct and impls are gated.
- `rate_limit` and `session` — each has a Redis-backed struct (`RedisRateLimiter`,
  `RedisSessionStore`) built on `redis_protocol`'s `TcpStream`; only those structs/impls/the
  `use redis_protocol::...` line are gated, the core in-memory `RateLimiter`/`SessionStore` stay.
- `metrics` — `circuit_breaker_prometheus_text()` (an internal helper backing `/metrics`) got a
  `wasm32` stub returning `""` instead of calling into the now-gated `circuit_breaker` module.

Verified clean via `cargo check --target wasm32-wasip2 --no-default-features --lib`, and the full
native `cargo test` (1548 unit + 106 doc tests) still passes with default features — the gating
didn't regress the native build.

**Found and fixed a real blocker outside this repo**: the `app` module (always compiled — every
controller lives under it) depends on the [`file-ext`](https://crates.io/crates/file-ext) crate
for path handling, and `file-ext` 12.0.0 only implemented `get_path_separator`/`root`/recursive
delete for `target_family = "unix"`/`"windows"` — neither matches `wasm32-wasip2`'s
`target_family = "wasm"`. Fixed upstream (same author's sibling repo): added
`target_family = "wasm"` alongside the `unix` branch for path separator/root/temp-folder (WASI
paths are POSIX-shaped), and gave `remove_directory_recursively_bypass_warnings` a `wasm`-specific
body using `fs::remove_dir_all` instead of shelling out to `rm` (no subprocess model in a WASI
guest). Released as file-ext 12.1.0; all 42 unit + 28 doc tests still pass natively.

Modules that compiled clean without any changes needed (pure-Rust math/crypto, no wasm blocker in
practice): `flate2` (gzip, via `compression`), `hmac`/`sha2`, `argon2`/`aes-gcm`, `rsa`/`p256`,
`regex` — none of these were exercised in the Foundation build (no default features enabled), so
"compiles" here means the *core* crate with no optional features; each feature should still get
its own verification pass before being declared wasm-safe.

Explicitly deferred, not attempted in this project: `model` (the DB layer needs real TCP wire
protocols to Postgres/MySQL/SQLite-over-driver — `wasi:sockets` support exists in WASI 0.2 but no
driver here speaks it; `sqlx` itself assumes `tokio::net`). Revisit only if a `wasi:sockets`-based
driver becomes realistic.

### 2. New workspace package: `rws-wasm-shim` — done

Added as a new `[workspace]` member (the root `Cargo.toml` previously had no explicit `[workspace]`
table at all — `rws-macros` was an implicitly-included path *dependency*; `rws-wasm-shim` depends
*on* `rust-web-server` rather than the other way around, so it needed an explicit
`members = ["rws-macros", "rws-wasm-shim"]` instead):

```
rws-wasm-shim/
  Cargo.toml       # crate-type = ["cdylib"]; path-depends on rust-web-server, default-features = false
  src/lib.rs       # incoming-handler impl (Phase 1) + unit tests for the pure translation logic
```

**Divergence from the original sketch**: no hand-vendored `wit/proxy.wit` and no direct
`wit-bindgen` dependency were needed. The [`wasip2`](https://crates.io/crates/wasip2) crate
(`bytecodealliance/wasi-rs`) already vendors the `wasi:http@0.2.x` WIT package and ships generated
Rust bindings, including `wasip2::exports::http::incoming_handler::Guest`,
`wasip2::http::proxy::export!`, and (under its default `std` feature) `impl std::io::Read for
InputStream` / `impl std::io::Write for OutputStream`. One dependency (`wasip2 = "1.0"`) instead
of a wit-bindgen build step. `wit-bindgen`, `wasmtime`, `wasmtime-wasi-http` as dev-dependencies
turned out unnecessary too — see item 4.

Build invocation (unchanged from the original plan):

```bash
rustup target add wasm32-wasip2
cd rws-wasm-shim
cargo build --release --target wasm32-wasip2
wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm
```

---

## Phase 1 — Minimal guest: buffered request/response round-trip ✅ shipped

### 3. The adapter itself — done

`rws-wasm-shim/src/lib.rs` implements `Guest::handle`:

1. Reads `IncomingRequest`'s method, path-with-query, headers, and body (buffered via
   `IncomingBody::stream()` + `std::io::Read::read_to_end`) and builds a
   `rust_web_server::request::Request`. `http_version` is hardcoded to `"HTTP/1.1"` — `wasi:http`
   has no wire-version field at all, the abstraction hides it entirely.
2. Builds a placeholder `ConnectionInfo` (`sni_hostname: None`, `client`/`server` addresses both
   `0.0.0.0:0`) — a real wasi-http guest is never told the peer address or SNI hostname.
3. Calls `App::new().execute(&request, &connection)` — **zero changes** to any existing
   `Controller`/`Application`/`Router`/middleware implementation, confirming the seam.
4. Translates the returned `Response` (status, headers, buffered body via
   `Response::generate_body`) into an `OutgoingResponse` + `OutgoingBody`. `Err(String)` from
   `execute()` gets a 400 response mirroring `Server::bad_request_response`'s shape, since the
   native `Server` this would otherwise go through doesn't exist on this target.
   `stream_file`/`stream_pipe` responses are out of scope for Phase 1 — buffer only, deferred to
   Phase 2 item 6.

### 4. Local dev / test loop — done, differently than sketched

- **Interactive, verified for real**: built the release component, ran
  `wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm` (Wasmtime 46.0.1, installed via
  Homebrew), and `curl`'d `/`, `/healthz`, and a 404 path — all three came back with the correct
  status, headers (CORS/CSP/client-hints from `App`'s default header list), and body, through a
  real `wasi:http` component, not a mock.
- **Automated, native `cargo test -p rws-wasm-shim`**: the originally-sketched approach (a
  `wasmtime`-crate-embedding dev-dependency test) turned out unnecessary — `wasip2`'s
  generated bindings link fine on the host target (they're not `wasm32`-gated internally), so
  ordinary `#[cfg(test)]` unit tests run natively for every piece of translation logic that
  doesn't require a live WASI host: `method_to_string` (all 9 methods + the `Other` case),
  `bad_request_response` (status + body), `guest_connection_info` (documents the
  no-peer-address/no-SNI limitation). Functions that construct real `wasi:http` resources
  (`Fields::new()`, `OutgoingResponse::new()`, etc.) link but panic without a live host, so
  `to_wasi_headers`/`write_response`/`handle` are covered by the `wasmtime serve` smoke test above
  instead, run manually rather than wired into CI yet.
- **Still open**: an automated CI job that builds the component and drives the `wasmtime serve`
  smoke test in scripted form (the manual steps above, scripted) — see Toolchain / CI below.

---

## Phase 2 — Feature parity for stateless middleware + streaming

### 5. Middleware that only touches `Request`/`Response`

Verify and wire up, one at a time, against the shim: `RequestIdLayer`, `RewriteLayer`, `IpFilter`,
CORS, CSRF, `BasicAuthLayer`/`JwtLayer` (`auth` feature). All operate purely on the
already-in-memory `Request`/`Response` — no socket access — so should need no changes, only
verification under the target.

### 6. Streaming bodies

Wire `Response::stream_pipe`/`stream_file` through wasi-http's `output-stream` resource so large
responses don't have to be fully buffered in guest linear memory (which is bounded and typically
much smaller than a native process's heap).

### 7. Outbound HTTP backend for `http_client`

`src/http_client/mod.rs::Client` currently opens `TcpStream`/`rustls` directly — that doesn't
exist in a wasi-http guest. Add a second backend using wasi-http's `outgoing-handler`, selected by
`#[cfg(target_arch = "wasm32")]`, behind the same public `Client`/`RequestBuilder` API. This is
what unlocks `sso`, `secrets`, `storage-s3`/`storage-azure`, `ForwardAuthLayer`, and webhook
verification code paths inside a guest — all of them go through `http_client` already, so this one
adapter covers all of them for free once it lands.

---

## Phase 3 — Explicitly out of scope (needs a decision, not just effort)

**The biggest open risk in this whole project**: WASI-HTTP guest components are commonly
instantiated *fresh per request* by real hosts (Fastly Compute, Spin) for isolation — this is a
platform property, not a bug. Any process-wide `Mutex`/`OnceLock`/`AtomicBool` state silently
resets on every single call unless the specific host you deploy to guarantees instance reuse
(`wasmtime serve` can pool instances; that is not a portable guarantee across hosts). This
directly affects:

- `RateLimiter`'s sliding window (`rate_limit`) — would silently stop limiting anything.
- `CacheLayer` (`cache`) — every request would miss.
- In-memory `SessionStore` (`session`) — every request would be a fresh session.
- `metrics` counters, MCP's `sessions`/`sse_clients` maps.

None of this can be fixed by more Rust code in this crate — it needs either a host-provided KV/
session binding (wasi-keyvalue proposal, still pre-1.0) or an explicit documented restriction:
"stateful middleware requires an instance-reuse-guaranteeing host." Do not ship this feature
implying rate limiting/caching "just works" under WASM without validating against the specific
host the plan targets.

Left native-only, not attempted: `thread_pool`, `scheduler`, `jobs` background queue, WebSocket
upgrade (`websocket`, `ws_proxy`), L4 `tcp_proxy`/`udp_proxy`, `GrpcProxy`/`H2ReverseProxy`, ACME,
`mailer`'s raw SMTP socket, the `model` DB layer. All assume real OS sockets or threads that a
wasi-http guest is never given.

---

## Toolchain / CI

- `wasm32-wasip2` is Rust Tier 2 since 1.82 — **higher than this crate's overall MSRV of 1.75**.
  Confirmed in practice: this environment's toolchain is 1.96, well past the floor; `rustup target
  add wasm32-wasip2` and `brew install wasmtime` (46.0.1) both worked with no friction.
- No dev-only tooling needed in the end — `wit-bindgen`/`wasmtime`/`wasmtime-wasi-http` as
  dev-dependencies turned out unnecessary (see Phase 1 item 4). `rws-wasm-shim`'s only dependency
  is `wasip2 = "1.0"`.
- **Still open**: a CI job that installs `wasm32-wasip2` + `wasmtime`, builds the release
  component, runs it under `wasmtime serve` in the background, and curls it (scripting the manual
  verification already done) — kept separate from the main `cargo test` job, not blocking it.
- file-ext 12.1.0 is published to crates.io; the root `Cargo.toml`'s existing `"12.0.0"`
  requirement resolves it directly via `^12.0.0` — no `[patch]` section needed.

---

## Sequencing (treat as one project, like `KAFKA_ROADMAP.md`)

1. **Foundation** ✅ — `cfg` gate the socket/thread modules out of wasm builds; scaffold
   `rws-wasm-shim`; fix `file-ext` for `wasm32`. Compiles clean for the target.
2. **Phase 1** ✅ — buffered request/response round trip through `App::execute()`; verified against
   a real `wasmtime serve` process via curl; native unit tests for the pure translation logic.
3. **Phase 2** (open) — stateless middleware parity, streaming bodies, `http_client`
   outgoing-handler backend, and the CI job described above.
4. **Phase 3** (open) — write down (don't silently ship) which stateful features are unsupported
   per-host, and revisit WASI 0.3 once it's out of RC.

Effort/risk for the remaining phases: unchanged from `TODO_FINAL.md`'s "Very large" rating — the
biggest unknowns are (a) per-request instance lifecycle breaking every stateful middleware
silently, and (b) real-world crate compatibility of `argon2`/`rsa`/`p256`/`regex` under
`wasm32-wasip2` once those optional features are actually enabled (untested — Foundation only
verified the core, no-default-features build).
