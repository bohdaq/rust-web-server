---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40182011441) of the
[Benchmark workflow](https://github.com/bohdaq/rust-web-server/blob/main/.github/workflows/bench.yml)
on a 4-CPU GitHub Actions runner (Linux x86_64), with the load generator on the
same machine.

**Load:** [`oha`](https://github.com/hatoo/oha), 64 keep-alive connections,
20 seconds per scenario after a 2-second warm-up, HTTP/1.1, no compression
(`--disable-compression`: the Caddy and nginx configs don't compress, and
`rws` compresses text by default), access logs off everywhere. `rws` is the
release build (`--features "acme auth"`), Caddy 2.11.7, nginx from Ubuntu with
`sendfile`, `open_file_cache` and upstream keep-alive enabled. nginx's
`open_file_cache_valid 1s` trusts a cached file's size and modification time
for a second; `rws` runs with the equivalent `open_file_cache_strict = false`
(its default re-checks them with an `fstat` on every request, so an edit in
place is served at once).

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 167,778 | 0.36 | 0.84 | 8.1 | 9.1 |
| | Caddy | 41,223 | 1.11 | 6.37 | 68.9 | 66.6 |
| | nginx | 226,059 | 0.26 | 0.92 | 8.3 | 23.6 |
| Static file, 1 MiB | rws | 17,662 | 3.41 | 7.22 | 101.5 | 9.5 |
| | Caddy | 3,882 | 11.60 | 75.54 | 711.9 | 70.6 |
| | nginx | 18,759 | 3.28 | 7.88 | 103.4 | 23.7 |
| Reverse proxy, 1 KiB | rws | 78,266 | 0.78 | 1.71 | 23.4 | 12.1 |
| | Caddy | 29,445 | 1.79 | 7.22 | 93.7 | 65.7 |
| | nginx | 95,151 | 0.63 | 1.70 | 17.8 | 24.6 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 149,020 | 0.38 | 1.04 | 11.0 | 12.3 |
| | Caddy | 10,003 | 3.68 | 38.65 | 360.6 | 98.6 |
| | nginx | 2,737 | 19.42 | 45.63 | 1,404.0 | 27.8 |
| Static 1 KiB, access log on | rws | 159,008 | 0.36 | 1.00 | 10.0 | 9.7 |
| | Caddy | 35,808 | 1.08 | 8.77 | 85.3 | 66.8 |
| | nginx | 201,943 | 0.24 | 1.87 | 9.6 | 23.7 |
| Static 1 KiB, HTTPS | rws | 135,007 | 0.42 | 1.18 | 11.8 | 15.5 |
| | Caddy | 65,040 | 0.78 | 4.31 | 42.7 | 121.9 |
| | nginx | 180,986 | 0.18 | 2.58 | 10.5 | 43.3 |
| Static 1 KiB, HTTP/2 | rws | 94,317 | 0.60 | 1.66 | 19.8 | 17.5 |
| | Caddy | 26,634 | 2.06 | 6.84 | 111.5 | 67.7 |
| | nginx | 131,887 | 0.45 | 1.28 | 12.4 | 43.6 |
| Static 1 MiB, HTTPS | rws | 2,562 | 24.42 | 36.80 | 782.9 | 29.5 |
| | Caddy | 3,037 | 17.92 | 57.65 | 722.8 | 67.4 |
| | nginx | 3,042 | 19.34 | 41.98 | 636.4 | 43.7 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is the
scenario where nginx clearly leads: most of `rws`'s cost there is in the `h2`
crate's framing and HPACK, not in serving the file.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 4.1× (small) to 4.5× (large) as many file
  requests and 2.7× as many proxied requests per second, using an eighth to
  a quarter of the CPU per request, with a p99 four to ten times lower and
  about a seventh of the memory.
- **Against nginx:** about the same CPU per request on static files (2%
  less, small and large), 31% more on proxying, 4% more with an access log,
  12% more over HTTPS, 23% more for large files over TLS (no `sendfile`) and
  60% more over HTTP/2; with gzip, a hundredth. nginx serves 6–26% more
  requests per second in this run; `rws` has the lower p99 on static files
  and about a third of the memory. The load generator shares the runner's 4
  CPUs, so throughput depends on how the two share them too — the
  CPU/request column is the comparison to read.
- The gap depends on the runner's hardware: on the slower runner type
  GitHub assigned to other runs the same week, with the same code, `rws`
  needed 28% less CPU per request than nginx on small files and 2–4% more
  on proxying.
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
scenario. The job summary lists the hottest symbols, and the `bench-profile`
artifact has flat and call-tree reports plus `perf script` output for flame
graphs (`inferno-collapse-perf | inferno-flamegraph`). Locally, on Linux:

```bash
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release --bin rws --features "acme auth"
PERF_DIR=perf RWS=target/release/rws OHA=oha NGINX=nginx bench/run.sh
perf report -i perf/static-1k.data --no-children
```

Sampling slows `rws` down, so take throughput numbers from an unprofiled run.
