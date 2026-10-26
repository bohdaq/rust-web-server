# Changelog

Notable changes per release. Versions follow [semver](https://semver.org/):
a major version means a breaking change — see
[Migrating to 18.0](https://rws8.tech/reference/migrating-18/) for the next one.
Releases and binaries: [GitHub releases](https://github.com/bohdaq/rust-web-server/releases).

## Unreleased (18.1.0)

### Fixed

- **WebSocket frames claiming a huge payload (#8).** `WebSocket::read_frame`
  allocated whatever length a client claimed (up to 2^64 − 1 bytes) before
  reading it, so one 14-byte frame could abort the process. Payloads are now
  read as they arrive and capped at `WebSocket::MAX_FRAME_PAYLOAD` (16 MiB);
  `WebSocket::read_frame_limited` takes another limit.
- **Redis replies (#8).** The RESP reader allocated a bulk string at the
  length the server claimed; bulk strings are now read as they arrive, capped
  at 512 MiB (Redis's own maximum), reply lines at 64 KiB, and a connection
  closed mid-reply is an error instead of reading as `+OK`.
- **DNS SRV discovery (#8).** A response whose SRV record ended inside its
  RDATA panicked while reading priority, weight and port; it is now an error.

### Added

- Fuzz targets for WebSocket frames, DNS SRV responses, Redis replies and
  `Cookie` headers, and a request-smuggling seed corpus (CL+TE, conflicting
  or listed `Content-Length`, obfuscated or folded `Transfer-Encoding`,
  bare-LF and oversized chunks) whose handling a test pins (#8).

## 18.0.0 — 2026-10-26

### Breaking

- **The `rws` binary runs one runtime per core** (`RWS_CONFIG_RUNTIME=per-core`,
  Unix): one single-threaded runtime per core on its own `SO_REUSEPORT`
  socket, as nginx runs workers. On the fastest benchmark runners that is
  where rws trailed nginx on requests per second; on slower machines it costs
  more CPU per small request. `runtime = "shared"` / `RWS_CONFIG_RUNTIME=shared`
  restores 17.x behaviour; library applications stay `shared` unless they opt in.
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
- **`Request.body` is a `request::Body`** (#31), not a `Vec<u8>`: the bytes as before (it derefs to `&[u8]`), or a stream of a body still arriving. Build with `.into()` / `Default::default()`; `.to_vec()` where a `Vec<u8>` is wanted. See the migration page.
- **ACME renewal follows the certificate and the CA** (#1). Certificates are renewed at two-thirds of their lifetime, or earlier when the CA's ACME Renewal Information (RFC 9773) suggests it, instead of 30 days before expiry — which would renew a 6-day certificate at once and leave a 45-day one (Let's Encrypt's default from 2028) a third of its life. Renewal orders name the certificate they replace (`replaces`). `RWS_CONFIG_ACME_PROFILE` orders a certificate profile (`classic`, `tlsserver`, `shortlived`). `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` has no default now; set it to keep a fixed margin. `AcmeConfig::renew_before_days` is an `Option<i64>` and `AcmeConfig` has a `profile` field. The renewal loop sends `SIGHUP` only after a renewal (it did after every check).
- **MCP sessions are bounded** (#4): idle for an hour (`McpServer::session_ttl`) or past 10 000 (`max_sessions`, least recently used first) a session is evicted with its subscriptions and SSE streams; `DELETE /mcp` ends one; at most 1000 SSE streams (`max_sse_clients`, then `503`). A request carrying an `Mcp-Session-Id` the server doesn't know now gets `404` — the transport's signal to initialize again — instead of running with an empty context.
- **CORS allow-all is off by default** (#59). `RWS_CONFIG_CORS_ALLOW_ALL` defaulted to `true`, which echoed any `Origin` with `Access-Control-Allow-Credentials: true` — any site could read a logged-in user's responses. With no CORS configuration `rws` now adds no CORS headers; list origins in `RWS_CONFIG_CORS_ALLOW_ORIGINS`, or set `RWS_CONFIG_CORS_ALLOW_ALL=true` for the old behaviour.
- **HTTP/2 server:** rws serves HTTP/2 with its own implementation
  (`h2_server`) instead of the `h2` crate; the hidden `h2_handler` module is
  gone. Same conformance (h2spec 146/146), limits and behaviour; the `h2`
  crate stays as the proxies' HTTP/2 client.
- **RSA keys:** `auth::verify_jwt_rs256` and `SamlIdpMetadata.signing_key` take
  `rsa_key::RsaPublicKey` (was `rsa::RsaPublicKey`); RSA keys under 2048 bits
  are rejected. The `rsa` crate (RUSTSEC-2023-0071) is no longer a dependency
  of the auth/SSO features.

### Added

- MCP conformance: the official suite (`@modelcontextprotocol/conformance`
  0.1.16, 2025-11-25 scenarios) runs on every push against
  `examples/mcp_conformance.rs` (`ci/e2e/mcp_conformance.sh`): 28 of 31 pass,
  the three elicitation scenarios are known failures
  (`ci/mcp-conformance-baseline.yml`). For it, `McpContent::audio`,
  `McpContent::blocks` (several items in one tool result) and
  `McpContent::blob` (binary resources, sent as `blob`). A `tools/call` with a
  `progressToken` from a client that accepts `text/event-stream` is answered
  as an event stream — progress notifications, then the result — instead of
  sending progress to the separate `GET /mcp` stream, which a client may not
  have opened yet; progress without such a stream now goes only to the
  caller's session (it went to every connected session).
- MCP gateway (#62): `mcp::gateway::McpGateway` puts one endpoint in front of
  upstream MCP servers — tools as `<prefix>__<tool>`, `allow(identity, tools)`
  rules (deny by default), upstream credentials set per `Upstream` and never
  the caller's; JSON and event-stream replies, paginated `tools/list`, session
  re-initialization; upstream failures are `isError` results.
  `McpServer::require_bearer_tokens` (a token per named caller, seen as
  `{"sub":…}` in `auth_claims`) and `McpServer::tool_filter` (per-request tool
  visibility) work on any MCP server. `examples/mcp_gateway.rs`; docs page
  [MCP gateway](/mcp/gateway/).
- Typed MCP tools (#61): `McpServer::typed_tool(name, description, |ctx, args: In| -> Result<Out, String>)`
  (`serde`) derives `inputSchema`/`outputSchema` from the types, deserializes
  the arguments and returns `structuredContent`. `#[derive(JsonSchema)]`
  (`macros`) and the `schema::JsonSchema` trait produce the schemas, following
  `#[serde]` renames; attributes that would make them disagree are compile
  errors. `examples/mcp_inventory.rs` exposes an app's domain functions as
  REST routes and MCP tools with OAuth and live resource updates; docs page
  [Build agent tools](/mcp/agent-tools/). **rws-macros must be published
  (with the derive) before rust-web-server 18.0.0.**
- `If-Modified-Since` answers `304` for static files (when there's no `If-None-Match`).
- `[[upstream]] tls_ca_file` / `ReverseProxy::tls_ca_file`: trust a private CA for `https://` upstreams.
- Inline tables in `rws.config.toml` settings (`rate_limit = { max_requests = 5 }`).
- `RWS_CONFIG_OPEN_FILE_CACHE_STRICT=false`: reuse a cached file's metadata for the cache window instead of checking it per request.
- `mimalloc` feature: the `rws` binary allocates with mimalloc v3; release binaries and images are built with it — 4–6% less CPU per request, at a larger memory footprint (about 34 MiB under load instead of 9; up to ~120 MiB serving large files over TLS). Build without the feature for the smaller footprint.
- `RWS_CONFIG_RUNTIME=per-core`: one single-threaded runtime per core, each on its own `SO_REUSEPORT` listener (Unix) — more throughput on fast CPUs, more CPU per request on slow ones; the default stays `shared`.
- Supply chain (#5): release binaries are built with `cargo auditable` (`cargo audit bin rws` works on them), releases carry a CycloneDX SBOM, archives and images get GitHub build-provenance attestations, and images are signed keyless with cosign. CI runs `cargo deny check` (advisories, permissive licenses only, crates.io sources only; `deny.toml`).
- Streamed request bodies (#31): an application that answers `true` from `Application::streams_body` (or `AppWithState::stream_body(method, pattern)`) gets `request.body` as a stream it reads while the upload arrives — a 512 MiB upload stays under 64 MiB of memory. HTTP/1.1 (plain and TLS) and HTTP/2 (the stream window is credited back as the handler reads); HTTP/3 and the HTTP/1.1-only build still read the body first. `Middleware::accepts_streamed_body` lets a header-only layer pass it through. `Storage::put_stream(key, reader, content_type)` stores a stream: S3 as a multipart upload in 8 MiB parts (aborted on failure), local disk via a temporary file renamed into place — a 2 GiB HTTP upload streamed to an S3-compatible server grew peak memory by under 96 MiB.
- MCP (#3): structured tool results (`McpContent::structured`, `McpServer::tool_output_schema` → `outputSchema`/`structuredContent`), tool and server titles (`tool_title`, `title`) and icons (`McpIcon`, `tool_icon`, `icon`). `tools/call` arguments are checked against the input schema (an object, `required` properties, property `type`s); a mismatch is an `isError` tool result rather than reaching the handler. With `require_oauth`, the server publishes RFC 9728 protected-resource metadata at `/.well-known/oauth-protected-resource` and its `401` carries `WWW-Authenticate: Bearer resource_metadata=…`; the `/.well-known/oauth-authorization-server` look-alike document is gone. The server speaks protocol versions 2024-11-05 through 2025-11-25 (it advertised 2024-11-05 only and echoed any older string): an unsupported requested version is answered with 2025-11-25, an unsupported `MCP-Protocol-Version` header gets `400`, and JSON-RPC batches are refused for sessions on 2025-06-18 or later.

### Fixed

- MCP (#60): `extract_arg` returns numbers and booleans (it read only strings), so the binary's `set_maintenance {"enabled": true}` and `feature_set` did nothing and `recent_requests`/`recent_errors` ignored `n`; the `reload_config` tool now reloads (it set a flag only the HTTP/1.1-only server reads) via the new `config_reload::request_reload()`. The built-in tools carry read-only/destructive hints (`McpServer::tool_annotations`), and the new [Operate rws from an AI client](https://rws8.tech/mcp/operate/) page has the setup and playbooks.
- Docs (#12): MCP sampling, roots and logging are marked deprecated by the 2026-07-28 MCP specification (they keep working); the MCP SSE stream is documented — and now tested — over HTTP/2 and HTTP/3, not only HTTP/1.1.
- A request head must arrive within 30 s of its first byte, else `408` — a slow-drip client could hold a connection for weeks.
- `H2ReverseProxy::read_timeout_ms` was ignored; a silent HTTP/2 backend hung the request.
- The reverse proxy sent the client's `Content-Length` upstream next to its own (two headers — strict backends reject that, and it's a desync risk). It now sends one, drops headers named in `Connection`, `Trailer` and `Proxy-Connection`, and appends the client address to an incoming `X-Forwarded-For` instead of sending a second one.
- `CacheLayer` no longer stores `no-cache` responses, responses to `Range` requests, or responses that vary on a header outside its key.
- A JWKS key that can't be used is skipped instead of failing verification.
- HTTP/2 (#6): resets the server sends because the client broke a stream's rules (a zero or overflowing `WINDOW_UPDATE`, a bad `PRIORITY`, frames on a closed stream) count toward the rapid-reset budget, closing the "MadeYouReset" hole (CVE-2025-8671) in rws's own HTTP/2 server; and a connection with nothing running that sends nothing for 30 s — a silent client, a half-sent header block or request body, a client that never opens the flow-control window — is closed with `GOAWAY(NO_ERROR)`. It used to stay open forever.
- `If-Modified-Since` (new in 18.0) accepts the obsolete RFC 850 and asctime date forms as well as the IMF-fixdate (RFC 9110 §5.6.7); a client sending them always got the whole file.
- `If-Range` with a date (on top of 17.113.2's entity-tag handling): the `Range` applies when the date equals the file's `Last-Modified` and that is at least a second old (a strong validator), else the whole file is sent.

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

### Upgrading

Some fixes refuse or change what 17.113.1 accepted:

- An HTTP/1.1 request without `Host`, with two `Host` headers, with a lower-case method (`get`), with a folded header line or with whitespace before a header's colon now gets `400` (#57).
- An HTTP/1.0 connection now closes after each response unless the client sends `Connection: keep-alive` (RFC 9112 §9.3; #57).
- CORS origins match whole: an allow-list entry that relied on matching part of an origin (`example.com` for `https://app.example.com`) now matches nothing — list each origin (#58).
- Request paths are percent-decoded: a file whose name contains a literal `%` is requested as `%25` (#55).

### Fixed

- **gzip no longer rewrites a partial body (#52).** A `Range` request with `Accept-Encoding: gzip` got the requested slice gzipped and `Content-Range` rewritten to the compressed size (`bytes=0-1200` of a 3,700-byte file → `Content-Range: bytes 0-37/37`), which no client can reassemble. A `206` is now sent uncompressed (RFC 9110 §14.4).
- **multipart/byteranges framing (#53).** A multi-range response had neither `Content-Length` nor `Transfer-Encoding`, so a keep-alive client couldn't tell where it ended and waited; the closing delimiter lacked its trailing `--`; and the boundary was the fixed string `String_separator`, which broke the framing of any part that contained it. The response now carries a `Content-Length`, ends with `--boundary--`, and uses a boundary (`Range::multipart_boundary`) that occurs in none of the parts (RFC 9112 §6.3, RFC 2046 §5.1.1). The legacy `Response::_parse_response` reads the boundary from `Content-Type` and no longer loops forever on a body without a closing delimiter.
- **Bodiless responses carry no body or Content-Length (#54).** `1xx`, `204` and `304` responses must have no content (RFC 9110 §6.4.1, §15.3.5): `OPTIONS` on a static file answered `204` with `Content-Length` and, over HTTP/2, a `DATA` frame. Every writer now drops the body (buffered, file or pipe) and `Content-Length`/`Transfer-Encoding` for those statuses (`Response::drop_content_for_status`). And a `200` to `OPTIONS` now sends its body: it used to announce it with `Content-Length` and then drop it, so the client waited for bytes that never came.
- **The static request path is percent-decoded (#55).** Files were looked up by the raw request target, so `GET /my%20file.txt` was a `404` for `my file.txt`, and the directory listing's (correctly encoded) links to such names were dead. The path is now decoded once, before the web-root and hidden-file checks of #43 (so `/%2e%2e/` is refused like `/../`); `%2520` stays a literal `%20`. A `Range` request for a name containing a space, quote, `&`, `|` or `;` no longer gets `416` (the file reader refused such paths), and the general static path finds escaped names' `ETag`.
- **If-None-Match is a list, compared weakly (#56).** Only a header equal to the file's `ETag` gave `304`: a client sending several tags (`"a", "b"`) or the weak form (`W/"…"`) always got the whole file again (RFC 9110 §13.1.2).
- **If-Range is honored (#56).** It was ignored: a client resuming a download with `If-Range` got `206` with bytes of the file as it is now, even after the file changed. A `Range` now applies only while `If-Range` strongly matches the current `ETag`; otherwise the whole file is sent with `200` (RFC 9110 §13.1.5).
- **Range: one satisfiable range is enough; 416 names the length (#56).** `bytes=0-9, 99999-` on a small file was a `416` because of the second range; unsatisfiable ranges are now skipped and `416` is sent only when none can be served — with `Content-Range: bytes */<length>`, which it lacked (RFC 9110 §14.2, §15.5.17).
- **405 with Allow for files (#56).** `POST`/`PUT`/`DELETE` on an existing static file got `404`, which says the resource doesn't exist; it now gets `405 Method Not Allowed` with `Allow: GET, HEAD, OPTIONS` (RFC 9110 §15.5.6).
- **Strict HTTP/1.1 request heads (#57).** Requests without `Host` or with two, a lower-case method, `HTTP/0.9`/`HTTP/2.0` in the request line, folded header lines, whitespace before a header's colon, and header names that aren't tokens are answered `400` — each is a way for `rws` and a proxy in front of it to read the same bytes differently (RFC 9112 §2.2–§3.2, §5.1). `Connection` is evaluated token by token: `Connection: close, TE` used to keep the connection open; an HTTP/1.0 connection now stays open only with `Connection: keep-alive`. `Request::parse` itself stays lenient for library callers; the checks are in the connection loops (`server::framing::validate_head`).
- **CORS: whole-origin matching; exposed headers on the response (#58).** An origin was allowed when it was a substring of the configured list, so with `https://app.example.com` listed, `https://app.example.co` (and `https://pp.example.com`) got CORS headers — with credentials when those are on. Origins are now compared whole (`Cors::origin_is_allowed`). `Access-Control-Expose-Headers` was sent only on the preflight, which browsers ignore; it is now on the actual response.

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
