---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40206635514) of the
[Benchmark workflow](https://github.com/bohdaq/rust-web-server/blob/main/.github/workflows/bench.yml)
on a 4-CPU GitHub Actions runner (Linux x86_64), with the load generator on the
same machine.

**Load:** [`oha`](https://github.com/hatoo/oha), 64 keep-alive connections,
20 seconds per scenario after a 2-second warm-up, HTTP/1.1, no compression
(`--disable-compression`: the Caddy and nginx configs don't compress, and
`rws` compresses text by default), access logs off everywhere. `rws` is the
release build (`--features "acme auth mimalloc"`), Caddy 2.11.7, nginx from Ubuntu with
`sendfile`, `open_file_cache` and upstream keep-alive enabled. nginx's
`open_file_cache_valid 1s` trusts a cached file's size and modification time
for a second; `rws` runs with the equivalent `open_file_cache_strict = false`
(its default re-checks them with an `fstat` on every request, so an edit in
place is served at once).

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 99,393 | 0.60 | 1.56 | 16.5 | 33.4 |
| | Caddy | 18,399 | 2.77 | 13.79 | 154.4 | 68.8 |
| | nginx | 81,016 | 0.54 | 3.13 | 24.7 | 23.4 |
| Static file, 1 MiB | rws | 9,413 | 6.27 | 14.08 | 211.4 | 33.4 |
| | Caddy | 2,438 | 18.63 | 124.06 | 1,184.9 | 71.8 |
| | nginx | 10,029 | 6.04 | 14.33 | 211.8 | 23.5 |
| Reverse proxy, 1 KiB | rws | 34,076 | 1.81 | 3.90 | 45.5 | 37.5 |
| | Caddy | 13,693 | 4.08 | 13.86 | 189.8 | 69.1 |
| | nginx | 33,975 | 1.76 | 4.45 | 47.5 | 24.6 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 87,858 | 0.68 | 1.86 | 20.6 | 41.8 |
| | Caddy | 5,714 | 8.42 | 54.37 | 606.0 | 110.8 |
| | nginx | 1,806 | 34.22 | 60.11 | 2,060.0 | 27.8 |
| Static 1 KiB, access log on | rws | 88,448 | 0.66 | 1.82 | 20.3 | 41.5 |
| | Caddy | 15,825 | 2.98 | 18.40 | 193.1 | 65.8 |
| | nginx | 77,402 | 0.59 | 3.09 | 27.7 | 23.2 |
| Static 1 KiB, HTTPS | rws | 71,484 | 0.81 | 2.22 | 25.5 | 51.9 |
| | Caddy | 25,897 | 2.21 | 9.11 | 112.4 | 61.7 |
| | nginx | 63,533 | 0.71 | 3.56 | 33.1 | 41.9 |
| Static 1 KiB, HTTP/2 | rws | 67,700 | 0.88 | 2.34 | 26.5 | 62.1 |
| | Caddy | 13,527 | 4.08 | 14.12 | 216.7 | 66.6 |
| | nginx | 51,542 | 0.75 | 4.33 | 37.2 | 42.1 |
| Static 1 MiB, HTTPS | rws | 1,324 | 42.33 | 98.26 | 1,652.5 | 67.8 |
| | Caddy | 1,471 | 39.16 | 114.48 | 1,394.0 | 68.3 |
| | nginx | 1,364 | 41.98 | 105.70 | 1,513.1 | 42.5 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is served by
`rws`'s own implementation (since 18.0.0): with the `h2` crate, framing and
HPACK cost more per request than all of nginx's own code, and nginx led this
scenario by 23–41%. Large files over TLS are where nginx can still come out
ahead: the encryption dominates (rustls in `rws`, OpenSSL in nginx), and the
result flips with the runner's CPU.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.9× (large) to 5.4× (small) as many file
  requests, 2.5× as many proxied and 5.0× as many HTTP/2 requests per
  second, using a quarter to a ninth of the CPU per request, with a p99
  three to nine times lower and about half the memory.
- **Against nginx:** less CPU per request on small files (−33%, 23% more
  requests per second), proxying (−4%), HTTP/2 (−29%, 31% more requests per
  second), with an access log (−27%), over HTTPS (−23%) and with gzip (a
  hundredth); the same on large files; more for large files over TLS (+9%
  here — on faster runners `rws` was 28–38% ahead). `rws` has the lower p99
  in every scenario. The load generator shares the runner's 4 CPUs, so the
  CPU/request column is the comparison to read.
- **Memory:** release builds allocate with mimalloc, which costs about 5%
  less CPU per request than the system allocator but holds more memory —
  33–68 MiB here, against nginx's 23–43. Built without the `mimalloc`
  feature, `rws` peaks at 9–17 MiB.
- The gap depends on the runner's hardware; same-run A/Bs (`base_ref`,
  below) are how changes are judged.
- Shared CI runners are noisy: absolute numbers vary by a factor of two
  between runs. Compare servers within one run, not numbers across runs.
- Compression matters: with `Accept-Encoding: gzip` sent and only `rws`
  compressing, the comparison measures `gzip`, not the servers. Compare like
  for like.

## Why it's fast

- **Open files are reused** (an open-file cache like nginx's): a request for
  a hot file costs one `fstat`; files up to 64 KiB are served from memory,
  and the response head and body go out in one write.
- **Quick requests run on the event loop.** Static files, health checks and
  metrics skip the hand-off to a blocking thread pool, as in nginx. Handlers
  that may block (your routes) still get a pool thread, so a slow handler
  can't stall other connections. See
  [`runs_inline`](/building-apps/middleware/#running-on-the-event-loop-runs_inline).
- **Proxying holds no thread.** Proxy routes without middleware, to HTTP or
  HTTPS backends, wait for the backend on the connection's task, over pooled
  keep-alive connections.
- **One idle timer per connection**, pushed back on each read without a lock.
- **`sendfile` for downloads** over plain HTTP on Linux and macOS: the kernel
  copies file pages straight to the socket, with the response head in the
  same segment (`TCP_CORK` on Linux, `sendfile`'s header vector on macOS).
- **Configuration is a cached snapshot**, not read from the environment on
  every request; a reload replaces it.
- **Few allocations per request.** The standard response headers are
  borrowed `&'static str`s (`Header` fields are `Cow<'static, str>`), so
  attaching them copies nothing; request heads and upstream response heads
  are parsed in one pass over the bytes; numbers are formatted on the stack;
  the file check is one `fstat`.

## Reproduce

Run the workflow from the repository's Actions tab ("Benchmark" → "Run
workflow"), or locally:

```bash
cargo build --release --bin rws --features "acme auth"
RWS=target/release/rws OHA=oha CADDY=caddy NGINX=nginx bench/run.sh
```

`CADDY` and `NGINX` are optional; the proxy scenario needs `NGINX`, which also
serves as the shared backend for every proxy under test. `DURATION` (default
`15s`) and `CONNECTIONS` (default `64`) tune the load. The script prints a
Markdown table and writes `bench-results.md` and `bench-results.json`.

`EXTENDED=1` (on by default in the workflow) adds the paths real deployments
hit that the three scenarios above leave out, each with equivalent settings
on every server: `text-gzip` (a 26 KiB JavaScript file with
`Accept-Encoding: gzip`, gzip level 6 everywhere), `log-1k` (an access log
written to a file), `https-1k` and `h2-1k` (TLS over HTTP/1.1 and HTTP/2),
and `https-1m` (a 1 MiB file over TLS, where `sendfile` doesn't apply). The
script makes a self-signed certificate with `openssl`, and skips a TLS
scenario for an nginx built without its ssl or http2 module.

## Profile

To see where `rws` spends its CPU, run the workflow with **profile** checked:
it builds with frame pointers and samples `rws` with `perf` during each
scenario, and nginx's workers too (`nginx-<scenario>` reports; the distro
nginx has no symbols, but its kernel time is comparable with `rws`'s). The job summary lists the hottest symbols, and the `bench-profile`
artifact has flat and call-tree reports plus `perf script` output for flame
graphs (`inferno-collapse-perf | inferno-flamegraph`). Locally, on Linux:

```bash
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release --bin rws --features "acme auth"
PERF_DIR=perf RWS=target/release/rws OHA=oha NGINX=nginx bench/run.sh
perf report -i perf/static-1k.data --no-children
```

Sampling slows `rws` down, so take throughput numbers from an unprofiled run.

## Compare two builds

Runner hardware differs from run to run (nginx itself varies threefold), so
a change can only be judged against its baseline in the same run. Give the
workflow a **base_ref** (a branch, tag or commit) and it also builds that
ref and benches it as `rws-base`, right after `rws`; locally, set
`RWS_BASE=<binary>`.
