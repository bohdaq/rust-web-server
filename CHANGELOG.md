# Changelog

Notable changes per release. Versions follow [semver](https://semver.org/):
a major version means a breaking change — see
[Migrating to 18.0](https://rws8.tech/reference/migrating-18/) for the next one.
Releases and binaries: [GitHub releases](https://github.com/bohdaq/rust-web-server/releases).

## Unreleased (18.8.0)

### Added

- Kubernetes Gateway API controller, in progress (#19, `gateway-api`
  feature; plan and status in `spec/GATEWAY_API.md`). So far: typed Gateway
  API v1.6 objects, watches through kube-rs, and the translation from the
  cluster's GatewayClasses, Gateways, HTTPRoutes and ReferenceGrants to the
  status the Gateway API requires (acceptance, listener conditions, attached
  routes, reference resolution), tested against the conformance suite's own
  manifests and a fake API server. It serves traffic: a Service per Gateway,
  HTTP/HTTPS listeners on container ports, HTTPRoute matching with the spec's
  precedence, weighted backends over EndpointSlices, the filters (header
  modifiers, redirects, URL rewrites, CORS), timeouts and retries, h2c and
  WebSocket backends, request mirroring, GRPCRoute, TLSRoute (passthrough and
  terminate), TCPRoute, UDPRoute, ListenerSet, Lease leader election, Helm
  values (`gatewayAPI.*`) and a CI job running the official conformance suite
  in kind.

### Changed

- The minimum supported Rust version is 1.99 (was 1.88): kube 4.2, under the
  `gateway-api` feature, and its dependencies need 1.89+, and the project
  now tracks the latest stable release.

## 18.7.0 — 2026-10-27

### Added

- OpenAPI request and response schemas (#22): `OpenApiConfig::operation(method,
  pattern, Operation)` describes an operation's summary, `operationId`, tags,
  JSON request body (`.body::<T>()`), responses (`.response::<T>(status, …)`,
  `.status(…)`), query parameters (`.query::<T>()`) and path parameter types,
  with schemas from `JsonSchema` — the derivation MCP tools use, so serde
  renames are followed. `examples/mcp_inventory.rs` describes its REST API;
  `ci/e2e/openapi.sh` validates the document with openapi-spec-validator.

- Tower interop, opt-in (#23, `tower` feature): `tower::mount` / `TowerApp`
  serve a Tower service (a tonic gRPC server, tower-http's `ServeDir`, an axum
  `Router`) from an rws app, bodies streaming both ways with gRPC trailers;
  `TowerLayer` uses a Tower layer (tower-http) as rws middleware;
  `TowerService` serves an rws app from hyper or axum. Tower futures run on a
  dedicated `rws-tower` runtime. rws's own request path is unchanged, and the
  default build gets no new dependency. `examples/tower.rs` (tonic-health +
  tower-http); `ci/e2e/tower.sh` calls it with grpcurl over h2c.

- `{param}` route syntax (#42): `/users/{id}` and `/files/{*path}` beside
  `/users/:id` and `/files/*path` — what axum 0.8 and OpenAPI use — in
  `Router`, `AppWithState`, `AsyncAppWithState` and WebSocket routes; `{{`
  and `}}` are literal braces. OpenAPI operations match routes in either
  spelling.

- Starters (#42): `starters/api` (JSON API + OpenAPI 3.1) and
  `starters/mcp` (MCP server), standalone projects with their own `AGENTS.md`,
  copyable with `cargo generate`; `ci/e2e/starters.sh` builds and tests them
  against the checkout on every push.
- An agent pack (#42): `agents/skills/rws/SKILL.md` (Agent Skills: Claude
  Code, Codex, …) with a Cursor rule and Copilot instructions generated from
  it (`ci/gen_agents.py`), published at `rws8.tech/agents/`.
- `llms.txt` / `llms-full.txt` on the site, at the root and per release under
  `/<major.minor>/` (they were only in the repository: the site's links to them
  were broken). Docs page: getting-started/ai-agents.

### Changed

- `/openapi.json` is OpenAPI 3.1.0 (was 3.0.3): `JsonSchema` emits JSON Schema
  2020-12, which 3.1 uses as is. `OpenApiConfig` has a new public field,
  `operations`; code that builds it with a struct literal must set it
  (`Vec::new()`), or use `OpenApiConfig::new`.
- `examples/mcp_inventory.rs` needs the `openapi` feature too.
- A route pattern segment written `{name}` is now a parameter; before 18.7.0
  it matched the literal text `{name}`. Write `{{name}}` for literal braces.

## 18.6.0 — 2026-10-27

### Added

- Config-file proxy parity (#21): an `[[upstream]]` takes `weights`
  (smooth weighted round-robin; proportional `random` and
  `least_connections`), `[upstream.sticky]` (a `HttpOnly` cookie naming the
  backend by a hash of its URL), `[upstream.circuit_breaker]` (passive health
  checks: failing backends leave rotation for `recovery_secs`; `503` +
  `Retry-After` when all are out) and `[upstream.retry]` (idempotent requests
  with a buffered body go to another backend on no response or a listed
  status). All checked by `rws --check`.
- Per-upstream metrics on `/metrics`: `rws_upstream_requests_total`,
  `rws_upstream_request_duration_seconds`, `rws_upstream_retries_total`,
  `rws_upstream_backend_up`, `rws_upstream_circuit_breaker_state`.
- `ci/e2e/proxy_policies.sh` runs these against real backends.
- `http_server_request_duration_seconds` (#25): the OpenTelemetry
  `http.server.request.duration` metric, recorded by the server core for every
  request on HTTP/1.1, HTTP/2 and HTTP/3, labelled by method, status, scheme
  and protocol version.
- Prometheus native histograms (#25): `/metrics` answers in the protobuf
  format when the scraper asks for it (Prometheus with
  `scrape_native_histograms`), with exponential buckets for
  `http_server_request_duration_seconds`. `ci/e2e/native_histograms.sh`
  checks it against Prometheus 3.15.
- OTLP metrics export (#25): `otel::metrics::start` / `start_from_env` push
  `http.server.request.duration` to an OpenTelemetry collector over
  OTLP/HTTP (JSON, cumulative) every `OTEL_METRIC_EXPORT_INTERVAL`, with a
  last export on shutdown. The `rws` binary exports metrics and traces (wrapping
  everything in `OtelLayer`) when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
  `OTEL_EXPORTER_OTLP_{METRICS,TRACES}_ENDPOINT` and `…_HEADERS` are honoured.
  `ci/e2e/otlp.sh` checks it against otelcol-contrib 0.162.
- Automatic client spans and W3C Baggage (#25): inside a request traced by
  `OtelLayer`, `http_client::Client`/`AsyncClient`, the HTTP/1.1 proxies and
  `DbPool`/`DbTransaction` statements get client spans, and outbound calls carry
  `traceparent` and `baggage` (`otel::Baggage`, `current_baggage`,
  `set_baggage`). `otel::Context` carries a span across threads (used for
  `AsyncAppWithState` handlers); `otel::detached_span` is a `Send` span for
  async code. `ci/e2e/otlp.sh` follows a proxied request into the collector.
  New fuzz target `otel_baggage`.
- TLS metrics (#25): `rws_tls_handshakes_total{tls_protocol_version,tls_cipher,tls_group}`,
  `rws_tls_handshake_failures_total` and
  `rws_tls_certificate_expiry_timestamp_seconds{name}` (default certificate,
  virtual hosts, on-demand names).

### Fixed

- HTTP/2 over cleartext (h2c) no longer sends `Strict-Transport-Security`.
- Trace and span IDs are random: they were a per-process counter (plus the
  start second), so two rws instances in one trace minted the same span IDs.
- OTLP span export no longer runs on the request thread when a batch fills
  (a background thread sends it; partial batches go out after 5 s), honours
  the endpoint's path and `https://`, and escapes span names and attribute
  values in its JSON.

### Changed

- rustls is built with `prefer-post-quantum` (its own default, which rws's
  `default-features = false` had dropped): the server now picks the
  X25519MLKEM768 hybrid key exchange when a client offers it (current Chrome,
  Firefox, Safari, curl with OpenSSL 3.5), instead of plain X25519, and
  outbound connections (proxy, ACME, HTTP client) offer it first.

- `proxy_config::UpstreamConfig` has new public fields (`weights`, `sticky`,
  `circuit_breaker`, `retry`); code that builds it with a struct literal must
  set them (`Vec::new()` / `None`).

## 18.5.0 — 2026-10-27

### Added

- MCP 2026-07-28 beside the session-based versions, on one endpoint (#15):
  stateless requests (`_meta` version and capabilities, `MCP-Protocol-Version`
  / `Mcp-Method` / `Mcp-Name` checked against the body: `-32602`, `-32020`,
  `-32022`, all `400`), `server/discover`, `resultType` and `serverInfo` on
  every result, `ttlMs`/`cacheScope` on cacheable ones (`.cache_hints`),
  `subscriptions/listen` (acknowledged, filtered, tagged), request-scoped
  progress and log messages (`ctx.log`, only with a `logLevel`), and multi
  round-trip requests: `McpContent::input_required(InputRequired::new()
  .elicit(..).sample(..).list_roots(..).state(..))`, `PromptReply` with
  `.prompt_with_context`, `ctx.input_response` / `elicitation_content` /
  `request_state` (HMAC-signed, bound to method, name, caller and 10 min;
  `.request_state_key` for replicas) / `client_supports`; a request for an
  undeclared capability is `-32021`. Removed methods are `404 -32601` there.
  Official conformance suite 0.2.0-alpha.12 in CI: every scenario 2026-07-28
  requires passes, and 2025-11-25 stays at its baseline (#16).
- The MCP Tasks extension (`io.modelcontextprotocol/tasks`, #15): a tool
  returns `ctx.start_task(|task| ...)` — a task handle for clients that
  declared the extension (`tasks/get`, `tasks/update`, `tasks/cancel`,
  `notifications/tasks` on `subscriptions/listen`), the work run inline for
  any other. `TaskContext::input` parks the task as `input_required` until
  `tasks/update` answers; `TaskError` maps to completed-with-error, failed or
  cancelled; `.task_required(name)` refuses non-declaring clients (`-32021`);
  `.task_ttl`, `.max_running_tasks` (256: each running task holds a thread).
  Tasks belong to their caller.
- `Mcp-Param-*` headers (MCP 2026-07-28, SEP-2243): tool parameters marked
  `x-mcp-header` must be mirrored in matching headers on `tools/call`
  (`400 -32020` otherwise); base64 header values must be canonical. The
  conformance suite's 2026-07-28 run now passes every check, 196, including
  the unscored ones.
- `resources/templates/list`: resources registered with a URI template are
  listed there; `resources/list` holds only concrete resources (it listed
  templates as resources before, which the suite flags).
- `crate::core::sha256`: SHA-256 and HMAC-SHA256, no dependency.
- Streaming Server-Sent Events (#24): `Sse::stream()` returns the response
  and an `SseSender`; events reach the client as they are sent, over
  HTTP/1.1, HTTP/2 and HTTP/3. `send` waits for room (bounded memory),
  `try_send` drops, `send_async` for async code; `Err(SseClosed)` once the
  client is gone, found by a keep-alive comment every 15 s. `SseStream`
  sets the buffer and interval. The MCP server's `GET /mcp` stream now uses
  it.
- WebSocket routes (#24): `AppWithState::websocket` / `Router::websocket`,
  or `WebSocket::upgrade(request, handler)` in any handler, hand the
  upgraded connection to the handler on a thread of its own.
  `WebSocketConnection::recv` returns whole messages (fragments joined,
  pings answered, the closing handshake done, protocol errors closed with
  1002/1007/1009); `WebSocketSender` sends from other threads. HTTP/1.1,
  plain and TLS, async and `http1` builds. `examples/realtime.rs`; e2e with
  Node's WebSocket and fetch (`ci/e2e/realtime.sh`).

### Fixed

- `SseEvent` encoding: a lone `\r` in `data`, or a line break in `id` or
  the event type, could add SSE fields of its own; `data` is now split at
  every line end and `id`/event type keep one line.

## 18.4.0 — 2026-10-27

### Fixed

- Per-core runtimes: startup returns only once every runtime's socket is
  bound. Between the setup listener closing and those sockets opening, a
  connection could be refused or reset; the gap is now microseconds, and
  "Started N per-core runtimes" is printed once the port is served again.
- A WebSocket Close frame whose reason isn't UTF-8 now fails the frame, as
  RFC 6455 §7.1.6 requires (text frames already did). It was decoded
  lossily, which could make the reason up to three times the frame payload
  limit (found by the `websocket_frame` fuzz target).

### Added

- TLS-ALPN-01 (RFC 8737, #17): `[acme] challenge = "tls-alpn-01"` gets and
  renews certificates on the HTTPS port alone — no port 80. At startup a
  temporary acceptor answers on the HTTPS socket; afterwards the TLS
  listener answers `acme-tls/1` handshakes with the challenge certificate
  (and only those). Tested against Pebble, first issuance and renewal.
- DNS-01 and wildcard certificates (#17): `[acme] challenge = "dns-01"`
  publishes `_acme-challenge` TXT records through `dns_provider = "rfc2136"`
  (a DNS UPDATE to the zone's primary server, signed with TSIG
  HMAC-SHA256/512 — BIND, Knot, PowerDNS, …) or `"exec"` (a program, for any
  DNS API), waits until the record is visible, and removes it afterwards;
  `*.example.com` in `domains` gets a wildcard certificate. Refusals are
  explained (REFUSED, NOTAUTH, TSIG BADSIG/BADKEY/BADTIME). Tested against
  Pebble and BIND (`ci/e2e/acme_dns01.sh`); the DNS reply parser — now one
  module, `dns`, shared with SRV service discovery — is fuzzed
  (`fuzz/fuzz_targets/dns_message.rs`).
- DNS-01 through the Cloudflare and Route 53 APIs (#17): `dns_provider =
  "cloudflare"` (an API token, `dns_api_token`) or `"route53"` (requests
  signed with AWS Signature V4 from `AWS_ACCESS_KEY_ID` /
  `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`); the zone by
  `dns_zone_id` or looked up; `dns_api_url` for a proxy or test double.
  Tested against stand-in APIs writing into BIND, signatures checked
  independently (`ci/e2e/acme_dns_api.sh`).
- `challenge = "dns-persist-01"` (#17, draft-ietf-acme-dns-persist): one
  TXT record at `_validation-persist.<domain>` you publish once — no port,
  no DNS credentials on the server, wildcards too. rws prints the exact
  record, checks it's visible before asking the CA, and never changes it.
  Tested against Pebble.
- On-demand TLS (#18): with `[acme] on_demand = true` and an
  `on_demand_ask` endpoint, rws orders a certificate at the first handshake
  for a name the endpoint allows (`GET <ask>?domain=<name>`, 2xx) — for
  customers' own domains. Refused names get the default certificate; one
  order per name, at most four at once; certificates kept under
  `.rws/acme/on-demand/`, loaded at startup and renewed. `rws --check`
  requires the ask endpoint and refuses DNS-01 with it. Tested against
  Pebble (`ci/e2e/acme_on_demand.sh`).
- Replicas sharing certificates through Redis (#18): with `[acme] redis`,
  one replica orders each certificate (a lock in Redis) while the others
  wait and install it, renewals reach every replica, and any replica answers
  HTTP-01 and TLS-ALPN-01 challenges another presented. Redis being down
  isn't fatal. Tested with two replicas against Pebble
  (`ci/e2e/acme_shared.sh`): one order per certificate.
- `[acme] challenge` (`RWS_CONFIG_ACME_CHALLENGE`: `http-01`, `tls-alpn-01`,
  `dns-01`) and the DNS-01 settings (`dns_provider`, `dns_server`,
  `dns_zone`, `dns_tsig_key`, `dns_tsig_secret`, `dns_tsig_algorithm`,
  `dns_exec`, `dns_propagation_secs`); `rws --check` validates them and
  accepts a wildcard domain with `dns-01`.

### Changed

- `acme::AcmeConfig` has new public fields (`challenge`, `dns`): code that
  builds it as a struct literal adds them (`AcmeConfig::from_env` is
  unaffected).
- Startup output no longer prints the values of settings whose names
  contain `SECRET`, `PASSWORD` or `TOKEN` ("(hidden)").

## 18.3.0 — 2026-10-27

### Added

- gRPC and HTTP/2 upstreams at parity with HTTP/1.1 (#13): `H2ReverseProxy`,
  `GrpcProxy` and the config file's `grpc` action keep one pooled,
  multiplexed connection per backend (a new handshake per request before);
  stream response bodies as they arrive and request bodies as they arrive, so
  server-streaming, client-streaming and bidirectional gRPC calls work; and
  pass the backend's trailers (`grpc-status`, `grpc-message`) to HTTP/2 and
  HTTP/3 clients, with `te: trailers` forwarded. A backend failing mid-call
  resets the client's stream.
- `Response::trailers` and `response::Trailers`: trailer fields sent after a
  body over HTTP/2 and HTTP/3 (HTTP/1.1 drops them).
- Upstream client certificates (mutual TLS): `tls_client_cert(cert, key)` on
  `ReverseProxy`, `H2ReverseProxy` and `GrpcProxy`, `[[upstream]]
  tls_client_cert_file` / `tls_client_key_file`, `UpstreamTls::new(ca,
  client)`. `tls_ca_file` now also applies to HTTP/2 and gRPC upstreams
  (`H2ReverseProxy::tls_ca_file`, `GrpcProxy::tls_ca_file`).
- `Middleware::answers_streamed_body`: a layer that answers a request itself
  can take its body as a stream, whatever the inner application says.
- gRPC-web bridge (#14): `GrpcWebLayer` translates `application/grpc-web`
  and `-text` requests to gRPC and the responses back — the status as the
  body's last frame — so browsers can call gRPC services through rws; the
  config file's `grpc` action does it on its own. Checked from headless
  Chromium over HTTP/1.1 and HTTP/2 (`ci/e2e/grpc_web.sh`).
- Inbound PROXY protocol v1 and v2 (#20): `proxy_protocol = true`
  (`RWS_CONFIG_PROXY_PROTOCOL`) takes the client address from the header an
  L4 load balancer (HAProxy, AWS NLB, GCP) prepends to each connection, on
  the plain and TLS listeners; connections without one are closed. Tested
  behind HAProxy (`ci/e2e/proxy_protocol.sh`); parser fuzzed
  (`fuzz/fuzz_targets/proxy_protocol.rs`).
- Cleartext HTTP/2 (h2c, prior knowledge) on the plain listener (#20): a
  connection opening with the HTTP/2 preface is served as HTTP/2 — gRPC
  without TLS, `grpcurl -plaintext`, `curl --http2-prior-knowledge`.
  Anything else is HTTP/1.1 as before.

### Changed

- `Response` and `proxy_config::UpstreamConfig` have new public fields
  (`trailers`; `tls_client_cert_file`, `tls_client_key_file`): code that
  builds them as struct literals adds them (`trailers: None`) — the
  constructors (`Response::new`, `text`, `json`, `get_response`) are
  unaffected. As with `stream_pipe` in 17.110.0.
- The config file's `grpc` action forwards over HTTP/2 (it used HTTP/1.1, so
  gRPC didn't work through it), and `grpc://`, `grpcs://` and `h2s://`
  backends pass `rws --check`.
- HTTP/2 requests keep `te: trailers` for the application (it was validated,
  then dropped).

### Fixed

- An HTTP/1.1 response with no body (a handler returning
  `Response::new()`, a gRPC-web trailers-only error) had no
  `Content-Length`, so a keep-alive client waited for a body until the
  connection closed. It now says `Content-Length: 0` (not for 1xx, 204 and
  304, which never have content, nor for streamed bodies).

## 18.2.0 — 2026-10-26

### Security

- zstd 0.14.3, zstd-safe 8.2.0, zstd-sys 2.1.2 (RUSTSEC-2026-0391, -0398,
  -0403, -0410, published 2026-10-16; taken although newer than the two-week
  rule, being security fixes). rws calls only `zstd::bulk::compress` on its
  own response bodies — a fresh context per call, no decompression, no
  WebAssembly build with zstd — so 18.1.0 isn't known to be exposed; the
  crate now requires zstd ≥ 0.14.3.

### Added

- Runnable examples (#11): `hello`, `rest_api`, `static_site`,
  `reverse_proxy`, `sse_ticker` and `upload` join the MCP examples in
  `examples/`, each with a test `cargo test` runs; listed on the new
  [Examples](https://rws8.tech/getting-started/examples/) page. CI now
  tests with the `serde` and `macros` features too, so `rest_api` and
  `mcp_inventory` run on every push.
- `AGENTS.md`: a guide for coding agents building an app on the crate.
- Helm chart published with each release (#26):
  `helm install my-rws oci://ghcr.io/bohdaq/charts/rws --version <version>`
  (chart version = rws version, image `ghcr.io/bohdaq/rws:<version>` by
  default). New optional `Ingress` and prometheus-operator `ServiceMonitor`;
  `helm/prometheus-adapter-values.yaml` serves the HPA's request-rate metric.
  CI lints the chart, checks it with kubeconform and installs it into kind
  with TLS on and off.
- deb and rpm packages (#27) for x86_64 and arm64 on each release: `/usr/bin/rws`,
  the hardened `rws.service` (now the one on the systemd page; it serves
  `/var/www/rws` and reads `/etc/rws/rws.config.toml`), started on port
  7878 on install, and signed apt and dnf repositories at
  `https://rws8.tech/apt` and `/rpm` (newest release; the rpm files are signed
  too). CI builds the repositories with a throwaway key and installs from
  them and from the files on systemd hosts. The release workflow now pushes
  the rws8.tech `docs` branch as one orphan commit per release (the branch
  is only a deploy target; packages would bloat its history).
- Windows service (#27): `rws --windows-service=<directory>` runs under the
  Service Control Manager; Stop and Shutdown end it gracefully
  (`server::request_shutdown`, which library users can call too). New page
  [Windows](https://rws8.tech/deployment/windows/). Adds the `windows-service`
  dependency on Windows only.
- winget manifests (`packaging/winget/generate.sh`) and a nixpkgs expression
  (`packaging/nix/package.nix`), checked in CI and submitted upstream per
  release (#27).

### Fixed

- Helm chart with `tls.enabled` (its default): liveness/readiness probes and
  the Prometheus scrape annotations used plain HTTP against the TLS port, so
  pods never became ready; they now use HTTPS (the Kubernetes docs' YAML had
  the same mistake). The HPA's custom metric was an `External`
  `rws_route_requests_total` nothing served; it is now the `Pods` metric
  `rws_requests_per_second` from the Prometheus Adapter rule. The chart's
  scrape annotations moved from `podAnnotations` to `metrics.podAnnotations`,
  and the Service port is named `http` (port 80) when TLS is off.
- MCP `ctx.sample()` / `ctx.list_roots()` called right after `initialize`,
  before the client had opened its `GET /mcp` stream, sent the request into
  nothing and timed out; they now wait for the stream within the same
  timeout. SDK clients open that stream in the background, so this made the
  conformance suite's `tools-call-sampling` scenario fail about one run in
  three.
- Release workflow: the docs-site job builds on Node 24 (Astro needs
  ≥ 22.23.4; the runner's Node 22 was older, so 18.1.0's site update failed).

## 18.1.0 — 2026-10-26

### Added

- brotli and zstd compression (#7): responses are compressed in the coding
  the client prefers by `Accept-Encoding` q-values (`br`, `zstd` or `gzip`),
  at the fast levels peers use per response (brotli 4, zstd 3); static files
  are compressed once per version per coding (brotli 6, zstd 6) and cached,
  and precompressed `app.js.br` / `.zst` / `.gz` siblings are served when at
  least as new as the file. `compression::apply_compression`, `negotiate`,
  `encode`, `Encoding`, `Level`; Cargo features `brotli` and `zstd` (default).
  New benchmark scenarios `text-br` and `text-zstd`.
- Reload on file change (#10): `watch = true` (`RWS_CONFIG_WATCH`) reloads
  when `rws.config.toml` or a configured certificate or key changes, with
  the same validation as `SIGHUP`; covered by `ci/e2e/config_reload.sh`.
- Access log to a file (#9): `access_log` (`RWS_CONFIG_ACCESS_LOG`) writes
  access-log lines to a file instead of stdout; `access_log_max_size` rotates
  it by size (`access.log` → `access.log.1` …, keeping `access_log_keep`),
  and `SIGUSR1` reopens it for logrotate (only when a file is configured;
  otherwise `SIGUSR1` keeps its default action). A hardened systemd unit is
  documented on the new [systemd](/deployment/systemd/) page.
- Fuzz targets for WebSocket frames, DNS SRV responses, Redis replies and
  `Cookie` headers, and a request-smuggling seed corpus (CL+TE, conflicting
  or listed `Content-Length`, obfuscated or folded `Transfer-Encoding`,
  bare-LF and oversized chunks) whose handling a test pins (#8).

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
- A compressed response kept the uncompressed body's strong `ETag`, so a
  cache could not tell the representations apart; it now carries the coding
  (`"x"` → `"x-gzip"`), static files included.
- An `Accept-Encoding` parameter starting with a multi-byte character
  (`gzip;aé`) panicked while looking for `q=`, ending that connection
  (found by the new `accept_encoding` fuzz target; the same code was in 17.x
  and 18.0.0).

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
