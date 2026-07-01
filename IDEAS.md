# IDEAS

Potential improvements and future directions for `rust-web-server`.
Items are grouped by theme; there is no implied priority order within a group.

---

## Streaming

**Streaming SSE** — The current `Sse` builder buffers the entire body before writing.
Add a `SseWriter` that holds a reference to the raw TCP stream and sends each event
immediately as it is produced.  Required for live AI token output, sensor feeds, and
long-running push scenarios.  The WebSocket module already documents the stream-takeover
pattern; SSE can use the same approach.

**WebSocket inside the router** — The WebSocket API today requires bypassing `Server::run`
and running a custom accept loop.  Expose a `ws_upgrade(stream, handler)` helper that
can be called from inside a normal router handler so WebSocket routes can live alongside
HTTP routes without a separate listener.

---

## Protocol

**gRPC / Connect** — HTTP/2 is already wired up.  Add gRPC-over-HTTP/2 framing (5-byte
length-prefixed protobuf messages, `application/grpc` content type, trailer-based status)
so the server can speak to gRPC clients without an external proxy.  Start with unary RPCs;
streaming RPCs follow the SSE/WebSocket stream-takeover model.

**HTTP/2 server push** — The `h2_handler` already negotiates HTTP/2 connections.  Add an
opt-in `push(path)` API on `Response` that triggers `PUSH_PROMISE` frames so assets can
be preemptively sent alongside an HTML page.

---

## Extractors and parsing

**`Json<T>` extractor** — When the `serde` feature is active, add a typed `Json<T: DeserializeOwned>`
extractor that parses the request body and returns a typed value (or a 400 response).  Mirrors
the pattern already used by `BodyText` and `Query`.

**Multipart form data** — No `multipart/form-data` parser exists.  Add one (no external crates,
consistent with the zero-dependency philosophy) to support file upload endpoints.  Output:
a `MultipartForm` extractor with named fields and file parts that expose a `Vec<u8>` body
and a content-disposition filename.

**Cookie signing and encryption** — The `cookie` module handles raw cookies.  Add
`SignedCookie` (HMAC-SHA256, already available via the `auth` feature) and `EncryptedCookie`
(AES-128-GCM) variants so session tokens and sensitive values can be stored client-side
safely.

---

## Middleware

**Request body size limit** — `RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES` caps the read
buffer but silently truncates.  Add an explicit `BodyLimitLayer` that returns `413 Content Too Large`
before reading the body when `Content-Length` exceeds a configured threshold.

**Request tracing IDs** — Add a `TraceIdLayer` that stamps every request with a
`X-Request-ID` UUID (or propagates an incoming one), attaches it to the request log entry,
and echoes it in the response header.

**Circuit breaker for the proxy** — `ReverseProxy` retries the next backend on connection
failure but has no memory of past failures.  Add a half-open circuit-breaker state machine
per backend: after N consecutive failures the backend is taken out of rotation for a
cooldown period, then probed with a single request before being restored.

---

## Observability

**Prometheus text format** — The `metrics` module already has atomic counters
(`REQUESTS_TOTAL`, `ERRORS_TOTAL`, `ACTIVE_CONNECTIONS`, `THREAD_POOL_QUEUED`).
Add a serializer that renders them in Prometheus text exposition format so `/metrics`
can be scraped by Prometheus without an external adapter.

**Structured JSON log format** — The current log format is plain text.  Add a
`RWS_CONFIG_LOG_FORMAT=json` option that emits each log line as a JSON object with
`ts`, `level`, `method`, `path`, `status`, `latency_ms`, and `request_id` fields for
log aggregation pipelines (Loki, Elasticsearch, Datadog).

**Per-route metrics** — Extend `MetricsController` (and the Prometheus endpoint) to
track request count and p50/p95/p99 latency per route pattern, not just server-wide
totals.

---

## Developer experience

**Admin web UI** — Serve a minimal HTML/JS dashboard at `/rws-admin` (behind
`BasicAuthLayer` by default) that wraps the existing MCP tools in a browser interface:
maintenance-mode toggle, IP blocklist editor, live metrics chart, and a recent-requests
table.  No external JS framework — keep it self-contained.

**`rwsctl` CLI** — A companion binary that speaks to the MCP `/mcp` endpoint so operators
can run `rwsctl maintenance on`, `rwsctl block-ip 1.2.3.4`, or `rwsctl config reload`
from the shell without needing an MCP-capable client.

**OpenAPI spec generation** — Let `AppWithState::route_entries()` emit enough metadata
(method, pattern, summary) for a `GET /openapi.json` controller to produce an OpenAPI
3.1 document.  Handler-level doc comments or a `#[route(summary = "…")]` attribute macro
can carry descriptions.

**Config validation at startup** — Validate all `rws.config.toml` fields at parse time
and print a human-readable error listing every unknown key and every value that fails
type or range checks, instead of silently ignoring them and falling back to defaults.

---

## Auth and security

**HTTP Message Signatures (RFC 9421)** — Add a `HttpSigLayer` for server-to-server
request signing and verification.  Builds on the HMAC-SHA256 already used in the `auth`
feature.

**API key rotation** — Extend `JwtLayer` / `BasicAuthLayer` with a multi-key validation
callback so secrets can be rotated without a restart: the new key is accepted immediately
and the old key is accepted during a configurable grace period.

**IP filter (allow-list mode)** — The `ip_filter` module exists but its scope is unclear.
Ensure `BlocklistLayer` can also run in allow-list mode (deny everyone except an explicit
set of CIDRs), useful for internal admin endpoints.

---

## Load balancing (proxy)

**`LeastConnections` strategy** — Track in-flight request counts per backend (using
`AtomicUsize`) and route new requests to the backend with the fewest active connections.

**`IpHash` strategy** — Hash the client IP to a consistent backend so sessions that do
not use a shared store land on the same upstream.

**`WeightedRoundRobin`** — Allow backends to carry a weight so traffic can be shifted
gradually during a rolling deploy.

---

## Persistence

**Pluggable session storage** — Sessions are currently in-memory and lost on restart.
Define a `SessionStore` trait and ship two implementations: `MemorySessionStore` (the
current behaviour) and `FileSessionStore` (JSON-serialised to a configurable directory).
An optional `SqliteSessionStore` behind a `sqlite` feature follows naturally.

---

## Distribution

**Minimal Docker image** — Publish a `Dockerfile` that builds the binary in a Rust
builder stage and copies only the static binary into a `scratch` image.  Target image
size under 10 MB for the `http1` feature set.

**Wasm middleware** — Allow middleware plugins compiled to Wasm (Wasmtime host) so users
can extend the server without native compilation or restarting the binary.  Define a
simple ABI: the Wasm module receives request bytes, returns a decision (pass / modify /
respond), and has no ambient capabilities.
