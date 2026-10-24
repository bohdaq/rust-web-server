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
`sendfile`, `open_file_cache` and upstream keep-alive enabled.

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | CPU/request (µs) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 70,227 | 0.78 | 2.36 | 28.0 | 8.5 |
| | Caddy | 19,084 | 2.68 | 13.14 | 150.3 | 68.4 |
| | nginx | 83,591 | 0.51 | 3.06 | 23.8 | 23.2 |
| Static file, 1 MiB | rws | 9,567 | 6.34 | 13.84 | 218.8 | 8.7 |
| | Caddy | 2,444 | 19.01 | 119.57 | 1,181.5 | 74.5 |
| | nginx | 10,255 | 5.93 | 14.37 | 204.6 | 23.2 |
| Reverse proxy, 1 KiB | rws | 31,591 | 1.87 | 4.55 | 57.9 | 13.1 |
| | Caddy | 13,958 | 4.01 | 13.52 | 185.4 | 68.0 |
| | nginx | 34,990 | 1.68 | 4.38 | 45.5 | 24.2 |

CPU/request is the server's user + system CPU time over the run (including
worker processes) divided by the requests it served.

Every request in every scenario returned `200 OK`.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 3.7× (small) to 3.9× (large) as many file
  requests and 2.3× as many proxied requests per second, using a fifth of the
  CPU per request, with a p99 three to nine times lower and about an eighth
  of the memory.
- **Against nginx:** on the 4-CPU runner nginx leads by 7% on large files,
  11% on proxying and 19% on small files, with a similar p99; with more cores
  (the 10-core run above) the two are even. The load generator shares the
  runner's 4 CPUs, so the server that needs the least CPU per request wins
  there — the CPU/request column is the gap (7–27% more for `rws`).
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
- **Proxying holds no thread.** Proxy routes to plain-HTTP backends wait for
  the backend on the connection's task, over pooled keep-alive connections.
- **One idle timer per connection**, pushed back on each read without a lock.
- **`sendfile` for downloads** over plain HTTP on Linux and macOS: the kernel
  copies file pages straight to the socket, with the response head in the
  same segment (`TCP_CORK` on Linux, `sendfile`'s header vector on macOS).
- **Configuration is a cached snapshot**, not read from the environment on
  every request; a reload replaces it.

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
