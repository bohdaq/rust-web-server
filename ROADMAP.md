[Read Me](README.md) > Roadmap

# Roadmap

## Done

### HTTP/3 over QUIC
Implemented via `quinn 0.11`, `h3 0.0.8`, `h3-quinn 0.0.10`. A second UDP listener binds on the same port as TCP. ALPN negotiates `h3` on the QUIC side and `h2`/`http1.1` on the TLS TCP side. HTTP/1.1 TLS responses advertise availability via `Alt-Svc: h3=":PORT"`.

### WebAssembly MIME type
`application/wasm` added to `MimeType`. `.wasm` files now served with the correct type so browsers can compile them.

### Security headers
Added to all responses via `get_header_list()`:
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Permissions-Policy: geolocation=(), microphone=(), camera=()`
- `Content-Security-Policy: default-src 'self'` (overridable via `RWS_CONFIG_CSP` env var)

Added only to HTTPS responses (per RFC 6797) in `process_h1_tls`, `h2_handler`, and `h3_handler`:
- `Strict-Transport-Security: max-age=31536000; includeSubDomains`

### ETag and 304 Not Modified
`StaticResourceController` computes an ETag from file mtime + size on every file response. If the request carries a matching `If-None-Match` header (or `*`), the server returns 304 with an empty body and no file I/O.

### Connection and read timeouts
Plain HTTP/1.1 TCP connections now have a 30-second `set_read_timeout`. Stalled clients can no longer hold a thread indefinitely.

### Graceful shutdown
The async accept loops (`run_tls` and `run_quic`) use `tokio::select!` with `tokio::signal::ctrl_c()`. Pressing Ctrl+C (or sending SIGINT) stops accepting new connections and exits cleanly.

### Standard access log format (Combined Log Format)
`Log::combined()` produces standard CLF lines:
```
127.0.0.1 - - [29/Jun/2026:14:23:05 +0000] "GET /index.html HTTP/1.1" 200 1234
```
All three server code paths (HTTP/1.1, HTTP/2, HTTP/3) use this format. Compatible with GoAccess, AWStats, and similar log analysis tools.

### Custom error pages
`NotFoundController` already checks for a `404.html` file in the working directory and serves it when present, falling back to an embedded default.

---

## Priority 1 — Performance

### 1. HTTP/1.1 keep-alive (persistent connections)
The server reads one request per TCP connection then closes. `Server::process()` needs to loop over requests on the same stream until `Connection: close` is received or the client disconnects. Without this, every asset on a page requires a new TCP handshake.

### 2. Response compression
`Content-Encoding` header constant exists, never used. Check `Accept-Encoding` on each request, compress text responses (HTML, CSS, JS, JSON, SVG, XML) with gzip/brotli/zstd, add `Content-Encoding` and `Vary: Accept-Encoding`.

### 3. Large file streaming
Every file is fully read into `Vec<u8>` before the first byte is sent. Files larger than available memory will crash the process. Needs chunked or streaming send via `Transfer-Encoding: chunked` (HTTP/1.1) or the built-in stream framing in HTTP/2 and HTTP/3.

---

## Priority 2 — Security

### 4. HTTP → HTTPS redirect
No plain-HTTP listener when running with a certificate. Need an optional second bind address (e.g. port 80) that returns `301 Moved Permanently` to the HTTPS equivalent.

---

## Priority 3 — Developer experience

### 5. Cookie handling
`Set-Cookie` constant defined, no implementation. A configurable signed cookie would enable basic session tracking without a third-party dependency.

---

## Summary

| # | Feature | Effort | Impact | Status |
|---|---------|--------|--------|--------|
| — | HTTP/3 over QUIC | High | High | ✅ Done |
| — | WebAssembly MIME type | Trivial | High | ✅ Done |
| — | Security headers (HSTS, CSP, Referrer-Policy, Permissions-Policy) | Low | High | ✅ Done |
| — | ETag / 304 Not Modified | Medium | High | ✅ Done |
| — | Read timeout | Low | Medium | ✅ Done |
| — | Graceful shutdown | Low | Medium | ✅ Done |
| — | Combined Log Format | Low | Low | ✅ Done |
| — | Custom error pages | Low | Low | ✅ Done |
| 1 | HTTP/1.1 keep-alive | Medium | High | Pending |
| 2 | Response compression | Medium | High | Pending |
| 3 | Large file streaming | High | High | Pending |
| 4 | HTTP → HTTPS redirect | Low | Medium | Pending |
| 5 | Cookies | High | Low | Pending |
