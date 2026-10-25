# Roadmap

Open work, in order. Rebuilt on 2026-10-25 from a review of the code, the
competitors (Caddy, nginx, HAProxy, Traefik, Envoy, Pingora, Ferron), the Rust
framework landscape, the MCP/AI ecosystem and standards changes.

The ordering rule: **Now** is deadline-driven or a correctness fix, and small;
**Next** is what peers treat as table stakes; **Later** is valuable but large
or depends on earlier items. rws has almost no outside users yet (5 stars, a
few hundred downloads per quarter, every issue filed by the maintainer), so
items are judged by whether they make rws something a newcomer can trust.

Unordered ideas live in [IDEAS.md](IDEAS.md); finished plans in [archive/](archive/).

## Now

1. **ACME renewal rewrite** — ACME Renewal Information ([RFC 9773](https://www.rfc-editor.org/rfc/rfc9773.html)), profile selection (`classic` / `tlsserver` / `shortlived`), a two-thirds-of-lifetime fallback; drop the fixed 30-day `RENEW_BEFORE_DAYS` (`src/acme/mod.rs`). Let's Encrypt offers 45-day certificates now and makes them the default by [Feb 2028](https://letsencrypt.org/2025/12/02/from-90-to-45) (64 days from Feb 2027); a fresh 6-day certificate would look due for renewal immediately.
2. ~~**Proxy request-header correctness**~~ — done in e5759f5e (one `Content-Length`, `Connection`-listed headers dropped, `X-Forwarded-For` extended).
3. **MCP catch-up to 2025-11-25** — the server advertises protocol `2024-11-05` (`src/mcp/mod.rs`). Advertise what's implemented; accept JSON-RPC batches only from clients on ≤ 2025-03-26 (removed in [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/changelog)); serve RFC 9728 `/.well-known/oauth-protected-resource` with `WWW-Authenticate: … resource_metadata=` instead of the authorization-server document; check the token audience (RFC 8707); `403` on a bad `Origin`; require `MCP-Protocol-Version`; `outputSchema` / `structuredContent`, `title` / `icons`; input-validation failures as `isError` tool results ([2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/changelog)).
4. **MCP session eviction** — TTL and a cap for sessions, subscriptions and SSE clients; today nothing is evicted.
5. **Supply chain** — `deny.toml` + cargo-deny in CI, cargo-auditable release builds, a CycloneDX SBOM, GitHub build-provenance attestations, keyless cosign signatures on images. EU CRA reporting duties for manufacturers (who would ship rws) began 11 Sep 2026.
6. **HTTP/2 DoS limits** — confirm the h2 version carries the MadeYouReset fix (CVE-2025-8671); set and test reset-rate, pending-accept-reset, CONTINUATION and header/idle timeouts (only two h2 limits are set in `src/h2_handler/mod.rs`).
7. **brotli and zstd** — on-the-fly encoding and precompressed `.br` / `.zst` siblings, q-value negotiation, cached per file version like gzip. Every peer has it; gzip-only is a visible gap.
8. **Fuzz the remaining untrusted-input parsers** — WebSocket frames, DNS SRV responses, Redis (RESP) replies, `CookieJar::parse`; add a CL/TE smuggling corpus to the request fuzzer.
9. **Access log to a file** with rotation (or reopen on `SIGUSR1`), plus a hardened systemd unit. The docs already promise rotation ("Coming Soon").
10. **Reload when the config file changes** — watch `rws.config.toml` and the certificate files and apply the same validated reload as `SIGHUP`. Caddy (`--watch`), Ferron and Traefik's file provider do this; rws needs a signal or a restart.
11. **`examples/` and `AGENTS.md`** — runnable Cargo examples (the prerequisite for demo apps and doc tests) and an agent guide for people building on the crate. Discoverability, not features, is the bottleneck.
12. **Doc and benchmark truth** — mark `ctx.sample()`, roots and `logging/setLevel` as deprecated by MCP 2026-07-28; verify MCP SSE over HTTP/2 and HTTP/3 at runtime; re-run the HTTP/2 benchmark (the published 53.6k vs nginx 86.4k req/s predates the h2 work).

## Next

13. **HTTP/2 upstream path at parity with HTTP/1.1** — `grpc-status` trailers (dropped today), streaming bodies (only unary gRPC works), connection pooling (a new handshake per request), `tls_ca_file`, and upstream client certificates on both paths (old "upstream client certs" item).
14. **gRPC-web bridge** — browsers speak gRPC-web; translate it to gRPC upstreams (Envoy, Pingora 0.4). Needs #13.
15. **MCP 2026-07-28 alongside 2025** — `server/discover`, per-request `_meta` version, multi-round-trip requests, `subscriptions/listen`, `Mcp-Method` / `Mcp-Name` headers, `ttlMs` / `cacheScope`, deterministic ordering, the new error codes, and the Tasks extension for async tools. Serve old and new clients on one endpoint ([Cloudflare's approach](https://blog.cloudflare.com/mcp-v2/)).
16. **MCP conformance suite in CI** ([modelcontextprotocol/conformance](https://github.com/modelcontextprotocol/conformance)) with the pass rate published per protocol version.
17. **More ACME challenges** — TLS-ALPN-01; DNS-01 (RFC 2136, Cloudflare, Route 53, an exec hook) with wildcard certificates (`rws --check` rejects wildcards today); then DNS-PERSIST-01. nginx, HAProxy and Caddy cover these.
18. **On-demand TLS and shared certificate storage** — issue a certificate at the first handshake for names an "ask" endpoint allows (Caddy, for SaaS custom domains), and keep certificates and challenge tokens in Redis so replicas share them (Ferron v3, HAProxy 3.3).
19. **Kubernetes Gateway API controller** — HTTPRoute, then GRPCRoute / TLSRoute, with a conformance report. Ingress-NGINX was retired in March 2026 with ["no drop-in replacement"](https://www.kubernetes.io/blog/2026/01/29/ingress-nginx-statement/); rws only watches Ingress.
20. **Inbound PROXY protocol v1/v2 and h2c** — client addresses behind L4 load balancers; cleartext HTTP/2 for gRPC.
21. **Config-file proxy parity** — weighted load balancing, cookie stickiness, circuit breaker / retries, passive health checks, per-upstream metrics. These exist as library types but are "planned" in config mode.
22. **OpenAPI with request/response schemas** from serde types (paths only today), sharing one schema derivation with MCP `outputSchema`.
23. **Opt-in tower compatibility** — tower `Layer` / `Service` usable as rws middleware and handlers, and an rws `Application` usable as a tower `Service`; opens tower-http and tonic (gRPC serving) without building them.
24. **Streaming SSE writer and router-level WebSocket upgrade** — the `stream_pipe` primitive exists; the `Sse` builder still buffers, and WebSocket needs its own accept loop.
25. **Observability** — OpenTelemetry HTTP semantic-convention metrics (`http.server.request.duration`), OTLP metrics export, Prometheus native histograms, W3C baggage and automatic child spans (promised in the docs), negotiated TLS group and certificate-expiry metrics.
26. **Publish the Helm chart** (OCI) with Ingress / ServiceMonitor templates, and prometheus-adapter rules for the HPA example (the example references a metric nothing exports under that name).
27. **OS packages** — deb / rpm repositories, a Windows service, nixpkgs / winget. Caddy and nginx ship OS packages; rws has Homebrew, Docker and archives.

## Later

28. **MCP gateway mode** — federate upstream MCP servers (`prefix__tool` names), per-identity tool ACLs, OAuth at the edge with upstream credential injection, per-tool rate limits on `Mcp-Name`, `tools/list` caching, audit log, bridging 2025 clients to 2026 servers. After #13, #15, #24.
29. **Admin API, then UI** — authenticated runtime config, backend drain, certificate renew / reload, status; a UI on top (plan: `archive/ADMIN_ROADMAP.md`, reordered API-first).
30. **WebSocket permessage-deflate, then WebSocket over HTTP/2 and HTTP/3** (RFC 8441 / RFC 9220 extended CONNECT).
31. **Streaming request bodies** — an upload API and multipart straight to storage (`Request.body` is a `Vec<u8>` today).
32. **Authorization API** — `RbacLayer` / `check(user, object, action)` on top of the SSO claims.
33. **i18n** — message catalogs and an `Accept-Language` extractor.
34. **WASM shim remainder** — a `wasm32-wasip2` CI smoke test first, then stateful middleware, then WASI 0.3 ([WASM_SHIM.md](WASM_SHIM.md)).
35. **`storage-gcs`** — the third cloud backend next to S3 and Azure.
36. **Encrypted Client Hello** — once rustls has a server-side API; publish ECH configs in DNS HTTPS records (Caddy 2.10 automates it; nginx 1.29 and HAProxy 3.3 ship it).
37. **HTTP/3 to upstreams** (HAProxy 3.3, experimental).
38. **103 Early Hints and compression dictionaries** — Early Hints from config or upstream `Link` headers (RFC 8297); `dcb` / `dcz` dictionary compression (RFC 9842, Pingora).
39. **Request mirroring, backend drain, disk cache with a purge API** — Traefik and Envoy have mirroring and drain; the cache is in-memory today.
40. **WASM plugins** — Wasmtime-hosted middleware with a small pass / modify / respond ABI and no ambient capabilities (Traefik v3 plugins, nginx njs as the scripting counterpart).
41. **LLM proxy middleware** — token accounting from `usage`, time-to-first-token metrics, token-based rate limits and budgets, provider fallback, virtual keys (Envoy AI Gateway, Kong, LiteLLM).
42. **Scaffolding and routing DX** — `rws new` with generators and starters (Loco), `{param}` path syntax next to `:param` (what axum 0.8 users expect).

## Not planned

- **HTTP/2 and HTTP/3 server push** — browsers dropped it.
- **OCSP stapling for Let's Encrypt certificates** — Let's Encrypt [ended OCSP](https://letsencrypt.org/2024/12/05/ending-ocsp.html) in August 2025.
- **Semantic caching, guardrails and billing** in an LLM proxy — plugin territory.
- **ROS 2 / MQTT / CoAP bridges, Arrow / Parquet, `SqliteRateLimiter`** — dropped from the old roadmap; no demand evidence.
- Moved to [IDEAS.md](IDEAS.md): Kafka, GraphQL, a native gRPC server, `Csv<T>` and content negotiation, demo apps.
