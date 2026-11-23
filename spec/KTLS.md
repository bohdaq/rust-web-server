# Kernel TLS (kTLS)

Goal: after the rustls handshake, hand the session keys to the Linux kernel
(`TCP_ULP "tls"`), so the kernel encrypts and decrypts records. Static files
over HTTPS then go out with `sendfile(2)` like plain HTTP. Opt-in, Linux
only, falls back to rustls whenever anything is unsupported.

Branch `ktls` (worktree `~/git/rws-ktls`), pushed, not merged, not released.

## Decisions (user, 2026-11-23)

- Branch + push, no merge, no release, no VM install.
- Own module on `libc` (no `ktls` crate).
- Scope: everything — server H1 + sendfile, control messages, H2,
  rws-proxy (splice), rws HTTP client. HITOP repo untouched (note only).
- Upstream: may open GitHub issues on rustls / nix / rustix, only for
  reproduced bugs or gaps, duplicate-searched, at most ~3. No kernel mail.
- Setting off by default. `fips` builds never use kTLS.
- Setting is a boolean, `tls_ktls = true` (repo convention), not `on|off`.

## Steps

| # | Step | Status |
|---|------|--------|
| 0 | Baseline benchmark: static files HTTPS vs HTTP | done |
| 1 | Server H1: secret extraction, `KtlsStream`, sendfile over TLS, setting `tls_ktls` | done |
| 2 | Control records (alert/close), KeyUpdate, startup probe, fallback, metrics | done |
| 3 | Tests (loopback rustls client), CI, docs, re-benchmark | done (CI: branch push) |
| 4a | H2 connections on kTLS | done with step 1 (same `KtlsStream`); h2spec run pending |
| 4b | rws-proxy: splice upstream → kTLS client socket | todo |
| 4c | rws HTTP client: kTLS after client handshake | todo |
| 5 | Upstream issues + HITOP note | todo |

## Design notes

- Hook: `serve_tls_connection` (`crates/rws-server/src/server/mod.rs`) after
  `Ok(tls_stream)`; `SendfileSocket for KtlsStream` returns the socket so
  `http1_async` sendfile path is reused.
- Ciphers: TLS 1.2/1.3 AES-128-GCM, AES-256-GCM, CHACHA20-POLY1305.
- Before extraction: flush rustls (session tickets), refuse if plaintext or
  ciphertext is buffered unread.
- RX: `recvmsg` with `TLS_GET_RECORD_TYPE` cmsg; non-data record → alert
  handling / close.

## Findings

- **tokio-rustls reads past the handshake.** Its handshake loop keeps
  reading while rustls `wants_read()`, so a TLS 1.3 client's first request
  (sent with its Finished) is already decrypted inside rustls when
  `accept` returns — `Corked` alone doesn't prevent it. `upgrade` drains
  that plaintext into `KtlsStream`'s prefix; rustls' read sequence number
  already counts those records, so the kernel continues correctly. Test:
  `a_request_sent_with_the_finished_message_is_kept` (holds the client's
  writes so Finished + request arrive together).
- **rustls' buffered `ServerConnection` has no `dangerous_into_kernel_connection`.**
  Only the unbuffered API returns a `KernelConnection` (which can follow
  TLS 1.3 KeyUpdates). With tokio-rustls (buffered) we get
  `dangerous_extract_secrets` only, so a KeyUpdate closes the connection.
  Upstream candidate (step 5).
- **`dangerous_extract_secrets` doesn't check for buffered received data**
  (only `sendable_tls`); a caller can silently lose decrypted plaintext or
  a half-read record. Upstream candidate (doc or check).
- **Unrelated bug found on the way: HTTP/2 + HTTP/3 static files > 256 KiB
  were broken since 18.17.0** (prepared 501 problem body sent beside the
  file → duplicate content-type/content-length; curl and browsers reject).
  Fixed in its own commit (`fix(h2,h3): a streamed file replaces a
  prepared body`) — worth releasing on its own.

## Benchmarks

Machine: Intel Core i7-7700 (4C/8T, AES-NI + AVX2, no VAES), Linux
6.8.0-60, oha 1.16 on the same box (32 connections, keep-alive, HTTP/1.1,
no compression), release build. `bench/ktls.sh`. CPU ms/req = server
utime+stime / 2xx responses. Numbers vary ±20% run to run (shared box).

| mode | 1k req/s | 1k CPU ms | 64k CPU ms | 1m CPU ms | 16m CPU ms |
|---|---:|---:|---:|---:|---:|
| http | 105–117k | 0.035 | 0.058 | 0.23 | 2.1–2.3 |
| rustls | 75–104k | 0.042 | 0.103–0.108 | 1.09–1.11 | 14.5–14.8 |
| ktls | 45–76k | 0.047 | 0.141–0.146 | 1.06 | 14.2 |

Profile (bpftrace, kernel frames, 1m over kTLS): 39% of samples in
`key_256_enc_update4` (the kernel's AES-GCM, `generic-gcm-aesni`; clients
prefer AES-256-GCM), the rws process itself ~5%. Over rustls: 49% user
(aws-lc + copies), 12% `rep_movs_alternative`. So on this CPU/kernel the
kernel's AES-GCM costs what the saved copies do; `TLS_TX_ZEROCOPY_RO` and
`TLS_RX_EXPECT_NO_PAD` changed nothing measurable and were dropped. Linux
6.11 rewrote x86 AES-GCM (Eric Biggers) — worth re-running there, and on
a NIC with TLS offload. Small responses are ~15% slower with kTLS
(`recvmsg` + per-record kernel crypto setup).

## Log

- 2026-11-23: steps 0–3 + 4a on branch `ktls`. Commits: fix(h2,h3)
  streamed file; feat(tls) kTLS.
