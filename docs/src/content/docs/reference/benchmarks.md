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
| Static file, 1 KiB | rws | 23,418 | 2.62 | 5.81 | 20.2 |
| | Caddy | 18,755 | 2.71 | 13.38 | 67.1 |
| | nginx | 70,904 | 0.64 | 2.93 | 23.2 |
| Static file, 1 MiB | rws | 2,882 | 21.86 | 41.16 | 47.4 |
| | Caddy | 2,382 | 18.89 | 126.97 | 69.4 |
| | nginx | 9,577 | 6.16 | 14.98 | 23.4 |
| Reverse proxy, 1 KiB | rws | 19,315 | 3.20 | 6.45 | 33.0 |
| | Caddy | 13,634 | 4.10 | 14.05 | 66.1 |
| | nginx | 33,324 | 1.79 | 4.61 | 24.4 |

Every request in every scenario returned `200 OK`.

## Reading the numbers

- **Against Caddy:** `rws` serves about 20–25% more static-file requests and
  40% more proxied requests per second, with a p99 two to three times lower
  and a third of the memory.
- **nginx** is two to three times faster than both. It's the reference point
  for what the hardware can do, not the project's target: `rws` competes with
  Caddy on what you get from one binary and one config file.
- Shared CI runners are noisy: absolute numbers vary by a factor of two
  between runs. Compare servers within one run, not numbers across runs.
- Compression matters: with `Accept-Encoding: gzip` sent and only `rws`
  compressing, the comparison measures `gzip`, not the servers. Compare like
  for like.

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
