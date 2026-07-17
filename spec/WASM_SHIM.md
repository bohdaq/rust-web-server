[Read Me](README.md) > WASM Shim Roadmap

# WASM / `wasm32-wasip2` Shim Roadmap

What is needed to run `rws` *inside* a WebAssembly runtime (Wasmtime, Spin, Fastly Compute, WasmEdge) as
opposed to embedding a WASM plugin *inside* `rws` (that's the separate "Wasm middleware" idea in
`IDEAS.md`, unrelated to this doc). Closes `GAPS_V3.md` §3.19 / `TODO.md`'s WebAssembly item /
`TODO_FINAL.md` #28. Rated "Very large / biggest unknown" there — this doc exists to turn that
one line into a sequenced, scoped project the same way `KAFKA_ROADMAP.md` did for Kafka.

**Status: Foundation, Phase 1, and most of Phase 2 are shipped** (`rws-wasm-shim/` +
`http_client`'s new wasm32 backend), verified end-to-end against a real `wasmtime serve`
process — not just a compile check, including real outbound HTTP **and HTTPS** (TLS terminated by
the host) and a 9 MB streamed file download that came back byte-identical. Phase 3's biggest risk
(per-request instance lifecycle) has been **empirically confirmed true** for `wasmtime serve`, not
just theorized. See the per-item notes below for what shipped, what's still open, and where the
implementation diverged from the original sketch.

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

### 5. Middleware that only touches `Request`/`Response` — done, verified per-feature

Every feature below was individually built for `wasm32-wasip2` via
`cargo check --target wasm32-wasip2 --no-default-features --features "<name>" --lib` and came back
clean, no code changes needed beyond Foundation's module gating: `auth` (`BasicAuthLayer`,
`JwtLayer`, and — once item 7 below landed — `ForwardAuthLayer` too), `auth-asymmetric`, `csrf`,
`crypto`, `webhook`, `rewrite-regex`. `RequestIdLayer`, `RewriteLayer`, `IpFilter`, CORS, and the
core in-memory `RateLimiter`/`CacheLayer` were already unconditionally available since Foundation
(no feature flag). `sso`, `sso-server`, `sso-saml`, `secrets`, `storage-s3`, and `storage-azure`
*also* came back clean — see item 7, which is what actually unblocked them.

### 6. Streaming bodies — done

`rws-wasm-shim`'s `write_response` now branches on `Response::stream_pipe`/`stream_file` before
falling back to the buffered `content_range_list` path, copying through the `wasi:http`
output-stream in 8 KB chunks via a small `copy_in_chunks` helper instead of buffering the whole
body. **Verified for real**: served a 9 MB file (well past the native server's 8 MB
buffer-vs-stream threshold) through a `wasmtime serve` process with an explicit `--dir` grant, and
the downloaded bytes were identical to the source file (`diff` clean).

### 7. Outbound HTTP backend for `http_client` — done

`src/http_client/mod.rs::send_once` (the one-hop-no-redirect core `RequestBuilder::send()` calls
into) now has a `#[cfg(target_arch = "wasm32")]` twin that builds a `wasi:http`
`OutgoingRequest`/`OutgoingBody`, calls `outgoing_handler::handle`, blocks on the
`future-incoming-response` via `.subscribe().block()`, and reads the result back into the same
`Response` struct the native `TcpStream`/`rustls` backend produces — `ParsedUrl`, the redirect
loop, and the whole `Response` API are untouched and shared between both backends. The host
performs the actual connect and TLS handshake; the guest never touches `rustls` for outbound
requests regardless of `http://` vs `https://`.

**This unblocked more than expected.** Enabling `http-client` (or anything depending on it —
`sso`, `sso-server`, `sso-saml`, `secrets`, `storage-s3`, `storage-azure`, `auth::forward`'s
`ForwardAuthLayer`) for `wasm32-wasip2` still failed even with the new backend in place, because
`rustls`'s own `aws-lc-rs` crypto-backend feature compiles C/asm (`aws-lc-sys`) that doesn't build
for `wasm32-wasip2` (`fatal error: 'sys/types.h' file not found`) — and Cargo pulls in an optional
dependency's full feature set the moment any enabled feature says `dep:rustls`, regardless of
whether the wasm32 code path ever calls into it. Fixed by moving `rustls`/`webpki-roots` into a
`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table in the root `Cargo.toml` — native
builds are unaffected (verified: `cargo test` and `cargo check --features http2/acme` all still
pass), and wasm32 builds simply never see `rustls` at all. `tls_connect` (native TLS handshake
helper, used by both `http_client` and `mailer`) got a matching `not(target_arch = "wasm32")` added
to its existing feature gate.

**`mailer` needed its own gate, not a fix**: it uses `TcpStream::connect` directly for the raw SMTP
protocol (not HTTP at all — there's no `wasi:http`-equivalent for arbitrary TCP protocols), so it's
now `#[cfg(all(feature = "mailer", not(target_arch = "wasm32")))]` in `src/lib.rs`, matching
`tcp_proxy`/`udp_proxy`/`websocket`.

**Verified for real**, not just compiled: `Client::new().get(url).send()` from inside a running
`wasmtime serve` guest round-tripped correctly against both a local plain-HTTP test server and
`https://example.com` (real TLS, terminated by Wasmtime's host implementation) — same public API,
zero code changes needed in `sso`/`secrets`/`storage-s3`/`storage-azure`/`auth::forward` themselves
to make them wasm32-compatible; they were already only using `http_client::Client`.

One more thing this surfaced: `secrets`'s Vault/AWS-SM/Key-Vault backends reuse
`service_discovery::json_lite` (a small hand-rolled JSON parser) purely for parsing, but the whole
`service_discovery` module was gated out in Foundation because `BackendPool`/`DiscoverySource`
(DNS/etcd/Consul/Docker) genuinely need sockets. Split the same way `scheduler::cron` was in
Foundation: `json_lite` (plus the `pub mod service_discovery;` declaration itself) stays available
on every target; `DiscoverySource`, `BackendPool`, and the `consul`/`dns_srv`/`docker`/`etcd`
submodules are now gated at the item level.

---

## Phase 3 — Stateful middleware and instance-reuse (confirmed, not just theorized)

**The biggest open risk in this whole project — empirically confirmed true, not a hypothetical.**
`wasmtime serve` instantiates a **fresh guest per request**. Proof: `rws-wasm-shim`'s adapter was
temporarily modified to call `rust_web_server::metrics::record_request()` five times per request
before generating the response body; the *first* `/metrics` request showed `rws_requests_total 5`
(from its own five calls), and every subsequent `/metrics` request — second, third, fourth — showed
exactly `5` again, never `10` or `15`. Global state (an `AtomicU64` behind a `OnceLock`) resets to
zero every single call. This is a platform property of `wasmtime serve`, not a bug reachable from
Rust code in this crate, and other hosts (Fastly Compute, Spin) are widely understood to behave the
same way for the same isolation reasons — though only `wasmtime serve` was actually tested here.

This directly affects, and currently silently breaks under `wasmtime serve` specifically:

- `RateLimiter`'s sliding window (`rate_limit`) — does not limit anything across requests.
- `CacheLayer` (`cache`) — every request misses.
- In-memory `SessionStore` (`session`) — every request is a fresh, empty session.
- `metrics` counters, MCP's `sessions`/`sse_clients` maps, `RequestIdLayer`'s ID generator's
  internal counter (the ID itself still gets generated correctly per-request, just not
  monotonically across requests).

None of this can be fixed by more Rust code in this crate — it needs either a host-provided KV/
session binding (the `wasi-keyvalue` proposal, still pre-1.0) or an explicit documented
restriction: **stateful middleware requires an instance-reuse-guaranteeing host, and `wasmtime
serve` is confirmed not to be one.** Do not ship this feature implying rate limiting/caching "just
works" under WASM — it doesn't, at least not under the one host actually tested.

**A second, separate limitation found during Phase 2 verification**: filesystem access is **not**
part of the `wasi:http/proxy` world's base contract (unlike outbound HTTP, which is — see item 7)
— it's a separate, opt-in host grant. Running `wasmtime serve` with no `--dir` flag, the static-file
controllers silently fall back to `rws`'s built-in embedded default pages (favicon/index/404/etc.)
for *every* path, even ones that would resolve to a real file if one existed — there is no error,
just a quiet fallback to the wrong content. Passing `--dir HOST_DIR::.` fixed this immediately
(confirmed: a real `index.html` and a 9 MB file both served correctly, see item 6). Document this
prominently for anyone deploying static-file serving in a wasm32 guest — it depends entirely on the
specific host granting filesystem access, and silently "working" with embedded fallback content
instead of real files is easy to miss in testing.

Left native-only, not attempted — all assume real OS sockets or threads that a wasi-http guest is
never given, and have no wasi:http equivalent to bridge to (unlike `http_client`, which does):
`thread_pool`, `scheduler`'s `Scheduler` (background jobs), `jobs` background queue, WebSocket
upgrade (`websocket`, `ws_proxy`), L4 `tcp_proxy`/`udp_proxy`, `GrpcProxy`/`H2ReverseProxy`, ACME,
`mailer`'s raw SMTP socket, the `model` DB layer, `service_discovery`'s DNS/etcd/Consul/Docker
sources.

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
3. **Phase 2** ✅ mostly — stateless middleware parity (verified per-feature), streaming bodies
   (verified with a real 9 MB file), `http_client` outgoing-handler backend (verified with real
   HTTP and HTTPS), which in turn unblocked `sso`/`secrets`/`storage-s3`/`storage-azure`/
   `ForwardAuthLayer` for free. Only the CI job (scripting the manual `wasmtime serve` verification)
   remains open.
4. **Phase 3** — the stateful-middleware risk is now **confirmed**, not theorized (see above);
   revisit WASI 0.3 once it's out of RC remains open.

Effort/risk for what's left: much smaller than the original "Very large" rating implied — the
transport-level work (Foundation + Phase 1 + Phase 2) is done and verified. What remains is mostly
documentation/decision work (Phase 3's per-host stateful-middleware writeup) plus routine CI
plumbing, not further exploratory engineering.
