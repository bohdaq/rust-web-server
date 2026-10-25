---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40195331932) of the
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
| Static file, 1 KiB | rws | 94,384 | 0.63 | 1.67 | 16.8 | 34.1 |
| | Caddy | 18,503 | 2.76 | 13.61 | 154.7 | 69.5 |
| | nginx | 82,258 | 0.54 | 3.23 | 23.8 | 23.3 |
| Static file, 1 MiB | rws | 9,128 | 6.44 | 14.48 | 211.8 | 34.1 |
| | Caddy | 2,372 | 19.24 | 124.87 | 1,210.2 | 72.4 |
| | nginx | 9,880 | 6.09 | 14.48 | 208.6 | 23.4 |
| Reverse proxy, 1 KiB | rws | 34,378 | 1.79 | 3.93 | 44.8 | 38.1 |
| | Caddy | 13,330 | 4.18 | 14.41 | 192.3 | 67.8 |
| | nginx | 34,635 | 1.71 | 4.37 | 46.3 | 24.5 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 84,366 | 0.71 | 1.92 | 21.4 | 42.3 |
| | Caddy | 5,723 | 8.52 | 50.61 | 605.9 | 105.6 |
| | nginx | 1,801 | 33.43 | 59.60 | 2,064.5 | 27.7 |
| Static 1 KiB, access log on | rws | 88,337 | 0.67 | 1.84 | 20.2 | 43.9 |
| | Caddy | 15,859 | 2.97 | 18.23 | 192.6 | 65.1 |
| | nginx | 76,527 | 0.64 | 3.09 | 28.1 | 23.2 |
| Static 1 KiB, HTTPS | rws | 72,602 | 0.80 | 2.24 | 25.0 | 50.1 |
| | Caddy | 25,715 | 2.20 | 9.55 | 111.8 | 70.7 |
| | nginx | 63,194 | 0.78 | 3.29 | 32.8 | 42.0 |
| Static 1 KiB, HTTP/2 | rws | 43,357 | 1.27 | 3.59 | 48.0 | 62.2 |
| | Caddy | 13,481 | 4.11 | 14.19 | 217.3 | 68.4 |
| | nginx | 49,325 | 0.79 | 4.43 | 38.1 | 42.2 |
| Static 1 MiB, HTTPS | rws | 1,224 | 44.30 | 104.24 | 1,943.5 | 92.6 |
| | Caddy | 1,465 | 38.28 | 120.05 | 1,446.8 | 68.8 |
| | nginx | 1,278 | 45.12 | 122.01 | 1,618.8 | 42.8 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is the
scenario where nginx clearly leads: most of `rws`'s cost there is in the `h2`
crate's framing and HPACK, not in serving the file.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.8× (large) to 5.1× (small) as many file
  requests and 2.6× as many proxied requests per second, using a quarter to
  a ninth of the CPU per request, with a p99 four to nine times lower and
  about half the memory.
- **Against nginx:** 29% less CPU per request on small files (15% more
  requests per second), 3% less proxying, 28% less with an access log, 24%
  less over HTTPS, a hundredth with gzip; about the same on large files
  (+1.5%); more over HTTP/2 (+26%) and for large files over TLS (+20%,
  where `sendfile` doesn't apply). `rws` has the lower p99 in every scenario
  but one tie. The load generator shares the runner's 4 CPUs, so the
  CPU/request column is the comparison to read.
- **Memory:** release builds allocate with mimalloc, which costs about 5%
  less CPU per request than the system allocator but holds more memory —
  34–62 MiB here, up to 93 MiB for large TLS downloads, against nginx's
  23–43. Built without the `mimalloc` feature, `rws` peaks at 9–17 MiB
  (30 for large TLS downloads).
- The gap depends on the runner's hardware: on the fastest runner type
  GitHub assigns, an earlier build (before mimalloc) needed 31% more CPU
  than nginx for proxying while matching it on files. Same-run A/Bs
  (`base_ref`, below) are how changes are judged.
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
