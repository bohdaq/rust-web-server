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

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 49,405 | 1.10 | 3.37 | 8.5 |
| | Caddy | 18,647 | 2.75 | 13.41 | 68.2 |
| | nginx | 82,035 | 0.54 | 3.01 | 23.4 |
| Static file, 1 MiB | rws | 8,926 | 6.83 | 14.81 | 8.8 |
| | Caddy | 2,420 | 18.82 | 123.23 | 70.6 |
| | nginx | 10,117 | 5.97 | 14.63 | 23.5 |
| Reverse proxy, 1 KiB | rws | 27,790 | 2.19 | 4.98 | 12.8 |
| | Caddy | 13,418 | 4.16 | 14.34 | 66.5 |
| | nginx | 34,176 | 1.73 | 4.42 | 24.5 |

Every request in every scenario returned `200 OK`.

On a 10-core Mac (same tools and configuration, `oha` on the same machine):
1 KiB static — rws 107k, nginx 100k; 1 MiB static — 9.8k vs 9.4k; proxy —
49.0k vs 48.9k requests per second.

## Reading the numbers

- **Against Caddy:** `rws` serves 2.6× (small) to 3.7× (large) as many file
  requests and 2.1× as many proxied requests per second, with a p99 three to
  eight times lower and about an eighth of the memory.
- **Against nginx:** on the 4-CPU runner nginx leads by 12% on large files,
  19% on proxying and 40% on small files, with a similar p99; with more cores
  (the 10-core run above) the two are even. The load generator shares the
  runner's 4 CPUs, so the server that needs the least CPU per request wins
  most there.
- Shared CI runners are noisy: absolute numbers vary by a factor of two
  between runs. Compare servers within one run, not numbers across runs.
- Compression matters: with `Accept-Encoding: gzip` sent and only `rws`
  compressing, the comparison measures `gzip`, not the servers. Compare like
  for like.

## Why it's fast

- **Open files are reused** (an open-file cache like nginx's): a request for
  a hot file costs one `fstat` and one positional read.
- **Quick requests run on the event loop.** Static files, health checks and
  metrics skip the hand-off to a blocking thread pool, as in nginx. Handlers
  that may block (your routes) still get a pool thread, so a slow handler
  can't stall other connections. See
  [`runs_inline`](/building-apps/middleware/#running-on-the-event-loop-runs_inline).
- **Proxying holds no thread.** Proxy routes to plain-HTTP backends wait for
  the backend on the connection's task, over pooled keep-alive connections.
- **One idle timer per connection**, pushed back on each read without a lock.
- **`sendfile` for downloads** over plain HTTP on Linux and macOS: the kernel
  copies file pages straight to the socket.
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
