# Changelog

## 17.113.1 — security patch

Fixes from the 2026-10-25 security review. Upgrade from 17.113.0.

### Security

- **Proxy: interim responses (#48).** A `1xx` from an upstream (`100 Continue`, `103 Early Hints`) was taken as the final response; the real response stayed in the pooled connection and was delivered to the next request on it — possibly another client's. Both forwarders now read past interim responses (at most 16; `101 Switching Protocols` is still final).
- **Static files stay inside the web root (#43).** A request path with `..` segments (`/../../etc/passwd`) was resolved against the working directory and could read any file the process could; such paths now get `404`, and a symlink inside the root that points outside it is not followed. Dot-files and dot-directories (`.git/`, `.env`) are no longer served; set `RWS_CONFIG_SERVE_DOTFILES=true` to serve them again (`.well-known/` is always served).
- **Byte-range limits (#46).** Each range of a multi-range request was read into memory separately: `Range: bytes=0-,0-,0-,…` (thousands of ranges in a 64 KiB head) made one request hold thousands of copies of a file. Overlapping ranges are now merged, and more than 16 ranges are answered with the whole file as one range (RFC 9110 §14.2).
- **No CR/LF in header lines (#47).** The HTTP/1.1 writers copied header names, values and the reason phrase verbatim, so a value carrying user input (a redirect target, an upstream header) could inject headers or a second response; CR and LF now become spaces. The mailer does the same for `Subject`, `To`, `Cc`, `Reply-To` and `From`, and refuses an address containing CR, LF, `<` or `>` (it went into `MAIL FROM` / `RCPT TO` as is). HTTP/2 and HTTP/3 already refused such values.
- **CacheLayer: never cache private responses (#49).** The cache is keyed by URL, so a response built from one user's `Cookie` or `Authorization` was served to the next visitor, a `Set-Cookie` response was replayed to everyone, a `Range` response was served for the whole resource, and a streamed response was replayed with an empty body. Requests with `Authorization`, `Cookie` or `Range` now skip the cache, and responses with `Set-Cookie` or a streamed body aren't stored. The `rws` binary wraps every response in `CacheLayer`.
- **Proxy routes match a normalized path (#50).** Routes matched the raw request path, so `/%61dmin`, `//admin` or `/x/../admin` skipped a route protecting `/admin/*` (auth, IP filter, rate limit) and reached the backend through a catch-all route, which then saw `/admin`. Paths are now normalized before matching and sent upstream normalized (unreserved escapes decoded, a stray `%` encoded, repeated slashes collapsed, dot segments resolved), and `host` matches ignoring case and port.
