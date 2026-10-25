---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from one run (40199049299) of the
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
| Static file, 1 KiB | rws | 95,728 | 0.62 | 1.62 | 16.6 | 34.1 |
| | Caddy | 18,897 | 2.70 | 13.12 | 151.3 | 67.8 |
| | nginx | 82,218 | 0.65 | 2.98 | 25.0 | 23.4 |
| Static file, 1 MiB | rws | 9,508 | 6.24 | 14.37 | 210.7 | 34.1 |
| | Caddy | 2,419 | 18.43 | 124.06 | 1,176.9 | 72.4 |
| | nginx | 9,874 | 6.22 | 14.54 | 213.2 | 23.4 |
| Reverse proxy, 1 KiB | rws | 34,732 | 1.77 | 3.83 | 44.4 | 38.1 |
| | Caddy | 13,774 | 4.07 | 13.81 | 186.7 | 68.0 |
| | nginx | 34,424 | 1.72 | 4.39 | 46.6 | 24.2 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 86,864 | 0.69 | 1.86 | 20.7 | 38.2 |
| | Caddy | 5,764 | 8.42 | 51.93 | 601.5 | 117.4 |
| | nginx | 1,805 | 35.55 | 58.57 | 2,065.4 | 27.3 |
| Static 1 KiB, access log on | rws | 86,809 | 0.68 | 1.86 | 20.7 | 44.1 |
| | Caddy | 16,321 | 2.86 | 18.04 | 187.6 | 66.6 |
| | nginx | 75,981 | 0.59 | 2.85 | 28.2 | 23.2 |
| Static 1 KiB, HTTPS | rws | 71,533 | 0.82 | 2.23 | 25.6 | 50.3 |
| | Caddy | 25,198 | 2.22 | 10.18 | 110.2 | 125.0 |
| | nginx | 63,530 | 0.94 | 2.94 | 33.3 | 42.6 |
| Static 1 KiB, HTTP/2 | rws | 45,686 | 1.21 | 3.45 | 44.8 | 58.2 |
| | Caddy | 13,971 | 3.94 | 13.82 | 211.0 | 68.2 |
| | nginx | 54,451 | 1.06 | 3.82 | 36.4 | 43.4 |
| Static 1 MiB, HTTPS | rws | 1,331 | 42.50 | 98.90 | 1,678.1 | 64.2 |
| | Caddy | 1,466 | 38.32 | 114.32 | 1,461.8 | 69.2 |
| | nginx | 1,338 | 43.99 | 98.91 | 1,578.6 | 43.5 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is the
scenario where nginx clearly leads: in a profile of both, the `h2` crate's
framing, stream bookkeeping and HPACK cost `rws` more per request than all
of nginx's own code, and HPACK work grows with the number of headers —
`rws` sends about 17 (security headers, nanosecond timestamps) to nginx's 7.
Its TLS (rustls) is cheaper than nginx's OpenSSL.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.9× (large) to 5.1× (small) as many file
  requests and 2.5× as many proxied requests per second, using a quarter to
  a ninth of the CPU per request, with a p99 three to nine times lower and
  about half the memory.
- **Against nginx:** less CPU per request on small files (−34%, 16% more
  requests per second), large files (−1%), proxying (−5%), with an access
  log (−27%), over HTTPS (−23%) and with gzip (a hundredth); more over
  HTTP/2 (+23%) and for large files over TLS (+6%). `rws` has the lower p99
  in every scenario (one tie). The load generator shares the runner's 4
  CPUs, so the CPU/request column is the comparison to read.
- **Memory:** release builds allocate with mimalloc, which costs about 5%
  less CPU per request than the system allocator but holds more memory —
  34–64 MiB here, against nginx's 23–44. Built without the `mimalloc`
  feature, `rws` peaks at 9–17 MiB.
- The gap depends on the runner's hardware: on the fastest runner type
  GitHub assigns, an earlier build (before mimalloc and the allocation
  work) needed 31% more CPU than nginx for proxying while matching it on
  files. Same-run A/Bs (`base_ref`, below) are how changes are judged.
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
