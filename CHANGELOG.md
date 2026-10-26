# Changelog

Notable changes per release. Versions follow [semver](https://semver.org/):
a major version means a breaking change — see
[Migrating to 18.0](https://rws8.tech/reference/migrating-18/) for the next one.
Releases and binaries: [GitHub releases](https://github.com/bohdaq/rust-web-server/releases).

## Unreleased (18.0.0)

### Breaking

- **`Header` fields are `Cow<'static, str>`** instead of `String`. Build with
  `Header::new(name, value)` or `.into()`; read with `&*h.value` /
  `h.value.as_ref()`.
- **Default response headers:** client hints (`Accept-CH`, `Critical-CH`, the
  hints in `Vary`) are opt-in with `RWS_CONFIG_CLIENT_HINTS=true`; `Vary` is
  `Origin` otherwise. Static files send `Cache-Control: no-cache`
  (`RWS_CONFIG_STATIC_CACHE_CONTROL`) instead of `no-store`, plus a standard
  `Last-Modified`; every response has a standard `Date`.
  `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy` and the default
  CSP are opt-in with `RWS_CONFIG_SECURITY_HEADERS=true` (a CSP set with
  `RWS_CONFIG_CSP` is always sent), and `Date-Unix-Epoch-Nanos` /
  `Last-Modified-Unix-Epoch-Nanos` with `RWS_CONFIG_NANOS_HEADERS=true`.
- **HTTP/2 server:** rws serves HTTP/2 with its own implementation
  (`h2_server`) instead of the `h2` crate; the hidden `h2_handler` module is
  gone. Same conformance (h2spec 146/146), limits and behaviour; the `h2`
  crate stays as the proxies' HTTP/2 client.
- **RSA keys:** `auth::verify_jwt_rs256` and `SamlIdpMetadata.signing_key` take
  `rsa_key::RsaPublicKey` (was `rsa::RsaPublicKey`); RSA keys under 2048 bits
  are rejected. The `rsa` crate (RUSTSEC-2023-0071) is no longer a dependency
  of the auth/SSO features.

### Added

- `If-Modified-Since` answers `304` for static files (when there's no `If-None-Match`).
- `[[upstream]] tls_ca_file` / `ReverseProxy::tls_ca_file`: trust a private CA for `https://` upstreams.
- Inline tables in `rws.config.toml` settings (`rate_limit = { max_requests = 5 }`).
- `RWS_CONFIG_OPEN_FILE_CACHE_STRICT=false`: reuse a cached file's metadata for the cache window instead of checking it per request.
- `mimalloc` feature: the `rws` binary allocates with mimalloc v3; release binaries and images are built with it — 4–6% less CPU per request, at a larger memory footprint (about 34 MiB under load instead of 9; up to ~120 MiB serving large files over TLS). Build without the feature for the smaller footprint.
- `RWS_CONFIG_RUNTIME=per-core`: one single-threaded runtime per core, each on its own `SO_REUSEPORT` listener (Unix) — more throughput on fast CPUs, more CPU per request on slow ones; the default stays `shared`.

### Fixed

- A request head must arrive within 30 s of its first byte, else `408` — a slow-drip client could hold a connection for weeks.
- `H2ReverseProxy::read_timeout_ms` was ignored; a silent HTTP/2 backend hung the request.
- The reverse proxy sent the client's `Content-Length` upstream next to its own (two headers — strict backends reject that, and it's a desync risk). It now sends one, drops headers named in `Connection`, `Trailer` and `Proxy-Connection`, and appends the client address to an incoming `X-Forwarded-For` instead of sending a second one.
- `CacheLayer` no longer stores `no-cache` responses, responses to `Range` requests, or responses that vary on a header outside its key.
- A JWKS key that can't be used is skipped instead of failing verification.

### Performance

On the benchmark runner, 1 KiB static files went from 49k to 126k requests/s
and the 1 KiB reverse proxy from 28k to 49k: open files, small
file contents and gzip output are cached per file version; one write per
response (`writev` for large bodies, `sendfile` with the head in the same
segment); header lists, dates and numbers built without allocation; request
and upstream-response heads parsed in one pass; HTTPS upstreams forwarded
without blocking a thread; access log buffered per thread (flushed every
100 ms). Files over TLS are read on the connection's task (13% less CPU and
about half the memory for large downloads), HTTP/2 is served by rws's own implementation (20–37% less CPU per request
than with the `h2` crate), and release builds
allocate with mimalloc (4–6% less CPU, more memory). Details:
[Benchmarks](https://rws8.tech/reference/benchmarks/).

## 17.113.2 — HTTP conformance

### Fixed

- **gzip no longer rewrites a partial body (#52).** A `Range` request with `Accept-Encoding: gzip` got the requested slice gzipped and `Content-Range` rewritten to the compressed size (`bytes=0-1200` of a 3,700-byte file → `Content-Range: bytes 0-37/37`), which no client can reassemble. A `206` is now sent uncompressed (RFC 9110 §14.4).
- **multipart/byteranges framing (#53).** A multi-range response had neither `Content-Length` nor `Transfer-Encoding`, so a keep-alive client couldn't tell where it ended and waited; the closing delimiter lacked its trailing `--`; and the boundary was the fixed string `String_separator`, which broke the framing of any part that contained it. The response now carries a `Content-Length`, ends with `--boundary--`, and uses a boundary (`Range::multipart_boundary`) that occurs in none of the parts (RFC 9112 §6.3, RFC 2046 §5.1.1). The legacy `Response::_parse_response` reads the boundary from `Content-Type` and no longer loops forever on a body without a closing delimiter.
- **Bodiless responses carry no body or Content-Length (#54).** `1xx`, `204` and `304` responses must have no content (RFC 9110 §6.4.1, §15.3.5): `OPTIONS` on a static file answered `204` with `Content-Length` and, over HTTP/2, a `DATA` frame. Every writer now drops the body (buffered, file or pipe) and `Content-Length`/`Transfer-Encoding` for those statuses (`Response::drop_content_for_status`). And a `200` to `OPTIONS` now sends its body: it used to announce it with `Content-Length` and then drop it, so the client waited for bytes that never came.
- **The static request path is percent-decoded (#55).** Files were looked up by the raw request target, so `GET /my%20file.txt` was a `404` for `my file.txt`, and the directory listing's (correctly encoded) links to such names were dead. The path is now decoded once, before the web-root and hidden-file checks of #43 (so `/%2e%2e/` is refused like `/../`); `%2520` stays a literal `%20`.

## 17.113.1 — security patch

Fixes from the 2026-10-25 security review. Upgrade from 17.113.0.

### Security

- **Proxy: interim responses (#48).** A `1xx` from an upstream (`100 Continue`, `103 Early Hints`) was taken as the final response; the real response stayed in the pooled connection and was delivered to the next request on it — possibly another client's. Both forwarders now read past interim responses (at most 16; `101 Switching Protocols` is still final).
- **Static files stay inside the web root (#43).** A request path with `..` segments (`/../../etc/passwd`) was resolved against the working directory and could read any file the process could; such paths now get `404`, and a symlink inside the root that points outside it is not followed. Dot-files and dot-directories (`.git/`, `.env`) are no longer served; set `RWS_CONFIG_SERVE_DOTFILES=true` to serve them again (`.well-known/` is always served).
- **Byte-range limits (#46).** Each range of a multi-range request was read into memory separately: `Range: bytes=0-,0-,0-,…` (thousands of ranges in a 64 KiB head) made one request hold thousands of copies of a file. Overlapping ranges are now merged, and more than 16 ranges are answered with the whole file as one range (RFC 9110 §14.2).
- **No CR/LF in header lines (#47).** The HTTP/1.1 writers copied header names, values and the reason phrase verbatim, so a value carrying user input (a redirect target, an upstream header) could inject headers or a second response; CR and LF now become spaces. The mailer does the same for `Subject`, `To`, `Cc`, `Reply-To` and `From`, and refuses an address containing CR, LF, `<` or `>` (it went into `MAIL FROM` / `RCPT TO` as is). HTTP/2 and HTTP/3 already refused such values.
- **CacheLayer: never cache private responses (#49).** The cache is keyed by URL, so a response built from one user's `Cookie` or `Authorization` was served to the next visitor, a `Set-Cookie` response was replayed to everyone, a `Range` response was served for the whole resource, and a streamed response was replayed with an empty body. Requests with `Authorization`, `Cookie` or `Range` now skip the cache, and responses with `Set-Cookie` or a streamed body aren't stored. The `rws` binary wraps every response in `CacheLayer`.
- **Proxy routes match a normalized path (#50).** Routes matched the raw request path, so `/%61dmin`, `//admin` or `/x/../admin` skipped a route protecting `/admin/*` (auth, IP filter, rate limit) and reached the backend through a catch-all route, which then saw `/admin`. Paths are now normalized before matching and sent upstream normalized (unreserved escapes decoded, a stray `%` encoded, repeated slashes collapsed, dot segments resolved), and `host` matches ignoring case and port.
- **`/mcp` needs a token (#44).** The `rws` binary served its MCP tools — config reload, blocklist, maintenance mode, server introspection — to anyone who could reach the port unless `MCP_TOKEN` was set. Now they're mounted only with a non-empty `MCP_TOKEN`, and `/mcp` sits behind the access log, IP blocklist and global rate limit. In `McpServer`: a request with an `Origin` header not listed in the new `.allowed_origins(...)` gets `403` (browser cross-site requests and DNS rebinding; `MCP_ALLOWED_ORIGINS` in the binary), a `POST` that isn't `application/json` gets `415`, an empty bearer token refuses every request, and tokens are compared in constant time.
- **Private keys out of the web root (#45).** With no paths configured, ACME stored `cert.pem`, `key.pem` and `acme_account.key` in the working directory — the web root — so both private keys could be downloaded. They now default to `.rws/acme/` (keys `0600`, directory `0700`, never served); on upgrade, files at the old default paths are moved there. The static server no longer serves `*.pem`, `*.key` or `rws.config.toml` anywhere. **If you ran ACME without explicit paths, rotate the certificate key and the ACME account key.** The Docker image already kept them in `/data`.

## 17.113.0 — 2026-10-24

- Proxy: pooled keep-alive upstream connections on every HTTP/1.1 path, pooled HTTPS backends, non-blocking forwarding to plain-HTTP upstreams; hot reload of routes, upstreams and `[middleware]` on `SIGHUP`.
- `rws --check` validates a config file; fixed inline middleware tables being ignored (an auth bypass), global `[middleware]`, IPv6 IP filters.
- HTTP/1.1: request heads read across reads (up to 64 KiB), pipelined requests kept, chunked bodies decoded.
- Static files: open-file cache, `sendfile` for plain-TCP downloads, quick requests run on the event loop.
- Compression skips small, incompressible and already-encoded bodies.
- Docker image no longer crashes at startup; published to Docker Hub with a smoke test and a `/data` volume.

## 17.112.0 — 2026-10-24

- RFC 9110 byte ranges; `Content-Range` only on `206`, now over HTTP/2 and HTTP/3 too.
- Every setting works from `rws.config.toml`; unknown keys warn instead of vanishing.
- Config-driven proxy auth fails closed instead of serving the route unauthenticated.
- Automatic HTTPS (ACME) from one config section, verified against Pebble.
- The documented proxy config no longer returns `502`; proxied responses no longer break HTTP/2.

## 17.111.0 — 2026-10-23

- Prebuilt binaries, Homebrew tap, GHCR image, `cargo binstall`; `rws --version` / `--help`.

## 17.110.0 — 2026-10-23

- Directory listing for static directories without `index.html`; opt-in SPA fallback.
- Ingress: TLS to the API server, watch API, `pathType: Exact`, IngressClass filtering.
- Service discovery: Consul, etcd, Docker labels, DNS SRV / weighted DNS.
- Canary: smooth weighted round-robin, live weight updates; circuit breaker wired into `ReverseProxy`.
- Secret references (`vault://`, `aws-sm://`, `azkv://`); `Response::json` / `Response::text`.
- WebAssembly guest component (`wasm32-wasip2`) with outbound HTTP and streaming bodies.
- Helm chart.

## Earlier

See the [GitHub releases](https://github.com/bohdaq/rust-web-server/releases) and the git tags.
