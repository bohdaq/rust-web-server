---
title: Benchmarks
description: rws compared with Caddy and nginx on static files and reverse proxying, and how to reproduce the numbers.
---

`rws` is measured against Caddy and nginx on the same machine, with the same
files and the same load. The numbers below come from the
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
| Static file, 1 KiB | rws | 125,681 | 0.47 | 1.24 | 13.9 | 8.6 |
| | Caddy | 27,752 | 1.89 | 8.52 | 103.7 | 67.7 |
| | nginx | 128,364 | 0.48 | 1.46 | 15.8 | 23.6 |
| Static file, 1 MiB | rws | 12,574 | 4.56 | 10.74 | 153.6 | 8.8 |
| | Caddy | 3,295 | 14.96 | 88.13 | 864.5 | 71.9 |
| | nginx | 13,112 | 4.47 | 11.08 | 150.5 | 23.7 |
| Reverse proxy, 1 KiB | rws | 49,330 | 1.22 | 2.93 | 33.2 | 13.5 |
| | Caddy | 20,928 | 2.65 | 9.37 | 122.1 | 69.8 |
| | nginx | 52,012 | 1.13 | 3.14 | 31.3 | 24.9 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

### Extended scenarios

The same run, with the extended scenarios (`EXTENDED=1`, see
[Reproduce](#reproduce)):

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| 26 KiB JS file, gzip | rws | 109,065 | 0.53 | 1.44 | 16.2 | 13.6 |
| | Caddy | 8,282 | 5.12 | 40.43 | 419.0 | 115.7 |
| | nginx | 2,030 | 20.84 | 98.46 | 1,860.1 | 28.1 |
| Static 1 KiB, access log on | rws | 111,760 | 0.51 | 1.45 | 16.0 | 9.8 |
| | Caddy | 23,304 | 2.07 | 11.76 | 130.7 | 64.8 |
| | nginx | 118,475 | 0.53 | 1.53 | 18.3 | 23.5 |
| Static 1 KiB, HTTPS | rws | 96,517 | 0.58 | 1.62 | 18.5 | 15.7 |
| | Caddy | 36,869 | 1.53 | 6.67 | 76.2 | 123.6 |
| | nginx | 102,849 | 0.46 | 2.66 | 20.2 | 43.1 |
| Static 1 KiB, HTTP/2 | rws | 53,579 | 1.10 | 3.01 | 43.2 | 18.5 |
| | Caddy | 20,169 | 2.76 | 9.19 | 144.8 | 68.9 |
| | nginx | 86,367 | 0.59 | 3.22 | 22.4 | 43.8 |
| Static 1 MiB, HTTPS | rws | 2,257 | 26.11 | 38.71 | 828.7 | 29.6 |
| | Caddy | 2,223 | 26.51 | 71.09 | 1,102.1 | 68.3 |
| | nginx | 2,407 | 24.29 | 62.78 | 906.5 | 44.9 |

`rws` compresses a static text file once per version and serves the cached
result; nginx and Caddy compress on every request, which is why the gzip row
differs by two orders of magnitude (level 6 everywhere). HTTP/2 is the
scenario where nginx clearly leads: `rws` runs each HTTP/2 request on its
own task, beside the connection's.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.8× (large) to 4.5× (small) as many file
  requests and 2.4× as many proxied requests per second, using a seventh to
  a quarter of the CPU per request, with a p99 three to eight times lower and
  about an eighth of the memory.
- **Against nginx:** within 2% on small files, 4% on large files and 5% on
  proxying. On small files `rws` needs 12% less CPU per request; on large
  files 2% and proxying 6% more. With gzip, an access log or HTTPS it needs
  less CPU per request than nginx; over HTTP/2, about twice as much. `rws` has the lower p99 in every scenario and
  about a third of the memory. The load generator shares the runner's 4
  CPUs, so the server that needs the least CPU per request wins there — the
  CPU/request column is the gap.
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
