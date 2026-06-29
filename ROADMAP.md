[Read Me](README.md) > Roadmap

# Roadmap

## Priority 1 — Performance

### 1. HTTP/1.1 keep-alive (persistent connections)
The server reads one request per TCP connection then closes. `Server::process()` needs to loop over requests on the same stream until `Connection: close` is received or the client disconnects. Without this, every asset on a page requires a new TCP handshake.

### 2. Response compression
`Content-Encoding` header constant exists, never used. Check `Accept-Encoding` on each request, compress text responses (HTML, CSS, JS, JSON, SVG, XML) with gzip/brotli/zstd, add `Content-Encoding` and `Vary: Accept-Encoding`.

### 3. ETag and conditional requests (304 Not Modified)
`ETag`, `Last-Modified`, `If-None-Match`, `If-Modified-Since` constants all exist. `304 Not Modified` status code exists. None of it is wired up — every request reads and sends the full file. Fix: compute ETag (mtime + size), store in response, compare against `If-None-Match` / `If-Modified-Since` on the next request and return 304 with empty body.

### 4. Large file streaming
Every file is fully read into `Vec<u8>` before the first byte is sent. Files larger than available memory will crash the process. Needs chunked or streaming send via `Transfer-Encoding: chunked` (HTTP/1.1) or the built-in stream framing in HTTP/2 and HTTP/3.

---

## Priority 2 — Security

### 5. HSTS (Strict-Transport-Security)
`_STRICT_TRANSPORT_SECURITY` constant exists, never sent. Must be added to all HTTPS responses. Recommended value: `max-age=31536000; includeSubDomains`.

### 6. Content-Security-Policy
`_CONTENT_SECURITY_POLICY` constant exists, never sent. Should be configurable (env var / config file key) and added to HTML responses by default.

### 7. Referrer-Policy and Permissions-Policy
Both constants defined, never sent. Sensible defaults: `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: geolocation=(), microphone=(), camera=()`.

### 8. HTTP → HTTPS redirect
No plain-HTTP listener when running with a certificate. Need an optional second bind address (e.g. port 80) that returns `301 Moved Permanently` to the HTTPS equivalent.

---

## Priority 3 — Correctness

### 9. WebAssembly MIME type missing
`application/wasm` is not in `MimeType`. `.wasm` files are served with the wrong type. Browsers refuse to compile Wasm unless the MIME type is exactly `application/wasm`.

### 10. Connection and read timeouts
No timeout on TCP reads. A slow or stalled client holds a thread indefinitely. Needs a configurable read timeout and idle connection timeout.

### 11. Graceful shutdown
`Server::run()` loops forever with no signal handling. `tokio::signal` is already a dependency. Hook up `SIGTERM` / `Ctrl+C` to stop accepting new connections and wait for in-flight requests to finish.

---

## Priority 4 — Developer experience

### 12. Custom error pages
404 and 500 responses serve hardcoded HTML. Should check for `404.html` / `500.html` in the working directory and serve those if present.

### 13. Standard access log format
Logs use a custom format. Tools like GoAccess and AWStats expect Combined Log Format (`127.0.0.1 - - [date] "GET /path HTTP/2" 200 1234`). Should be an option alongside the current verbose format.

### 14. Cookie handling
`Set-Cookie` constant defined, no implementation. A configurable signed cookie would enable basic session tracking without a third-party dependency.

---

## Summary

| # | Feature | Effort | Impact |
|---|---------|--------|--------|
| 1 | HTTP/1.1 keep-alive | Medium | High |
| 2 | Response compression | Medium | High |
| 3 | ETag / 304 | Medium | High |
| 4 | File streaming | High | High |
| 5 | HSTS | Low | High |
| 6 | CSP | Low | Medium |
| 7 | Referrer-Policy / Permissions-Policy | Low | Medium |
| 8 | HTTP → HTTPS redirect | Low | Medium |
| 9 | Wasm MIME type | Trivial | High |
| 10 | Timeouts | Low | Medium |
| 11 | Graceful shutdown | Low | Medium |
| 12 | Custom error pages | Low | Low |
| 13 | Standard access log | Low | Low |
| 14 | Cookies | High | Low |
