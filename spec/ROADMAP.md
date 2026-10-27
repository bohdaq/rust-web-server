# Roadmap

Open work as a sequence of releases. Items come from a review of the code, the competitors (Caddy, nginx, HAProxy, Traefik, Envoy, Pingora, Ferron), the Rust framework landscape, the MCP/AI ecosystem and standards changes (2026-10-25).

Every breaking change is collected into 18.0 (the "Breaking changes" milestone before it); after 18.0, minor releases are additive and anything breaking waits for 19.0. Each release has a theme, a set of items and exit criteria — it ships when the criteria are met, not on a date. Deadline-driven work comes first: Let's Encrypt's default certificate lifetime drops to 64 days on 10 Feb 2027, so 18.0 must ship well before then.

Unordered ideas live in [IDEAS.md](IDEAS.md); finished plans in [archive/](archive/). Item numbers are stable references (`#N`), not an order.

## 17.113.1 — Security patch

From the 2026-10-25 security review. Patches the released 17.113.0 (branch off the `17.113.0` tag, no 18.0 changes), then the same fixes go to `main` for 18.0. Ships first, before anything else on this list.

- **#43** ✅ *17.113.x: a2aa39b4 · main: cdbca432* **Static files stay inside the web root** — resolve the request path, refuse `..` segments, and require the resolved path (after symlinks) to stay under the root; directory listings likewise. Stop serving dot-files and dot-directories (`.git/`, `.env`) by default.
- **#44** ✅ *17.113.x: abbfcbe7 · main: d9495ae6* **`/mcp` needs a token** — with `MCP_TOKEN` unset or empty the built-in MCP tools are not mounted (or answer `401`); check `Origin`, accept only `application/json`, and put `/mcp` behind the blocklist, maintenance and rate-limit layers.
- **#45** ✅ *17.113.x: 0b0d4e3c · main: 15d9b2e2* **Private keys out of the web root** — ACME writes its certificate key and account key outside the served directory (a state directory, as the Docker image's `/data`), with `0600` permissions; refuse to serve `*.pem` / `*.key` and `rws.config.toml`.
- **#46** ✅ *17.113.x: f2c3aaac · main: 5be76a90* **Byte-range limits** — cap the number of ranges per request, merge overlapping ones, and fall back to a full `200` response past the cap (RFC 9110 §14.2 allows ignoring a `Range`).
- **#47** ✅ *17.113.x: 43f2b517 · main: 5af2b39a* **No CR/LF in header values** — the HTTP/1.1, HTTP/2 and HTTP/3 writers and the mailer reject or strip CR/LF in header values (redirect targets, `return_to`, subjects, recipients).
- **#48** ✅ *17.113.x: 6f75c849 · main: 4ed84728* **Proxy: interim responses** — a `1xx` from the upstream is not the final response; read past it before returning the connection to the pool, in both forwarders. Otherwise one client's response can be delivered to another.
- **#49** ✅ *17.113.x: 93ffa0f3 · main: ae03dea6* **CacheLayer: never cache private responses** — skip requests carrying `Authorization` or `Cookie`, responses with `Set-Cookie`, and streamed responses (their body isn't stored). The default binary wraps its app in `CacheLayer`.
- **#50** ✅ *17.113.x: b1f5a4dd · main: 8b1147c3* **Proxy routes match a normalized path** — percent-decoding of unreserved characters, dot segments and repeated slashes resolved before matching (and the same path sent upstream), host matching case-insensitive, so per-route auth, IP filters and rate limits can't be skipped through a catch-all route.

**Progress:** fixes land on branch `17.113.x` (from the `17.113.0` tag), each with a regression test that fails on 17.113.0, then are ported to `main` — all eight done on both (2026-10-26: cargo test, http1, all-features, MSRV 1.88, sso feature set, h2spec 146/146, acme_pebble, slow_clients, config_reload green on each; same-run A/B vs pre-port main, run 40250864064: CPU per request within noise — −3% to +6% per scenario, rws still ahead of nginx in every one); the version bump, tag, publish and advisory are the maintainer's.

**Done when:** a regression test per fix; the reproductions from the review fail against the patched binary; 17.113.1 published (crates.io, GitHub release, Docker Hub / GHCR, Homebrew) with a GitHub security advisory; the same fixes merged to `main`.

## 17.113.2 — HTTP conformance

Net-new RFC-conformance bugs from the 2026-10-26 review, each reproduced against a running build. Correctness, not new features, so they land before 18.0 (on `17.113.x` after the security patch, then `main`). The request-validation leniency overlaps the smuggling hardening in #43–#50; the rest is user-facing breakage.

- **#52** ✅ *17.113.2: 8ef732b4 · main: 631784be* **gzip must not rewrite a partial body** — a `Range` request with `Accept-Encoding: gzip` is gzipped after slicing, and `Content-Range` is rewritten to the compressed size (`bytes=0-1200` of 3700 bytes → `Content-Range: bytes 0-37/37`). Don't compress a `206`/`Content-Range` body (RFC 9110 §8.4.1, §14.4). `src/compression/mod.rs`.
- **#53** ✅ *17.113.2: 41a95623 · main: 77a3500f* **multipart/byteranges framing** — the response has no `Content-Length` and no `Transfer-Encoding`, so a keep-alive client can't delimit the body and hangs; the close delimiter is missing its trailing `--`; the boundary is a fixed literal (`String_separator`) that collides with matching content. Set a length, close the delimiter, and generate a unique boundary (RFC 9112 §6.3, RFC 2046 §5.1.1). `src/response/mod.rs`.
- **#54** ✅ *17.113.2: 8c194614 · main: 3a27fd5e* **bodiless responses carry no body or Content-Length** — `204`, `304`, `HEAD` and `1xx` must send no content and no `Content-Length`; today `OPTIONS`→`204` sends `Content-Length: 3` (and on HTTP/2, a `DATA` frame — malformed per RFC 9113 §8.1.1). RFC 9110 §6.4.1, §15.3.5. `src/response/mod.rs`, `src/h2_handler/`.
- **#55** ✅ *17.113.2: cc10e833, d5008397, 4052d259 · main: a5e8cf51, bbc19851, c81c40ce* **percent-decode the request path** — the filesystem lookup uses the raw target, so `GET /my%20file.txt` 404s for `my file.txt`, and directory-listing links are emitted percent-encoded and then 404 (RFC 3986 §2.1). Decode once, after the smuggling checks, before the path is resolved (keep the `..`-confinement from #43). `src/url/mod.rs`, `src/app/controller/static_resource/`.
- **#56** ✅ *17.113.2: 0a80ae7c, 0b02016a, a0511235, 74237030 · main: 9d3b8a0a, 7e799d31, d50ee2a1, f27970c5, 3a0f5a00* **conditional requests and status codes** — honor `If-Range` (a stale validator must drop the `Range` and send `200`, not `206`); parse `If-None-Match` as a list with weak comparison; accept all three HTTP-date formats in `If-Modified-Since` (RFC 850 and asctime, not only IMF-fixdate); answer `405` + `Allow` for a known path with an unsupported method; put `Content-Range: bytes */len` on a `416`, and serve a `206` when at least one requested range is satisfiable. RFC 9110 §13, §14, §15.5.6. `src/app/controller/static_resource/`, `src/range/`, `src/http/date.rs`.
- **#57** ✅ *17.113.2: 4d1c8082 · main: 51a81d33* **strict request validation** — reject a missing or duplicated `Host` (`400`), whitespace before a field colon, obs-fold continuation lines, and field names that aren't tokens; match the method case-sensitively; reject `HTTP/0.9` and `HTTP/2.0` on the HTTP/1.1 wire; evaluate `Connection` token by token (`close, TE` currently serves keep-alive). RFC 9112 §2.2–§3.2, §5.1, §9.6. `src/request/`, `src/server/framing.rs`, `src/server/http1_async.rs`.
- **#58** ✅ *17.113.2: a9a04935 · main: 895a9c97* **CORS matching and exposed headers** — match a configured origin exactly, not by substring (`app.example.com` currently also matches `app.example.co`), and send `Access-Control-Expose-Headers` on the actual response, not only the preflight. Correctness, so it stays in the patch; the allow-all default change is #59 under 18.0. Fetch standard / RFC 6454. `src/cors/`.

**Done when:** a regression test per item that fails on the current build; the review's reproductions (gzip-on-206, the multi-range hang, `OPTIONS`→`204`, the `%20` 404) pass; h2spec still 146/146.

**Progress:** fixes land on branch `17.113.2` (from `17.113.x`, so 17.113.1 can be tagged from `17.113.x` without them), then are ported to `main`. HTTP/1.0 closing unless `Connection: keep-alive` (#57, beyond the item's text) kept by decision 2026-10-26. All seven on `17.113.2` (2026-10-26: cargo test, http1, sso feature set, all-features, MSRV 1.88, h2spec 146/146, acme_pebble, slow_clients, config_reload green; new fuzz target `static_path_decode`, `validate_head` added to `http1_framing`). #56's `If-Modified-Since` date formats are main-only: 17.113.x has no `If-Modified-Since` support to fix. Found on the way: Range requests for names with a space, quote, `&`, `|` or `;` got `416` (fixed in #55's second commit). Ported to `main` 2026-10-26, same checks green there (h2spec 146/146, acme_pebble, slow_clients, config_reload, http1, all-features, MSRV); the review's reproductions (gzip-on-206, multi-range keep-alive, `OPTIONS`→`204` on h1 and h2, the `%20` 404) checked against a running main build. On `main` `If-Range` also honors a date equal to `Last-Modified`.

## 18.0.0 milestone — Breaking changes

All of 18.0's breaking changes land together, on `main` before 18.0.0 is tagged, so users migrate once and every later 18.x release stays additive. The changes already on `main` (Header `Cow` fields, default response headers, `rsa_key`) are listed under 18.0.0 below; this milestone adds the one still to build.

- **#31** ✅ *decided 2026-10-26: `Request.body` becomes a `Body` type with `body_bytes()`; requests stay buffered unless the app opts in per request · main: 160de19c (Body), 7f101e92 (HTTP/1.1 streaming, 512 MiB constant memory), f61ecbb6 (HTTP/2, flow-control backpressure), step 4 (`Storage::put_stream`: S3 multipart in 8 MiB parts, local temp+rename; 2 GiB HTTP→S3 e2e < 96 MiB growth). Not done: HTTP/3 and the http1-only build read the body first; Azure `put_stream` buffers; no streaming multipart-form parser* **Streaming request bodies** — an upload API and multipart straight to storage (`Request.body` is a `Vec<u8>` today).
- **#30** *decided 2026-10-26: not in 18.0 — done whole in 18.x as one server-hosted WebSocket session type (HTTP/1.1 Upgrade, HTTP/2 extended CONNECT, HTTP/3), pulling #24's router-level upgrade forward, with deflate on top (its per-connection context needs that type)* **WebSocket permessage-deflate, then WebSocket over HTTP/2 and HTTP/3** (RFC 8441 / RFC 9220 extended CONNECT) — not breaking itself, but it builds on the streaming body type, so it follows #31 (it can ship in an 18.x release if 18.0 needs to go out first).
- **#59** ✅ *main: 6054bc38* **CORS allow-all off by default** — `cors_allow_all` currently defaults on, reflecting any origin with `Access-Control-Allow-Credentials: true`. Default it off (opt-in), so a server doesn't permit cross-origin credentialed reads unless configured. A behavior change existing deployments would feel, so it rides 18.0 rather than a patch.

**Breaking:** `Request.body` becomes a streaming body type, with a buffered helper (`body_bytes()`) for handlers that want the whole body. `cors_allow_all` now defaults off (#59).

**Done when:** a multi-GB upload streamed to S3 at constant memory; existing handlers ported through the buffered helper; the migration page covers it.

## 18.0.0 — Correct and current

The breaking release already on `main` (Header `Cow` fields, new default headers, in-crate RSA, the per-request CPU work), plus the fixes that can't wait: certificate renewal that survives Let's Encrypt's shorter lifetimes, an MCP server that speaks a current protocol, and the first signed, SBOM-carrying release.

- **#1** ✅ *main: 8d784866 — two-thirds fallback, ARI window + `replaces` (retried without if refused), `RWS_CONFIG_ACME_PROFILE`; Pebble e2e on `shortlived` + ARI; `RENEW_BEFORE_DAYS` kept as an optional override (not dropped; confirmed 2026-10-26)* **ACME renewal rewrite** — ACME Renewal Information ([RFC 9773](https://www.rfc-editor.org/rfc/rfc9773.html)), profile selection (`classic` / `tlsserver` / `shortlived`), a two-thirds-of-lifetime fallback; drop the fixed 30-day `RENEW_BEFORE_DAYS` (`src/acme/mod.rs`). Let's Encrypt offers 45-day certificates now and makes them the default by [Feb 2028](https://letsencrypt.org/2025/12/02/from-90-to-45) (64 days from Feb 2027); a fresh 6-day certificate would look due for renewal immediately.
- **#2** ~~**Proxy request-header correctness**~~ — done in e5759f5e (one `Content-Length`, `Connection`-listed headers dropped, `X-Forwarded-For` extended).
- **#3** ✅ *main: 8a408f2f, b7ba174a, 9831d4e9, e73b8f23 — speaks 2024-11-05..2025-11-25 (`negotiate_version`, per-session), `MCP-Protocol-Version` (400 if unsupported), batches only ≤ 2025-03-26, RFC 9728 metadata + `resource_metadata` in `WWW-Authenticate`, audience check (was already enforced; documented as the RFC 8707 resource), `403` on bad `Origin` (17.113.1 #44), `outputSchema`/`structuredContent`, titles and icons, input validation as `isError` results. Not done: 2025-11-25's optional Tasks, URL elicitation and sampling-with-tools (not advertised)* **MCP catch-up to 2025-11-25** — the server advertises protocol `2024-11-05` (`src/mcp/mod.rs`). Advertise what's implemented; accept JSON-RPC batches only from clients on ≤ 2025-03-26 (removed in [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/changelog)); serve RFC 9728 `/.well-known/oauth-protected-resource` with `WWW-Authenticate: … resource_metadata=` instead of the authorization-server document; check the token audience (RFC 8707); `403` on a bad `Origin`; require `MCP-Protocol-Version`; `outputSchema` / `structuredContent`, `title` / `icons`; input-validation failures as `isError` tool results ([2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/changelog)).
- **#4** ✅ *main: 9856fae1 — 1 h idle TTL, 10 000 cap (LRU), `DELETE /mcp`, `404` for unknown session ids, SSE cap 1000 (`503`); evicted sessions drop their subscriptions and streams* **MCP session eviction** — TTL and a cap for sessions, subscriptions and SSE clients; today nothing is evicted.
- **#5** ✅ *main: 76c23cfc — `deny.toml` + CI `cargo deny`; release: `cargo auditable` builds, CycloneDX SBOM, build-provenance attestations (archives + image), keyless cosign; tools tried locally and the workflow actionlint-clean, but the release path first runs on the next tag* **Supply chain** — `deny.toml` + cargo-deny in CI, cargo-auditable release builds, a CycloneDX SBOM, GitHub build-provenance attestations, keyless cosign signatures on images. EU CRA reporting duties for manufacturers (who would ship rws) began 11 Sep 2026.
- **#6** ✅ *main: 6c6679ea — rws's own h2 server counts provoked resets (MadeYouReset) in the rapid-reset budget and closes idle/stalled connections after 30 s; CONTINUATION, header-list and concurrency limits were already tested; the `h2` crate (now client-only) is 0.4.20, past the 0.4.12 fix* **HTTP/2 DoS limits** — confirm the h2 version carries the MadeYouReset fix (CVE-2025-8671); set and test reset-rate, pending-accept-reset, CONTINUATION and header/idle timeouts (only two h2 limits are set in `src/h2_handler/mod.rs`).
- **#12** ✅ *main: 2758eedd — deprecation notes (MCP 2026-07-28, SEP-2577) in docs and rustdoc; MCP SSE tested over real HTTP/2 and HTTP/3 clients; HTTP/2 re-benchmarked: rws ahead of nginx (runs 40252061115–67156 on AMD, 40259499110 on Intel; the old 53.6k/86.4k figure was already gone)* **Doc and benchmark truth** — mark `ctx.sample()`, roots and `logging/setLevel` as deprecated by MCP 2026-07-28; verify MCP SSE over HTTP/2 and HTTP/3 at runtime; re-run the HTTP/2 benchmark (the published 53.6k vs nginx 86.4k req/s predates the h2 work).

**MCP: agent-ready by 18.0.** Three ways an AI agent uses rws, each wired to the items that make it real and safe. These ship with 18.0 (layers 1–2 are gated by #44 and #3, already here; the gateway foundation is a stretch — 18.0 may ship without it if the ACME deadline forces the release, and the full gateway stays #28 at 18.9.0):

- **#60** ✅ *main: b9d47fb5 — docs page + Claude Code/Desktop/Cursor recipe, tools annotated read-only/destructive; running the recipe against the binary found and fixed two bugs: `extract_arg` ignored booleans/numbers (maintenance and feature flags could never be switched on) and `reload_config` did nothing in async builds* **Operate rws from an AI client (layer 1)** — the ~20 built-in tools are already there; this adds the authenticated setup recipe and a docs page (point an MCP client at `/mcp` with a token; diagnose a spike → `recent_requests`/`block_ip`, bad deploy → `set_maintenance`/`reload_config`). Needs #44 (token) + #3 (protocol).
- **#61** ✅ *main: `typed_tool` + `#[derive(JsonSchema)]`/`schema::JsonSchema` (serde-aware, shared with #22 later), `examples/mcp_inventory.rs` (REST + MCP tools on one domain, bearer or OAuth, `inventory://items` live updates over SSE, run live), page `mcp/agent-tools`. Release note: rws-macros must be published with the derive before 18.0.0* **Build agent tools on rws (layer 2)** — an `examples/` app that exposes its domain functions via `app.mcp()` with `outputSchema` derived from serde types (shares the derivation with #22), `require_oauth`, and SSE; plus a docs page. Shows rws turning any app into an MCP server for free. Needs #3.
- **#62** ✅ *main: `mcp::gateway::McpGateway`/`Upstream` + `McpServer::require_bearer_tokens`/`tool_filter`; fake-upstream tests (token never forwarded, SSE, pagination, 404 re-init, error mapping), fuzz target `mcp_gateway_reply` (1M runs clean), `examples/mcp_gateway.rs` (run live), page `mcp/gateway`. Left for #28: refresh/caching of `tools/list`, federating resources/prompts/notifications, per-tool rate limits, audit log, version bridging* **Minimal MCP gateway (layer 3 foundation)** — one authenticated endpoint federating N upstream MCP servers: `prefix__tool` names, a per-identity tool allow-list, and upstream credentials injected server-side (never passing the client token through). The richer gateway (`tools/list` caching, per-tool rate limits, version bridging, audit log) is #28 at 18.9.0. This is rws's sharpest AI positioning, so a minimal form targets 18.0.

**Breaking:** Already on `main` — see [CHANGELOG.md](../CHANGELOG.md) and [Migrating to 18.0](../docs/src/content/docs/reference/migrating-18.md). New: `acme.renew_before_days` stops being the renewal trigger (ARI decides; the setting becomes a fallback); MCP clients announcing 2025-06-18 or later can no longer send JSON-RPC batches. Plus the streaming request body from the milestone above.

**Done when:** CI green on all feature sets, h2spec 146/146, ACME e2e against Pebble with ARI, MCP server passing the official conformance scenarios for 2025-11-25, the three agent-ready usages (#60, #61; #62 if it makes the release) each with a runnable example or recipe, a fresh benchmark run, release artifacts signed with an SBOM and provenance attestation, migration page updated.

## 18.1.0 — Operations

Close the everyday gaps an operator meets on day one: compression peers all have, logs that land in a file, and config changes that apply themselves.

- **#7** ✅ *main: 908477f6 — br/zstd negotiated by q-value (zstd first per response, brotli first for cached static files), fast levels per response (br 4, zstd 3), level 6 cached per file version, precompressed siblings, per-encoding ETags; `text-br`/`text-zstd` scenarios; same-run A/B within noise on 3 CPUs; fuzzing found an `Accept-Encoding` panic present since 17.x (fixed)* **brotli and zstd** — on-the-fly encoding and precompressed `.br` / `.zst` siblings, q-value negotiation, cached per file version like gzip. Every peer has it; gzip-only is a visible gap.
- **#8** ✅ *main: 44b9893a — targets websocket_frame, dns_srv_response, redis_reply, cookie_header (60 s clean each); found and fixed: WebSocket frame payload allocated at the claimed length (process abort), same for RESP bulk strings, DNS SRV RDATA panic; smuggling corpus pinned by a framing test* **Fuzz the remaining untrusted-input parsers** — WebSocket frames, DNS SRV responses, Redis (RESP) replies, `CookieJar::parse`; add a CL/TE smuggling corpus to the request fuzzer.
- **#9** ✅ *main: 30ba4e80 — `access_log` + size rotation (`access_log_max_size`, `access_log_keep`) + SIGUSR1 reopen (only when a file is configured); systemd page with a hardened unit (systemd-analyze security 1.4 OK); `ci/e2e/access_log.sh`* **Access log to a file** with rotation (or reopen on `SIGUSR1`), plus a hardened systemd unit. The docs already promise rotation ("Coming Soon").
- **#10** ✅ *main: d4043018 — `watch = true` polls the config and every cert/key once a second, reloads after a settled change through the SIGHUP path; covered in `ci/e2e/config_reload.sh`* **Reload when the config file changes** — watch `rws.config.toml` and the certificate files and apply the same validated reload as `SIGHUP`. Caddy (`--watch`), Ferron and Traefik's file provider do this; rws needs a signal or a restart.

**Done when:** brotli/zstd cached like gzip and covered by the static benchmark (`text-br`, `text-zstd` scenarios); log rotation tested across `SIGUSR1` and size limits; file-watch reload covered by `ci/e2e/config_reload.sh`; new fuzz targets in the weekly fuzz job.

## 18.2.0 — Getting started

With a few hundred downloads per quarter, discoverability is the bottleneck: runnable examples, an agent guide, and installation the way people expect it.

- **#11** ✅ *main: examples `hello`, `rest_api`, `static_site`, `reverse_proxy`, `sse_ticker`, `upload` (each tested; SSE and a 5 MB streamed upload checked live), `AGENTS.md`, page getting-started/examples, CI tests with `serde macros`* **`examples/` and `AGENTS.md`** — runnable Cargo examples (the prerequisite for demo apps and doc tests) and an agent guide for people building on the crate. Discoverability, not features, is the bottleneck.
- **#26** ✅ *main: release job `chart` pushes oci://ghcr.io/bohdaq/charts/rws:<tag> (first publish at 18.2.0); Ingress + ServiceMonitor templates; adapter rules in `helm/prometheus-adapter-values.yaml` with a `Pods` metric; found and fixed: TLS-mode probes/scrape used plain HTTP (pods never ready); CI `helm.yml`: lint, kubeconform, kind install TLS on/off* **Publish the Helm chart** (OCI) with Ingress / ServiceMonitor templates, and prometheus-adapter rules for the HPA example (the example references a metric nothing exports under that name).
- **#27** ✅ *main: deb/rpm via nfpm (`packaging/linux`, release job `packages`), signed apt/dnf repos at rws8.tech/apt and /rpm (need the PACKAGES_GPG_KEY secret), `rws --windows-service=<dir>`, winget manifests + nixpkgs expression (submitted after the release tag); CI `packaging.yml`: deb/rpm and repo installs on systemd hosts, a real Windows service, winget validate+install, nix build with tests; found: reinstall after removal left the service disabled (fixed)* **OS packages** — deb / rpm repositories, a Windows service, nixpkgs / winget. Caddy and nginx ship OS packages; rws has Homebrew, Docker and archives.

**Done when:** `cargo run --example …` works for every example and CI builds them; the Helm chart installs from an OCI registry; deb/rpm packages install and start a systemd service in a CI container.

## 18.3.0 — gRPC and HTTP/2 upstreams

Make the HTTP/2 upstream path as good as the HTTP/1.1 one, so gRPC works end to end, and accept traffic from L4 load balancers.

- **#13** ✅ *main: pooled HTTP/2 upstreams on their own runtime, bodies streamed both ways, trailers to h2/h3 clients (and `Content-Length: 0` for bodiless HTTP/1.1 responses, found by the browser test), `te` forwarded, `tls_ca_file` + client certificates on HTTP/1.1, HTTP/2 and gRPC upstreams, config `grpc` action over HTTP/2; verified against tonic (unary, server/client/bidi streaming, status after messages, trailers-only errors; cleartext and mTLS upstreams) in `ci/e2e/grpc.sh`* **HTTP/2 upstream path at parity with HTTP/1.1** — `grpc-status` trailers (dropped today), streaming bodies (only unary gRPC works), connection pooling (a new handshake per request), `tls_ca_file`, and upstream client certificates on both paths (old "upstream client certs" item).
- **#14** ✅ *main: `proxy::GrpcWebLayer` (binary and `-text`, streamed both ways, trailers as the last frame), automatic in the config `grpc` action; e2e: headless Chromium via Playwright, fetch over HTTP/1.1 and HTTP/2 (`ci/e2e/grpc_web.sh`)* **gRPC-web bridge** — browsers speak gRPC-web; translate it to gRPC upstreams (Envoy, Pingora 0.4). Needs #13.
- **#20** ✅ *main: `server::proxy_protocol` (v1/v2, `proxy_protocol = true`, plain + TLS + http1 build, fuzz target), h2c prior knowledge on the plain listener (peek for the preface); e2e: `ci/e2e/proxy_protocol.sh` behind HAProxy (v1/v2, plain/TLS), h2c with `grpcurl -plaintext` in `ci/e2e/grpc.sh`* **Inbound PROXY protocol v1/v2 and h2c** — client addresses behind L4 load balancers; cleartext HTTP/2 for gRPC.

**Done when:** Streaming gRPC (client, server and bidirectional) and `grpc-status` trailers verified against a tonic server; gRPC-web verified from a browser client; PROXY protocol v1/v2 tested behind HAProxy; h2c tested with `grpcurl -plaintext`.

## 18.4.0 — Certificates

Every certificate a user can ask for: wildcards, TLS-ALPN-01, on-demand issuance for customer domains, and replicas that share one set of certificates.

- **#17** ✅ *main: TLS-ALPN-01 (temporary acceptor at startup, live resolver at renewal), DNS-01 via RFC 2136 + TSIG, Cloudflare, Route 53 (SigV4) and an exec hook, wildcard certificates, DNS-PERSIST-01 (record checked, never changed); e2e: Pebble (TLS-ALPN-01 issuance + renewal), Pebble + BIND (`ci/e2e/acme_dns01.sh`: wildcard, BADSIG reported, exec hook; `ci/e2e/acme_dns_api.sh`: Cloudflare/Route 53 stand-ins with signatures checked, DNS-PERSIST-01 wildcard)* **More ACME challenges** — TLS-ALPN-01; DNS-01 (RFC 2136, Cloudflare, Route 53, an exec hook) with wildcard certificates (`rws --check` rejects wildcards today); then DNS-PERSIST-01. nginx, HAProxy and Caddy cover these.
- **#18** ✅ *main: replicas share certificates and HTTP-01/TLS-ALPN-01 challenges through Redis (`[acme] redis`; one order per certificate via a lock, renewals adopted by every replica); e2e: two replicas + Pebble (`ci/e2e/acme_shared.sh`); on-demand TLS (`[acme] on_demand` + `on_demand_ask`, issuance during the first handshake via `LazyConfigAcceptor`, one order per name, refusals held off; e2e: Pebble + BIND + ask endpoint, `ci/e2e/acme_on_demand.sh`)* **On-demand TLS and shared certificate storage** — issue a certificate at the first handshake for names an "ask" endpoint allows (Caddy, for SaaS custom domains), and keep certificates and challenge tokens in Redis so replicas share them (Ferron v3, HAProxy 3.3).

**Done when:** DNS-01 and TLS-ALPN-01 issuance against Pebble in CI (DNS-01 with an RFC 2136 test server); wildcard certificates accepted by `rws --check`; two replicas sharing certificates through Redis without double issuance.

## 18.5.0 — MCP 2026

Speak the current MCP revision (2026-07-28) next to 2025 clients on the same endpoint, prove it with the conformance suite, and add the streaming primitives the gateway release needs.

- **#15** ✅ *main: dual-era endpoint — stateless 2026-07-28 (validation, `server/discover`, result decoration, caching hints, `subscriptions/listen`, MRTR with signed `requestState`, request-scoped progress/logs) beside session-based 2024-11-05…2025-11-25; the Tasks extension (`ctx.start_task`, `tasks/get|update|cancel`) and `Mcp-Param-*` validation; the 2026-07-28 suite passes every check (196)* **MCP 2026-07-28 alongside 2025** — `server/discover`, per-request `_meta` version, multi-round-trip requests, `subscriptions/listen`, `Mcp-Method` / `Mcp-Name` headers, `ttlMs` / `cacheScope`, deterministic ordering, the new error codes, and the Tasks extension for async tools. Serve old and new clients on one endpoint ([Cloudflare's approach](https://blog.cloudflare.com/mcp-v2/)).
- **#16** *server suite in CI per protocol version, pass rate in each run's summary: 0.1.16 (2025-11-25: 28/31) and 0.2.0-alpha.12 `--requirements` 2025-11-25 (69/72, elicitation baselined) and 2026-07-28 (all scored scenarios). The client suite remains (rws's MCP client is the gateway's upstream side)* **MCP conformance suite in CI** ([modelcontextprotocol/conformance](https://github.com/modelcontextprotocol/conformance)) with the pass rate published per protocol version.
- **#24** ✅ *main: `Sse::stream()` + `SseSender` (bounded, keep-alive, MCP `GET /mcp` uses it); WebSocket routes via `WebSocket::upgrade` / `.websocket()` (whole messages, pings, close handshake, a sender for other threads; async and `http1` builds); e2e `ci/e2e/realtime.sh` with Node clients (plain/TLS, shared/per-core, SSE over HTTP/2)* **Streaming SSE writer and router-level WebSocket upgrade** — the `stream_pipe` primitive exists; the `Sse` builder still buffers, and WebSocket needs its own accept loop.

**Done when:** Conformance suite in CI with a published pass rate per protocol version; a 2025-06-18 client and a 2026-07-28 client served by one `/mcp` endpoint; SSE writer used by the MCP server and documented.

## 18.6.0 — Proxy parity and observability

Everything the library proxy can do, configurable from `rws.config.toml`, and metrics in the shapes dashboards expect.

- **#21** ✅ *main: `weights` (smooth WRR, proportional random/least-connections), `[upstream.sticky]`, `[upstream.circuit_breaker]` (passive health, 503 + Retry-After when all are out), `[upstream.retry]` (idempotent + buffered only), `rws_upstream_*` metrics; `rws --check` validation; e2e `ci/e2e/proxy_policies.sh`* **Config-file proxy parity** — weighted load balancing, cookie stickiness, circuit breaker / retries, passive health checks, per-upstream metrics. These exist as library types but are "planned" in config mode.
- **#25** ✅ *main 6911c5a9, 02933fe0, b60beffc, b99f822e, 0bc5426b, 8d38c519: `http_server_request_duration_seconds` (server core, all protocols) + protobuf `/metrics` with native histograms, verified by Prometheus 3.15 (`ci/e2e/native_histograms.sh`); OTLP metrics export + binary OTel wiring from `OTEL_*`, verified by otelcol-contrib 0.162 (`ci/e2e/otlp.sh`); TLS handshake (protocol/cipher/group) and certificate-expiry metrics, rustls `prefer-post-quantum` restored; W3C Baggage + automatic client spans (HTTP client, proxies, DB) with cross-thread `Context`, random trace/span IDs, propagation verified through a proxied request in `ci/e2e/otlp.sh`; same-run bench A/B vs 6c558422 (run 40552230950) within noise* **Observability** — OpenTelemetry HTTP semantic-convention metrics (`http.server.request.duration`), OTLP metrics export, Prometheus native histograms, W3C baggage and automatic child spans (promised in the docs), negotiated TLS group and certificate-expiry metrics.

**Done when:** Each config-mode feature has a `rws --check` validation and an e2e test; OpenTelemetry HTTP metrics verified in an OTLP collector; native histograms scraped by Prometheus 3.

## 18.7.0 — Framework

Bring the library side up to what Rust web developers expect: typed OpenAPI, tower interop, and a project generator.

- **#22** ✅ *main: `Operation` (body/response/query/path schemas from `JsonSchema`), OpenAPI 3.1.0, `examples/mcp_inventory.rs` described, validated by openapi-spec-validator 0.9.0 (`ci/e2e/openapi.sh`)* **OpenAPI with request/response schemas** from serde types (paths only today), sharing one schema derivation with MCP `outputSchema`.
- **#23** ✅ *main: adapters only, decided 2026-10-27 — Tower stays out of rws's own request path (performance): `tower` feature with `mount`/`TowerApp`, `TowerLayer`, `TowerService`, dedicated `rws-tower` runtime; `examples/tower.rs` mounts tonic-health + a tower-http layer, `ci/e2e/tower.sh` (grpcurl over h2c: unary, trailers-only, server streaming)* **Opt-in tower compatibility** — tower `Layer` / `Service` usable as rws middleware and handlers, and an rws `Application` usable as a tower `Service`; opens tower-http and tonic (gRPC serving) without building them.
- **#42** ✅ *main: `{param}` syntax (0ac36e80); `starters/api` + `starters/mcp` (`ci/e2e/starters.sh`); `agents/` skill + generated Cursor/Copilot files (`ci/gen_agents.py`); llms files per release on the site* **Scaffolding and routing DX, for people and coding agents** — `{param}` path syntax next to `:param` (what axum 0.8 users expect); starters that are standalone projects (`starters/`, each with its own `AGENTS.md`, copyable with `cargo generate`); an agent skill/rules pack (`agents/`); `llms.txt` per release. *Changed 2026-10-27:* no `rws new` / `cargo-rws` — for agent-driven development a scaffolder adds little next to accurate, versioned context; the docs MCP server for that context is in [WEBSITE.md](WEBSITE.md#hosting-and-the-docs-mcp-server), for the hosted site.

**Done when:** OpenAPI output validated with an OpenAPI 3.1 validator for an example app; a tower-http layer and a tonic service mounted in an example; each starter builds and passes its own tests in CI against the checkout; the agent pack and per-release `llms.txt` are published with the release.

## 18.8.0 — Kubernetes Gateway API

Give clusters leaving Ingress-NGINX a place to go.

- **#19** *in progress — plan and status: [GATEWAY_API.md](GATEWAY_API.md); all five route types (HTTP, GRPC, TLS, TCP, UDP), every standard-channel extended feature at Gateway API v1.6.3, shared and provisioned modes* **Kubernetes Gateway API controller** — HTTPRoute, GRPCRoute, TLSRoute, TCPRoute, UDPRoute, with conformance reports. Ingress-NGINX was retired in March 2026 with ["no drop-in replacement"](https://www.kubernetes.io/blog/2026/01/29/ingress-nginx-statement/); rws only watches Ingress.

**Done when:** Conformant reports (shared and provisioned modes, every profile and claimed feature passing at Gateway API v1.6.3) accepted into kubernetes-sigs/gateway-api and rws listed on the implementations page.

## 18.9.0 — AI gateway

Put MCP servers and LLM providers behind rws with one policy point.

- **#28** **MCP gateway mode** — the full gateway on #62's foundation: federate upstream MCP servers (`prefix__tool` names), per-identity tool ACLs, OAuth at the edge with upstream credential injection, per-tool rate limits on `Mcp-Name`, `tools/list` caching, audit log, bridging 2025 clients to 2026 servers. After #13, #15, #24.
- **#41** **LLM proxy middleware** — token accounting from `usage`, time-to-first-token metrics, token-based rate limits and budgets, provider fallback, virtual keys (Envoy AI Gateway, Kong, LiteLLM).

**Done when:** Federation of two MCP servers with prefixed tools and per-identity ACLs in an e2e test; streaming LLM responses proxied with token usage recorded in metrics.

## 18.10.0 — Admin and extensibility

Change a running server without editing files, and extend it without recompiling.

- **#29** **Admin API, then UI** — authenticated runtime config, backend drain, certificate renew / reload, status; a UI on top (plan: `archive/ADMIN_ROADMAP.md`, reordered API-first).
- **#40** **WASM plugins** — Wasmtime-hosted middleware with a small pass / modify / respond ABI and no ambient capabilities (Traefik v3 plugins, nginx njs as the scripting counterpart).

**Done when:** Admin API authenticated by default and covered by `rws --check` and e2e tests; a WASM plugin example that rewrites a header, with a CPU/memory limit test.

## 19.0.0 — Breaking changes

- **#63** **Proxy response pass-through.** On the 18.0.0 benchmarks rws leads nginx in most scenarios, but on proxy-1k it is level or slightly behind (EPYC 7763: 48.7 vs 45.3 µs CPU per request; the one Intel run: −2% requests per second, +9% CPU per request). The remaining cost is rws parsing every upstream response into a `Response` and writing it out again, about 7% of a proxied request's CPU in the per-core profile. Carry a simple upstream head through unparsed instead, keeping what the parse does for safety: dropping hop-by-hop and duplicate `Content-Length` headers and rejecting CR/LF. That needs a new field on `Response`, whose fields are all public and built by struct literal, so it breaks apps; mark `Response` `#[non_exhaustive]` (with constructors) in the same release so later fields don't. Judge it by same-run A/B against 18.x on proxy-1k. *Planned 2026-10-26.*

## Unscheduled

Valuable, but blocked on an upstream dependency or waiting for a user who needs it. Picked into a release when that changes.

- **#32** **Authorization API** — `RbacLayer` / `check(user, object, action)` on top of the SSO claims.
- **#33** **i18n** — message catalogs and an `Accept-Language` extractor.
- **#34** **WASM shim remainder** — a `wasm32-wasip2` CI smoke test first, then stateful middleware, then WASI 0.3 ([WASM_SHIM.md](WASM_SHIM.md)).
- **#35** **`storage-gcs`** — the third cloud backend next to S3 and Azure.
- **#36** **Encrypted Client Hello** — once rustls has a server-side API; publish ECH configs in DNS HTTPS records (Caddy 2.10 automates it; nginx 1.29 and HAProxy 3.3 ship it).
- **#37** **HTTP/3 to upstreams** (HAProxy 3.3, experimental).
- **#38** **103 Early Hints and compression dictionaries** — Early Hints from config or upstream `Link` headers (RFC 8297); `dcb` / `dcz` dictionary compression (RFC 9842, Pingora).
- **#39** **Request mirroring, backend drain, disk cache with a purge API** — Traefik and Envoy have mirroring and drain; the cache is in-memory today.

## Not planned (for 18.x)

- **HTTP/2 and HTTP/3 server push** — browsers dropped it.
- **OCSP stapling for Let's Encrypt certificates** — Let's Encrypt [ended OCSP](https://letsencrypt.org/2024/12/05/ending-ocsp.html) in August 2025.
- **Removing the MCP features deprecated by 2026-07-28** (sampling, roots, logging, the GET stream) — not before their 12-month runway ends in July 2027, so in 19.0 at the earliest.
- **Semantic caching, guardrails and billing** in an LLM proxy — plugin territory.
- **ROS 2 / MQTT / CoAP bridges, Arrow / Parquet, `SqliteRateLimiter`** — dropped from the old roadmap; no demand evidence.
- Moved to [IDEAS.md](IDEAS.md): Kafka, GraphQL, a native gRPC server, `Csv<T>` and content negotiation, demo apps.

## Performance research — replacing tokio on the serving path

Not scheduled: research first, and only worth building if it shows a clear win.

- **#51** **A per-core event loop instead of tokio for the HTTP/1.1 and HTTP/2 serving path** — one `epoll` loop per core over its own `SO_REUSEPORT` socket (nginx's worker model), with handlers that may block still sent to a thread pool, and tokio kept for proxy upstreams, ACME, HTTP/3 and the async APIs unless the research says otherwise.
  - *Why:* on the fastest (Intel) benchmark runners nginx serves 6–8% more plain-HTTP requests per second although rws needs less CPU per request (run 40209996636). Profiles put the remainder in tokio's scheduler: idle workers park and are woken by futex (about 2 wake-ups per proxied request), which adds latency on fast CPUs. The opt-in `RWS_CONFIG_RUNTIME=per-core` (one `current_thread` runtime per core) narrows the gap there — proxy −4% CPU, +12% requests per second, small files +16% (run 40242045036) — but costs 5–17% more CPU per request on slower runners, so it stays opt-in.
  - *Research questions:* how much of tokio's per-request cost (wakers, task polling, timer wheel, I/O driver registration) a hand-written loop removes; whether a `current_thread` runtime with tuned `event_interval` gets the same (on the shared runtime it doesn't: `event_interval` 7 instead of 61 bought 5–11% more small-file requests per second for 2–7% more CPU per request, 200 changed nothing — runs 40244381005…40244386199); how `io_uring` compares to `epoll`; what it would take to keep `Application`/`Middleware` unchanged and TLS (rustls) working without `tokio-rustls`.
  - *Cost:* large — a second I/O stack to maintain next to tokio's, for a gain of a few percent on one class of hardware. Measure with the same-run A/B (`base_ref`, `base_runtime`) before committing to it.

**Done when (research):** a prototype serving static files and the proxy route, benchmarked in the same run against the shared and per-core tokio runtimes on fast and slow runners, with a decision recorded here.
