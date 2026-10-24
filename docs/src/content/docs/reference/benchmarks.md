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
20 seconds per scenario after a 2-second warm-up, HTTP/1.1, access logs off
everywhere. `rws` is the release build (`--features "acme auth"`), Caddy 2.11.7,
nginx from Ubuntu with `sendfile` and upstream keep-alive enabled.

| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| Static file, 1 KiB | rws | 45,137 | 1.35 | 3.11 | 24.9 |
| | Caddy | 39,319 | 1.14 | 7.10 | 66.7 |
| | nginx | 140,750 | 0.39 | 2.41 | 23.6 |
| Static file, 1 MiB | rws | 4,901 | 12.87 | 25.22 | 54.8 |
| | Caddy | 3,951 | 11.65 | 76.57 | 70.7 |
| | nginx | 15,990 | 3.72 | 9.76 | 23.6 |
| Reverse proxy, 1 KiB | rws | 25,412 | 2.41 | 5.20 | 42.1 |
| | Caddy | 27,819 | 1.85 | 8.24 | 68.3 |
| | nginx | 65,618 | 0.87 | 2.75 | 24.5 |

Every request in every scenario returned `200 OK`.

## Reading the numbers

- **Static files:** `rws` serves more requests per second than Caddy, with a
  p99 two to three times lower and less memory.
- **Reverse proxy:** `rws` reaches about 90% of Caddy's throughput with a lower
  p99. Its upstream path is synchronous (each proxied request holds a
  blocking-pool thread for the round trip); an async path is
  [tracked here](https://github.com/bohdaq/rust-web-server/issues/30).
- **nginx** is about three times faster than both. It's the reference point for
  what the hardware can do, not the project's target: `rws` competes with
  Caddy on what you get from one binary and one config file.
- Shared CI runners are noisy. Compare servers within one run, not numbers
  across runs.

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
