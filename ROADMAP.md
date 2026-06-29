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

### HTTP/1.1 keep-alive (persistent connections)
`Server::process()` loops over requests on the same TCP stream. The `Connection` header controls persistence; HTTP/1.1 defaults to keep-alive, HTTP/1.0 defaults to close. A 30-second read timeout prevents stalled clients from holding a thread indefinitely.

### Response compression
`src/compression/mod.rs` — `apply_gzip()` checks `Accept-Encoding: gzip` and compresses text responses (HTML, CSS, JS, JSON, SVG, XML) using `flate2`. Adds `Content-Encoding: gzip` and `Vary: Accept-Encoding`. Wired into HTTP/1.1, HTTP/2, and HTTP/3 code paths.

### Large file streaming
`Response.stream_file: Option<String>` — when set, `Server::write_chunked_file()` streams the file in 64 KB chunks using `Transfer-Encoding: chunked` instead of buffering into RAM. `StaticResourceController` uses this path for files larger than 8 MB that are not range requests.

### HTTP → HTTPS redirect
`RWS_CONFIG_HTTP_REDIRECT_PORT` — when set to a port (e.g. `"80"`), `Server::run_redirect()` binds a plain-HTTP listener on that port and returns `301 Moved Permanently` to the HTTPS equivalent for every request. No-op when TLS is not configured.

### Cookie handling
`src/cookie/mod.rs` — `Cookie` (name/value pair), `CookieJar::parse()` (parses the `Cookie` request header), and `SetCookie` builder (produces `Set-Cookie` response header values with `Path`, `Domain`, `Max-Age`, `Secure`, `HttpOnly`, `SameSite` attributes).

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
| 1 | HTTP/1.1 keep-alive | Medium | High | ✅ Done |
| 2 | Response compression | Medium | High | ✅ Done |
| 3 | Large file streaming | High | High | ✅ Done |
| 4 | HTTP → HTTPS redirect | Low | Medium | ✅ Done |
| 5 | Cookies | High | Low | ✅ Done |
