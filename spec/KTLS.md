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

## Steps

| # | Step | Status |
|---|------|--------|
| 0 | Baseline benchmark: static files HTTPS vs HTTP | todo |
| 1 | Server H1: secret extraction, `KtlsStream`, sendfile over TLS, setting `tls.ktls` | todo |
| 2 | Control records (alert/close), KeyUpdate, startup probe, fallback, metrics | todo |
| 3 | Tests (loopback rustls client), CI, docs, re-benchmark | todo |
| 4a | H2 connections on kTLS | todo |
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

## Log

(filled in as steps land: commits, benchmarks, upstream links, blockers)
