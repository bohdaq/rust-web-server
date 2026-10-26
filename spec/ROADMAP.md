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

**Progress:** fixes land on branch `17.113.x` (from the `17.113.0` tag), each with a regression test that fails on 17.113.0, then are ported to `main` — all eight done on both (2026-10-26: cargo test, http1, all-features, MSRV 1.88, sso feature set, h2spec 146/146, acme_pebble, slow_clients, config_reload green on each); the version bump, tag, publish and advisory are the maintainer's.

**Done when:** a regression test per fix; the reproductions from the review fail against the patched binary; 17.113.1 published (crates.io, GitHub release, Docker Hub / GHCR, Homebrew) with a GitHub security advisory; the same fixes merged to `main`.

## 17.113.2 — HTTP conformance

Net-new RFC-conformance bugs from the 2026-10-26 review, each reproduced against a running build. Correctness, not new features, so they land before 18.0 (on `17.113.x` after the security patch, then `main`). The request-validation leniency overlaps the smuggling hardening in #43–#50; the rest is user-facing breakage.

- **#52** **gzip must not rewrite a partial body** — a `Range` request with `Accept-Encoding: gzip` is gzipped after slicing, and `Content-Range` is rewritten to the compressed size (`bytes=0-1200` of 3700 bytes → `Content-Range: bytes 0-37/37`). Don't compress a `206`/`Content-Range` body (RFC 9110 §8.4.1, §14.4). `src/compression/mod.rs`.
- **#53** **multipart/byteranges framing** — the response has no `Content-Length` and no `Transfer-Encoding`, so a keep-alive client can't delimit the body and hangs; the close delimiter is missing its trailing `--`; the boundary is a fixed literal (`String_separator`) that collides with matching content. Set a length, close the delimiter, and generate a unique boundary (RFC 9112 §6.3, RFC 2046 §5.1.1). `src/response/mod.rs`.
- **#54** **bodiless responses carry no body or Content-Length** — `204`, `304`, `HEAD` and `1xx` must send no content and no `Content-Length`; today `OPTIONS`→`204` sends `Content-Length: 3` (and on HTTP/2, a `DATA` frame — malformed per RFC 9113 §8.1.1). RFC 9110 §6.4.1, §15.3.5. `src/response/mod.rs`, `src/h2_handler/`.
- **#55** **percent-decode the request path** — the filesystem lookup uses the raw target, so `GET /my%20file.txt` 404s for `my file.txt`, and directory-listing links are emitted percent-encoded and then 404 (RFC 3986 §2.1). Decode once, after the smuggling checks, before the path is resolved (keep the `..`-confinement from #43). `src/url/mod.rs`, `src/app/controller/static_resource/`.
- **#56** **conditional requests and status codes** — honor `If-Range` (a stale validator must drop the `Range` and send `200`, not `206`); parse `If-None-Match` as a list with weak comparison; accept all three HTTP-date formats in `If-Modified-Since` (RFC 850 and asctime, not only IMF-fixdate); answer `405` + `Allow` for a known path with an unsupported method; put `Content-Range: bytes */len` on a `416`, and serve a `206` when at least one requested range is satisfiable. RFC 9110 §13, §14, §15.5.6. `src/app/controller/static_resource/`, `src/range/`, `src/http/date.rs`.
- **#57** **strict request validation** — reject a missing or duplicated `Host` (`400`), whitespace before a field colon, obs-fold continuation lines, and field names that aren't tokens; match the method case-sensitively; reject `HTTP/0.9` and `HTTP/2.0` on the HTTP/1.1 wire; evaluate `Connection` token by token (`close, TE` currently serves keep-alive). RFC 9112 §2.2–§3.2, §5.1, §9.6. `src/request/`, `src/server/framing.rs`, `src/server/http1_async.rs`.
- **#58** **CORS matching and exposed headers** — match a configured origin exactly, not by substring (`app.example.com` currently also matches `app.example.co`), and send `Access-Control-Expose-Headers` on the actual response, not only the preflight. Correctness, so it stays in the patch; the allow-all default change is #59 under 18.0. Fetch standard / RFC 6454. `src/cors/`.

**Done when:** a regression test per item that fails on the current build; the review's reproductions (gzip-on-206, the multi-range hang, `OPTIONS`→`204`, the `%20` 404) pass; h2spec still 146/146.

## 18.0.0 milestone — Breaking changes

All of 18.0's breaking changes land together, on `main` before 18.0.0 is tagged, so users migrate once and every later 18.x release stays additive. The changes already on `main` (Header `Cow` fields, default response headers, `rsa_key`) are listed under 18.0.0 below; this milestone adds the one still to build.

- **#31** **Streaming request bodies** — an upload API and multipart straight to storage (`Request.body` is a `Vec<u8>` today).
- **#30** **WebSocket permessage-deflate, then WebSocket over HTTP/2 and HTTP/3** (RFC 8441 / RFC 9220 extended CONNECT) — not breaking itself, but it builds on the streaming body type, so it follows #31 (it can ship in an 18.x release if 18.0 needs to go out first).
- **#59** **CORS allow-all off by default** — `cors_allow_all` currently defaults on, reflecting any origin with `Access-Control-Allow-Credentials: true`. Default it off (opt-in), so a server doesn't permit cross-origin credentialed reads unless configured. A behavior change existing deployments would feel, so it rides 18.0 rather than a patch.

**Breaking:** `Request.body` becomes a streaming body type, with a buffered helper (`body_bytes()`) for handlers that want the whole body. `cors_allow_all` now defaults off (#59).

**Done when:** a multi-GB upload streamed to S3 at constant memory; existing handlers ported through the buffered helper; the migration page covers it.

## 18.0.0 — Correct and current

The breaking release already on `main` (Header `Cow` fields, new default headers, in-crate RSA, the per-request CPU work), plus the fixes that can't wait: certificate renewal that survives Let's Encrypt's shorter lifetimes, an MCP server that speaks a current protocol, and the first signed, SBOM-carrying release.

- **#1** **ACME renewal rewrite** — ACME Renewal Information ([RFC 9773](https://www.rfc-editor.org/rfc/rfc9773.html)), profile selection (`classic` / `tlsserver` / `shortlived`), a two-thirds-of-lifetime fallback; drop the fixed 30-day `RENEW_BEFORE_DAYS` (`src/acme/mod.rs`). Let's Encrypt offers 45-day certificates now and makes them the default by [Feb 2028](https://letsencrypt.org/2025/12/02/from-90-to-45) (64 days from Feb 2027); a fresh 6-day certificate would look due for renewal immediately.
- **#2** ~~**Proxy request-header correctness**~~ — done in e5759f5e (one `Content-Length`, `Connection`-listed headers dropped, `X-Forwarded-For` extended).
- **#3** **MCP catch-up to 2025-11-25** — the server advertises protocol `2024-11-05` (`src/mcp/mod.rs`). Advertise what's implemented; accept JSON-RPC batches only from clients on ≤ 2025-03-26 (removed in [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/changelog)); serve RFC 9728 `/.well-known/oauth-protected-resource` with `WWW-Authenticate: … resource_metadata=` instead of the authorization-server document; check the token audience (RFC 8707); `403` on a bad `Origin`; require `MCP-Protocol-Version`; `outputSchema` / `structuredContent`, `title` / `icons`; input-validation failures as `isError` tool results ([2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/changelog)).
- **#4** **MCP session eviction** — TTL and a cap for sessions, subscriptions and SSE clients; today nothing is evicted.
- **#5** **Supply chain** — `deny.toml` + cargo-deny in CI, cargo-auditable release builds, a CycloneDX SBOM, GitHub build-provenance attestations, keyless cosign signatures on images. EU CRA reporting duties for manufacturers (who would ship rws) began 11 Sep 2026.
- **#6** **HTTP/2 DoS limits** — confirm the h2 version carries the MadeYouReset fix (CVE-2025-8671); set and test reset-rate, pending-accept-reset, CONTINUATION and header/idle timeouts (only two h2 limits are set in `src/h2_handler/mod.rs`).
- **#12** **Doc and benchmark truth** — mark `ctx.sample()`, roots and `logging/setLevel` as deprecated by MCP 2026-07-28; verify MCP SSE over HTTP/2 and HTTP/3 at runtime; re-run the HTTP/2 benchmark (the published 53.6k vs nginx 86.4k req/s predates the h2 work).

**Breaking:** Already on `main` — see [CHANGELOG.md](../CHANGELOG.md) and [Migrating to 18.0](../docs/src/content/docs/reference/migrating-18.md). New: `acme.renew_before_days` stops being the renewal trigger (ARI decides; the setting becomes a fallback); MCP clients announcing 2025-06-18 or later can no longer send JSON-RPC batches. Plus the streaming request body from the milestone above.

**Done when:** CI green on all feature sets, h2spec 146/146, ACME e2e against Pebble with ARI, MCP server passing the official conformance scenarios for 2025-11-25, a fresh benchmark run, release artifacts signed with an SBOM and provenance attestation, migration page updated.

## 18.1.0 — Operations

Close the everyday gaps an operator meets on day one: compression peers all have, logs that land in a file, and config changes that apply themselves.

- **#7** **brotli and zstd** — on-the-fly encoding and precompressed `.br` / `.zst` siblings, q-value negotiation, cached per file version like gzip. Every peer has it; gzip-only is a visible gap.
- **#8** **Fuzz the remaining untrusted-input parsers** — WebSocket frames, DNS SRV responses, Redis (RESP) replies, `CookieJar::parse`; add a CL/TE smuggling corpus to the request fuzzer.
- **#9** **Access log to a file** with rotation (or reopen on `SIGUSR1`), plus a hardened systemd unit. The docs already promise rotation ("Coming Soon").
- **#10** **Reload when the config file changes** — watch `rws.config.toml` and the certificate files and apply the same validated reload as `SIGHUP`. Caddy (`--watch`), Ferron and Traefik's file provider do this; rws needs a signal or a restart.

**Done when:** brotli/zstd cached like gzip and covered by the static benchmark (`text-br`, `text-zstd` scenarios); log rotation tested across `SIGUSR1` and size limits; file-watch reload covered by `ci/e2e/config_reload.sh`; new fuzz targets in the weekly fuzz job.

## 18.2.0 — Getting started

With a few hundred downloads per quarter, discoverability is the bottleneck: runnable examples, an agent guide, and installation the way people expect it.

- **#11** **`examples/` and `AGENTS.md`** — runnable Cargo examples (the prerequisite for demo apps and doc tests) and an agent guide for people building on the crate. Discoverability, not features, is the bottleneck.
- **#26** **Publish the Helm chart** (OCI) with Ingress / ServiceMonitor templates, and prometheus-adapter rules for the HPA example (the example references a metric nothing exports under that name).
- **#27** **OS packages** — deb / rpm repositories, a Windows service, nixpkgs / winget. Caddy and nginx ship OS packages; rws has Homebrew, Docker and archives.

**Done when:** `cargo run --example …` works for every example and CI builds them; the Helm chart installs from an OCI registry; deb/rpm packages install and start a systemd service in a CI container.

## 18.3.0 — gRPC and HTTP/2 upstreams

Make the HTTP/2 upstream path as good as the HTTP/1.1 one, so gRPC works end to end, and accept traffic from L4 load balancers.

- **#13** **HTTP/2 upstream path at parity with HTTP/1.1** — `grpc-status` trailers (dropped today), streaming bodies (only unary gRPC works), connection pooling (a new handshake per request), `tls_ca_file`, and upstream client certificates on both paths (old "upstream client certs" item).
- **#14** **gRPC-web bridge** — browsers speak gRPC-web; translate it to gRPC upstreams (Envoy, Pingora 0.4). Needs #13.
- **#20** **Inbound PROXY protocol v1/v2 and h2c** — client addresses behind L4 load balancers; cleartext HTTP/2 for gRPC.

**Done when:** Streaming gRPC (client, server and bidirectional) and `grpc-status` trailers verified against a tonic server; gRPC-web verified from a browser client; PROXY protocol v1/v2 tested behind HAProxy; h2c tested with `grpcurl -plaintext`.

## 18.4.0 — Certificates

Every certificate a user can ask for: wildcards, TLS-ALPN-01, on-demand issuance for customer domains, and replicas that share one set of certificates.

- **#17** **More ACME challenges** — TLS-ALPN-01; DNS-01 (RFC 2136, Cloudflare, Route 53, an exec hook) with wildcard certificates (`rws --check` rejects wildcards today); then DNS-PERSIST-01. nginx, HAProxy and Caddy cover these.
- **#18** **On-demand TLS and shared certificate storage** — issue a certificate at the first handshake for names an "ask" endpoint allows (Caddy, for SaaS custom domains), and keep certificates and challenge tokens in Redis so replicas share them (Ferron v3, HAProxy 3.3).

**Done when:** DNS-01 and TLS-ALPN-01 issuance against Pebble in CI (DNS-01 with an RFC 2136 test server); wildcard certificates accepted by `rws --check`; two replicas sharing certificates through Redis without double issuance.

## 18.5.0 — MCP 2026

Speak the current MCP revision (2026-07-28) next to 2025 clients on the same endpoint, prove it with the conformance suite, and add the streaming primitives the gateway release needs.

- **#15** **MCP 2026-07-28 alongside 2025** — `server/discover`, per-request `_meta` version, multi-round-trip requests, `subscriptions/listen`, `Mcp-Method` / `Mcp-Name` headers, `ttlMs` / `cacheScope`, deterministic ordering, the new error codes, and the Tasks extension for async tools. Serve old and new clients on one endpoint ([Cloudflare's approach](https://blog.cloudflare.com/mcp-v2/)).
- **#16** **MCP conformance suite in CI** ([modelcontextprotocol/conformance](https://github.com/modelcontextprotocol/conformance)) with the pass rate published per protocol version.
- **#24** **Streaming SSE writer and router-level WebSocket upgrade** — the `stream_pipe` primitive exists; the `Sse` builder still buffers, and WebSocket needs its own accept loop.

**Done when:** Conformance suite in CI with a published pass rate per protocol version; a 2025-06-18 client and a 2026-07-28 client served by one `/mcp` endpoint; SSE writer used by the MCP server and documented.

## 18.6.0 — Proxy parity and observability

Everything the library proxy can do, configurable from `rws.config.toml`, and metrics in the shapes dashboards expect.

- **#21** **Config-file proxy parity** — weighted load balancing, cookie stickiness, circuit breaker / retries, passive health checks, per-upstream metrics. These exist as library types but are "planned" in config mode.
- **#25** **Observability** — OpenTelemetry HTTP semantic-convention metrics (`http.server.request.duration`), OTLP metrics export, Prometheus native histograms, W3C baggage and automatic child spans (promised in the docs), negotiated TLS group and certificate-expiry metrics.

**Done when:** Each config-mode feature has a `rws --check` validation and an e2e test; OpenTelemetry HTTP metrics verified in an OTLP collector; native histograms scraped by Prometheus 3.

## 18.7.0 — Framework

Bring the library side up to what Rust web developers expect: typed OpenAPI, tower interop, and a project generator.

- **#22** **OpenAPI with request/response schemas** from serde types (paths only today), sharing one schema derivation with MCP `outputSchema`.
- **#23** **Opt-in tower compatibility** — tower `Layer` / `Service` usable as rws middleware and handlers, and an rws `Application` usable as a tower `Service`; opens tower-http and tonic (gRPC serving) without building them.
- **#42** **Scaffolding and routing DX** — `rws new` with generators and starters (Loco), `{param}` path syntax next to `:param` (what axum 0.8 users expect).

**Done when:** OpenAPI output validated with an OpenAPI 3.1 validator for an example app; a tower-http layer and a tonic service mounted in an example; `rws new` creates a project that builds and passes its own tests.

## 18.8.0 — Kubernetes Gateway API

Give clusters leaving Ingress-NGINX a place to go.

- **#19** **Kubernetes Gateway API controller** — HTTPRoute, then GRPCRoute / TLSRoute, with a conformance report. Ingress-NGINX was retired in March 2026 with ["no drop-in replacement"](https://www.kubernetes.io/blog/2026/01/29/ingress-nginx-statement/); rws only watches Ingress.

**Done when:** Gateway API conformance report (HTTPRoute core profile) published and listed on the Gateway API implementations page.

## 18.9.0 — AI gateway

Put MCP servers and LLM providers behind rws with one policy point.

- **#28** **MCP gateway mode** — federate upstream MCP servers (`prefix__tool` names), per-identity tool ACLs, OAuth at the edge with upstream credential injection, per-tool rate limits on `Mcp-Name`, `tools/list` caching, audit log, bridging 2025 clients to 2026 servers. After #13, #15, #24.
- **#41** **LLM proxy middleware** — token accounting from `usage`, time-to-first-token metrics, token-based rate limits and budgets, provider fallback, virtual keys (Envoy AI Gateway, Kong, LiteLLM).

**Done when:** Federation of two MCP servers with prefixed tools and per-identity ACLs in an e2e test; streaming LLM responses proxied with token usage recorded in metrics.

## 18.10.0 — Admin and extensibility

Change a running server without editing files, and extend it without recompiling.

- **#29** **Admin API, then UI** — authenticated runtime config, backend drain, certificate renew / reload, status; a UI on top (plan: `archive/ADMIN_ROADMAP.md`, reordered API-first).
- **#40** **WASM plugins** — Wasmtime-hosted middleware with a small pass / modify / respond ABI and no ambient capabilities (Traefik v3 plugins, nginx njs as the scripting counterpart).

**Done when:** Admin API authenticated by default and covered by `rws --check` and e2e tests; a WASM plugin example that rewrites a header, with a CPU/memory limit test.

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
