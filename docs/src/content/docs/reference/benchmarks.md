---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40209996636) of the
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
| Static file, 1 KiB | rws | 153,668 | 0.37 | 0.96 | 9.9 | 33.4 |
| | Caddy | 35,444 | 1.25 | 8.02 | 75.5 | 69.1 |
| | nginx | 166,909 | 0.36 | 1.13 | 12.4 | 23.6 |
| Static file, 1 MiB | rws | 15,305 | 3.99 | 9.28 | 142.0 | 33.4 |
| | Caddy | 4,027 | 11.26 | 75.06 | 651.6 | 73.4 |
| | nginx | 16,354 | 4.06 | 8.96 | 137.1 | 23.7 |
| Reverse proxy, 1 KiB | rws | 61,311 | 0.98 | 2.63 | 24.1 | 35.4 |
| | Caddy | 29,427 | 1.75 | 7.90 | 79.9 | 69.1 |
| | nginx | 65,415 | 0.87 | 2.80 | 24.2 | 24.5 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 148,531 | 0.38 | 1.03 | 11.1 | 39.7 |
| | Caddy | 13,491 | 3.24 | 23.96 | 253.9 | 109.3 |
| | nginx | 2,599 | 29.17 | 43.31 | 1,145.9 | 27.5 |
| Static 1 KiB, access log on | rws | 153,129 | 0.37 | 0.98 | 10.7 | 41.6 |
| | Caddy | 35,015 | 1.08 | 9.28 | 80.5 | 66.9 |
| | nginx | 142,905 | 0.27 | 2.56 | 14.5 | 23.6 |
| Static 1 KiB, HTTPS | rws | 143,688 | 0.40 | 1.10 | 11.7 | 47.3 |
| | Caddy | 60,882 | 0.68 | 5.68 | 44.1 | 124.7 |
| | nginx | 135,672 | 0.25 | 3.24 | 14.3 | 42.7 |
| Static 1 KiB, HTTP/2 | rws | 126,973 | 0.44 | 1.18 | 12.7 | 55.3 |
| | Caddy | 28,971 | 1.87 | 7.20 | 97.0 | 72.1 |
| | nginx | 124,057 | 0.28 | 3.27 | 14.9 | 43.1 |
| Static 1 MiB, HTTPS | rws | 3,954 | 14.64 | 26.16 | 368.8 | 65.6 |
| | Caddy | 2,959 | 19.26 | 57.56 | 853.8 | 70.8 |
| | nginx | 3,338 | 17.29 | 45.52 | 667.9 | 43.7 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is served by
`rws`'s own implementation (since 18.0.0): with the `h2` crate, framing and
HPACK cost more per request than all of nginx's own code, and nginx led this
scenario by 23–41%. Large files over TLS were slow on AMD
EPYC 7763 runners until 18.0.0: rustls copies each record's plaintext into
a buffer 5 bytes past a page boundary, and with the file buffer page-aligned
too, glibc's `rep movsb` ran that copy at 2.8 instead of 40 GB/s (4 KiB
aliasing). The file buffer now starts mid-page; that scenario went from
nginx +9% to `rws` −35 to −50%.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.8× (large) to 4.3× (small) as many file
  requests, 2.1× as many proxied and 4.4× as many HTTP/2 requests per
  second, using a third to an eighth of the CPU per request, with a p99
  three to eight times lower and about half the memory.
- **Against nginx:** less CPU per request for small files (−20%), with an
  access log (−26%), over HTTPS (−18%), over HTTP/2 (−15%), for large files
  over HTTPS (−45%) and with gzip (a hundredth); the same for proxying; 4%
  more for large files (`sendfile` on both — the kernel does that work).
  `rws` has the lower p99 in every scenario but large files. This run used
  the fastest runner type GitHub assigns, where nginx also serves 6–8% more
  plain-HTTP requests per second. On an AMD EPYC runner (run 40206635514,
  just before the large-file TLS fix below) `rws` needed less CPU than nginx
  in every scenario but large files over HTTPS — which that fix then turned
  into a 35–50% lead — and served more requests per second in all but the
  two large-file scenarios. The load generator shares the runner's 4 CPUs, so the
  CPU/request column is the comparison to read.
- **Memory:** release builds allocate with mimalloc, which costs about 5%
  less CPU per request than the system allocator but holds more memory —
  33–66 MiB here, against nginx's 24–44. Built without the `mimalloc`
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
`RWS_BASE=<binary>`. **base_workers** (`RWS_BASE_WORKERS`) runs `rws-base`
with that many tokio workers — with no base ref, the same build again — to
compare worker counts. (Fewer workers than CPUs raises the measured
requests per second only because the load generator shares the runner's
CPUs; on a dedicated server one worker per core is right.)
