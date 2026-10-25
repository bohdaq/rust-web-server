---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40243377254) of the
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
| Static file, 1 KiB | rws | 96,670 | 0.61 | 1.65 | 16.9 | 33.5 |
| | Caddy | 18,592 | 2.73 | 13.49 | 154.4 | 66.1 |
| | nginx | 81,161 | 0.59 | 3.10 | 25.1 | 23.2 |
| Static file, 1 MiB | rws | 9,511 | 6.26 | 14.00 | 209.8 | 33.5 |
| | Caddy | 2,398 | 19.03 | 124.03 | 1,194.7 | 69.4 |
| | nginx | 9,767 | 6.42 | 14.05 | 216.2 | 23.4 |
| Reverse proxy, 1 KiB | rws | 34,260 | 1.79 | 3.89 | 44.6 | 35.5 |
| | Caddy | 13,402 | 4.16 | 14.17 | 190.6 | 66.7 |
| | nginx | 33,942 | 1.74 | 4.51 | 47.1 | 24.2 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 86,327 | 0.69 | 1.87 | 20.9 | 41.8 |
| | Caddy | 5,729 | 8.51 | 51.96 | 606.3 | 109.2 |
| | nginx | 1,802 | 35.26 | 71.94 | 2,063.6 | 27.3 |
| Static 1 KiB, access log on | rws | 88,643 | 0.66 | 1.81 | 20.2 | 41.6 |
| | Caddy | 15,964 | 2.97 | 18.12 | 191.7 | 65.6 |
| | nginx | 75,518 | 0.62 | 3.16 | 28.6 | 23.2 |
| Static 1 KiB, HTTPS | rws | 70,372 | 0.82 | 2.22 | 25.9 | 49.4 |
| | Caddy | 25,555 | 2.22 | 9.57 | 112.6 | 69.4 |
| | nginx | 63,752 | 0.75 | 3.75 | 33.5 | 41.7 |
| Static 1 KiB, HTTP/2 | rws | 66,141 | 0.90 | 2.44 | 27.4 | 63.7 |
| | Caddy | 13,604 | 4.04 | 14.20 | 216.0 | 66.9 |
| | nginx | 51,416 | 0.84 | 4.46 | 37.9 | 42.2 |
| Static 1 MiB, HTTPS | rws | 1,602 | 35.23 | 49.34 | 923.7 | 65.7 |
| | Caddy | 1,460 | 38.13 | 108.27 | 1,483.6 | 68.0 |
| | nginx | 1,311 | 45.19 | 101.34 | 1,639.9 | 42.3 |

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

- **Against Caddy:** `rws` serves 4.0× (large) to 5.2× (small) as many file
  requests, 2.6× as many proxied and 4.9× as many HTTP/2 requests per
  second, using a quarter to a ninth of the CPU per request, with a p99
  three to nine times lower and about half the memory.
- **Against nginx:** less CPU per request in every scenario — small files
  −33%, large files −3%, proxying −5%, HTTP/2 −28%, HTTPS −23%, large files
  over HTTPS −44%, with an access log −29%, with gzip a hundredth — more
  requests per second in all but large plain files (−3%), and the lower p99
  everywhere. This run used an AMD EPYC runner. On the fastest (Intel)
  runner type nginx keeps 6–8% more plain-HTTP requests per second and
  about the same CPU for large files and proxying (run 40209996636);
  `RWS_CONFIG_RUNTIME=per-core` narrows that (run 40242045036). The load
  generator shares the runner's 4 CPUs, so the CPU/request column is the
  comparison to read.
- **Memory:** release builds allocate with mimalloc, which costs about 5%
  less CPU per request than the system allocator but holds more memory —
  34–66 MiB here, against nginx's 23–42. Built without the `mimalloc`
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
compare worker counts, and **base_runtime** (`RWS_BASE_RUNTIME`) does the same
for `RWS_CONFIG_RUNTIME` (shared or per-core). (Fewer workers than CPUs raises the measured
requests per second only because the load generator shares the runner's
CPUs; on a dedicated server one worker per core is right.)
