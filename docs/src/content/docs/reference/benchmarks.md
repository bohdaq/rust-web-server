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
`sendfile` and upstream keep-alive enabled.

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 52,601 | 1.22 | 3.23 | 8.4 |
| | Caddy | 28,367 | 1.79 | 8.67 | 68.1 |
| | nginx | 109,021 | 0.39 | 2.40 | 23.6 |
| Static file, 1 MiB | rws | 11,362 | 5.18 | 11.39 | 8.8 |
| | Caddy | 3,279 | 14.99 | 88.70 | 70.6 |
| | nginx | 12,704 | 4.72 | 11.15 | 23.7 |
| Reverse proxy, 1 KiB | rws | 29,436 | 2.10 | 4.31 | 15.5 |
| | Caddy | 21,601 | 2.60 | 8.82 | 66.5 |
| | nginx | 51,591 | 1.14 | 3.05 | 24.6 |

Every request in every scenario returned `200 OK`.

## Reading the numbers

- **Against Caddy:** `rws` serves 1.9× (small) to 3.5× (large) as many
  file requests and 36% more proxied requests per second, with a lower p99
  and about a sixth of the memory.
- **Against nginx:** large files are within 11% — both use `sendfile`, so
  the kernel does the copying. On small files and proxying nginx is about
  twice as fast: its event loop does all the work on one thread, while rws
  still crosses threads for the parts that may block (proxying).
- Shared CI runners are noisy: absolute numbers vary by a factor of two
  between runs. Compare servers within one run, not numbers across runs.
- Compression matters: with `Accept-Encoding: gzip` sent and only `rws`
  compressing, the comparison measures `gzip`, not the servers. Compare like
  for like.

## Why it's fast

- **One `open` and one `fstat` per static file**; size and modification time
  come from the open file, and a small file is read from that same handle.
- **Quick requests run on the event loop.** Static files, health checks and
  metrics skip the hand-off to a blocking thread pool, as in nginx. Handlers
  that may block (your routes, proxying) still get a pool thread, so a slow
  backend can't stall other connections. See
  [`runs_inline`](/building-apps/middleware/#running-on-the-event-loop-runs_inline).
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
