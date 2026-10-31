# HTTP/3 to upstreams (#37)

Written 2026-10-31. Roadmap item #37 ([ROADMAP.md](ROADMAP.md)). Built
tranche by tranche; this file is updated as each lands. Target release:
**18.13.0**.

## What exists

- rws **serves** HTTP/3 (`h3_handler`, `quinn` 0.11 + `h3` 0.0.8 +
  `h3-quinn` 0.0.10, feature `http3`, the default) and its tests already use
  the `h3` **client** against it (`h3_handler/tests.rs`).
- Upstreams speak HTTP/1.1 (`forward_upstream*`, pooled; the async
  forwarder `async_upstream::forward` for config proxy routes) or HTTP/2
  (`h2_upstream`: one multiplexed connection per backend and trust setting
  on its own runtime `rws-h2-upstream`, streaming both ways, trailers).
- Upstream TLS: WebPKI, or `tls_ca_file` / client certificates
  (`UpstreamTls`), per `[[upstream]]`.

## Decisions

User, 2026-10-31:
- **Explicit opt-in per upstream**: `[[upstream]] protocol = "h3"` with
  ordinary `https://` backends (QUIC needs TLS). Default `protocol` stays
  what it is today; no Alt-Svc discovery.
- **Fall back to TCP**: when the QUIC connection can't be made (UDP blocked,
  handshake timeout), the request goes over HTTPS on TCP (HTTP/1.1, same
  host and port), that backend stays on TCP for a while (5 min), and
  `rws_upstream_h3_fallbacks_total{upstream}` counts it. The client sees no
  error. A failure after the QUIC connection is up is an ordinary backend
  failure (breaker, retries).
- **Scope**: config-file `proxy` routes (DynamicProxy, sync and async
  paths) and an `H3ReverseProxy` middleware for library apps. Gateway API
  and the `grpc` action stay on HTTP/1.1 / HTTP/2.

Implementation (defaults):
- **`proxy::h3_upstream`**, shaped like `h2_upstream`: a process-wide
  runtime `rws-h3-upstream` (2 workers) owning one `quinn::Endpoint` (bound
  to `0.0.0.0:0` / `[::]:0`) and one multiplexed `h3` connection per
  `(backend, trust)`; entries carry an id so a closing connection removes
  only itself; the connection driver runs as a task. ALPN `h3`; the rustls
  client config comes from the upstream's `UpstreamTls` (or WebPKI), cloned
  with ALPN set, through `crate::tls_client_builder()`.
- **Requests**: head + body sent as in `h2_upstream` (a `Body::Stream` read
  on the runtime's blocking pool), response head returned over a channel,
  body streamed through `stream_pipe` (`BodyReader`), trailers into
  `Response::trailers`; small non-gRPC responses with a length are
  buffered. Hop-by-hop headers dropped both ways; `:authority` = backend.
- **Async path**: `DynamicProxy::execute_async` awaits the h3 forward via a
  oneshot from the h3 runtime (no blocking pool hop); `execute` waits on a
  std channel. Retries, breaker, weights, sticky, mirror and early hints keep
  working through `prepare`/`settle` unchanged.
- **Fallback**: `connect` failures (no handshake within the connect
  timeout, ICMP unreachable, TLS alert) mark the backend `tcp_until =
  now + 5 min` and call the existing HTTPS forwarder for that request. A
  request body that was a stream already partly consumed can't fall back —
  then it's a 502 (only possible if the failure comes mid-request, which
  isn't a connect failure).
- **Health checks** for an h3 upstream stay HTTP/1.1 over TCP (they check
  the backend, not the transport); documented.
- **`rws --check`**: `protocol` must be `http1` (the default spelling) or
  `h3`; `h3` needs `https://` backends and the `http3` feature in this
  build; `h3` with `tls = false` / `http://` backends is an error.
- **Metrics**: `rws_upstream_h3_fallbacks_total{upstream}`; requests keep
  their `rws_upstream_*` series.

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | h3 client | `proxy::h3_upstream::forward(request, peer, target)`: runtime, endpoint, pooled connections, streaming both ways, trailers, buffering rule, timeouts; unit tests against `h3_handler` in-process (GET, POST body, streamed body both ways, big body, trailers, a dead connection replaced, connect timeout) |
| 2 | Config + DynamicProxy | `[[upstream]] protocol = "h3"`; both execute paths; TCP fallback with the 5-minute memory and metric; `rws --check`; tests incl. fallback when nothing listens on UDP |
| 3 | Library + e2e | `H3ReverseProxy` (`Middleware`, with `UpstreamTls`); docs (proxy pages, config file, api.md, CHANGELOG); `ci/e2e/h3_upstream.sh`: rws → rws over QUIC (a backend rws with a cert from a test CA; the proxy trusts it via `tls_ca_file`), the backend sees HTTP/3 (access log `protocol: HTTP/3`), then UDP blocked (backend's QUIC port closed) → served over TCP, fallback counted |
| 4 | Release | A same-run benchmark note (proxy over h3 vs h1 to the same backend, local), 18.13.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing (the `http1` build has no QUIC:
`protocol = "h3"` is a `--check` error there).

## Risks

- `h3` 0.0.8's client API (`SendRequest`, driver `poll_close`) is young;
  pin the versions already locked and keep the client code in one module.
- QUIC to a backend on the same host in CI: loopback UDP works (the
  `h3_handler` tests already do it).

## Status

| # | State |
|---|---|
| 1 | **done 2026-10-31**: `proxy::h3_upstream` (feature `http3`): runtime `rws-h3-upstream` (2 workers); QUIC client endpoints per address family made lazily on it; `quinn::ClientConfig` per rustls config (ALPN `h3`, cached by pointer with the source `Arc` kept); pool of `h3` `SendRequest`s keyed by trust + address with connection ids, the driver's `poll_close` removes the entry. `forward` (sync, std channel) and `forward_async` (oneshot), `H3Error::{Connect, Failed}` — name lookup, endpoint, handshake (bounded by the connect timeout), TLS alert and HTTP/3 setup are `Connect`. Request head drops hop-by-hop headers, keeps `te: trailers`, adds `x-forwarded-*` and `via: 3 rws`; `RequestStream::split`: body sent by a task (`Body::Bytes` at once, `Body::Stream` read on the blocking pool, a read error stops the stream), response head returned at once, ≤ 1 MiB non-gRPC with a length buffered, else `stream_pipe`; trailers into `Response::trailers`; each wait bounded by the read timeout. Tests (6, against `h3_handler` in-process with a self-signed cert): GET + forwarded headers + pooled connection, 404, body whole and streamed (45 KB), 2 MiB streamed response, trailers, connect failure on a dead UDP port within the timeout + untrusted certificate = `Connect`, async entry, backend restart on the same port → reconnect. 5 repeated runs green. 2204 default / 2018 http1 |
| 2 | **done 2026-10-31**: `UpstreamConfig.protocol` (`protocol` key); `DynamicProxy::with_h3` (in every build, a no-op without `http3`) holds a per-backend `tcp_until` map; both execute paths try `h3_upstream` first when `h3_target` allows (trust = `UpstreamTls::client_config()` or the WebPKI `tls_client_config()`), `H3Error::Connect` → `fall_back_to_tcp` (5 min, `rws_upstream_h3_fallbacks_total{upstream}`, logged) and the usual HTTP/1.1-over-TLS forward for that request; any other result goes through `settle` (breaker, retries) like a TCP one. `rws --check`: `protocol` ∈ {`http1`, `h3`}, `h3` needs `https://` backends and the `http3` feature. Tests (3 more, 9 in the module): config route over HTTP/3 on the sync and the async path (backend sees `HTTP/3.0`, `via: 3 rws`), TCP-only TLS backend → served over TCP within the connect timeout, second request with no QUIC attempt and no new fallback count, check errors. Also fixed: feature-combo warnings (`dictionary` without zstd/brotli, `Sink::reset` in http1, `with_h3`/metric without http3). Docs: proxy page "HTTP/3 upstreams", config file key, CHANGELOG. 2207 default / 2018 http1 |
| 3 | **done 2026-10-31**: `proxy::H3ReverseProxy` (`http3`): wraps `ReverseProxy` (`new`, `path_prefix`, timeouts, `tls_ca_file`, `tls_client_cert`) plus `.name()` (fallback metric label, default `h3-reverse-proxy`); round-robin with failover like `H2ReverseProxy` (one attempt for a streamed body), HTTP/3 per `https://` backend unless on TCP, `Connect` → 5 min TCP + metric + log, then `forward_upstream_trusting`; non-`https://` backends go HTTP/1.1. Test: over HTTP/3 behind a prefix, pass-through outside it, TCP-only TLS backend → TCP, counted once. `ci/e2e/h3_upstream.sh` (CI `h3-upstream`): openssl cert (needs `basicConstraints=CA:FALSE` — webpki refuses a CA as end entity), backend rws with TLS on TCP+UDP, proxy rws with two `protocol = "h3"` upstreams: backend access log shows `HTTP/3.0` for every proxied request (the log flushes every 100 ms — polled), none fell back; Python HTTPS (TCP only) → served over TCP after the 1 s connect timeout (no ICMP shortcut seen on loopback), the next request in ~70 ms, `rws_upstream_h3_fallbacks_total{upstream="tcp-only"} 1`. 3 runs green. Docs: reverse-proxy "HTTP/3 backends", api.md row, CHANGELOG. 2208 default / 2018 http1 |
| 4 | **benchmarked 2026-10-31** (local, `scratchpad/h3bench/run.sh`: release build, proxy rws → backend rws with TLS on one 8-core box, 1 KiB file, oha 1.16 -c 50 10 s, 3 alternating rounds, proxy CPU from /proc): HTTP/1.1-over-TLS ~41.6k req/s, 64 ms CPU/1000 req; HTTP/3 first ~19k req/s, 144 ms/1000 — the async path waited on a std channel through `spawn_blocking`; replaced by a tokio oneshot (`blocking_recv` for the sync path, no timeout needed: every wait in the exchange is bounded) → ~20.7k req/s, 112 ms/1000 (−22% CPU). The rest is QUIC's cost (user-space packets + crypto, one multiplexed connection); documented on the proxy page as such. Bumped to 18.13.0 (CHANGELOG dated, Cargo/helm/site lock/llms); tag after CI is green |
