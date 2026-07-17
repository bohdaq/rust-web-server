[Read Me](../README.md) > [Spec](.) > TODO Final

# TODO Final — Consolidated Implementation Priority Order

Built by surveying all 20 docs in `spec/` and cross-checking each doc's claimed-open items
against the current codebase. Most backlog docs turned out to be stale/closed already; this
file is the single ranked list of what's actually left, in the order to tackle it.

**Closed/stale docs (no open items after verification):** `ARCHITECTURAL_CHANGES.md`,
`ROADMAP.md`, `K8S_ROADMAP.md`, `GAPS.md`, `GAPS_V2.md`, `GAPS_V3.md`, `MODEL.md`,
`LIKE_SPRING.md`, `PROXY_SERVER_CONFIG.md`, `SSO.md`, `FRAMEWORK_ROADMAP.md`, `MCP_TODO.md`,
`PLAN.md`, `REACT_GAPS.md`. `GAPS_V4.md` is nearly closed (two items remain, listed below).

---

## Tier 0 — Trivial, zero dependencies, do first

1. ✅ **Done** — **`Response::json()` / `Response::text()` helpers** (`AI_ADOPTION.md`) — tiny, but referenced by nearly every future example/demo below. Shipped in v17.106.0 (`src/response/mod.rs`); `Request::body_as_str`/`Request::query_param` from the same doc entry remain open.
2. **HPA custom-metrics doc example** (`GAPS_V4.md`) — docs only, closes the last open item in an otherwise-finished doc.
3. **`SECURITY.md`, `CHANGELOG.md`, `deny.toml`/cargo-deny policy** (`TODO.md` governance) — no code dependency, standard OSS hygiene, unblocks CI below.
4. **`prompts/SYSTEM_PROMPT.md`** (`AI_ADOPTION.md`) — docs only.
5. **Document real-time/embedded no_std scope boundaries** (`TODO.md` robotics) — docs only.

## Tier 1 — Small effort, unblocks later tiers

6. **CI pipeline (`.github/workflows/`)** (`TODO.md`) — every other item below benefits from automated `cargo test` gating.
7. **`examples/` directory (Cargo examples)** (`AI_ADOPTION.md` + `DEMOS_TODO.md` overlap) — the scaffold DEMOS_TODO's reference apps need; build before the demos.
8. **Dependabot alert triage** (`TODO.md`) — security hygiene, cheap.
9. **Upstream mTLS (`ca_file`/`client_cert`)** (`IDEAS.md`) — small remainder on an already-mostly-built feature.
10. **Access log rotation** (`TODO.md`) — small, operationally useful.

## Tier 2 — Medium effort, clear standalone value

11. **`storage-gcs`** (`GAPS_V4.md`) — closes 3-cloud storage parity, mechanical, follows the existing `storage-s3`/`storage-azure` pattern almost exactly — lowest-risk medium item on the list.
12. **`Csv<T>` extractor/responder** (`TODO.md` data/ML) — small-medium, and a prerequisite for content negotiation (#20).
13. **Admin UI Phase 1** (`RuntimeConfig` + `AdminAuthLayer` skeleton, `ADMIN_ROADMAP.md`) — gates all later admin phases; do before phases 2–7.
14. **RBAC/authorization framework (`RbacLayer`)** (`TODO.md` enterprise) — medium, no hard dependency, good enterprise-adoption lever.
15. **i18n (`src/i18n/`)** (`TODO.md`) — small, no dependency.

## Tier 3 — Demos (assembly work, needs Tier 1's `examples/`)

16. DEMOS_TODO Tier 1 apps — Task Tracker API, Realtime Chat, SaaS Auth Gateway, AI Assistant Backend, Media/Upload Service (Medium–Large each, but pure assembly of already-shipped features).
17. DEMOS_TODO Tier 2 apps — API Gateway, Polyglot Proxy, Production-Ops Reference App (Small–Medium, config-only).

## Tier 4 — Larger verticals, sequence-dependent

18. **Admin UI Phases 2–7** (mutable config API → proxy mgmt → JSON metrics → session inspector → SSE log tail → embedded HTML UI) — strictly sequential per `ADMIN_ROADMAP.md`, lowest business priority per `IDEAS.md` so it's fine for this to trail.
19. **SqliteRateLimiter** (`TODO.md`) — blocked today on the async-DbPool/sync-Middleware mismatch already noted in the doc; needs that resolved first, independent of anything else here.
20. **Content negotiation (JSON/CSV/Arrow)** (`TODO.md` data/ML) — depends on #12 and Arrow support existing.
21. **Kafka roadmap** (`KAFKA_ROADMAP.md`) — entirely unbuilt, 11 items with a hard sequential chain (connection config → producer → consumer → rebalance → DLQ, then outbox/dedup/metrics/schema-registry branch off that spine). Treat as one project, not scattered items.
22. **ROS/ROS 2 bridge** (`TODO.md` robotics) — "highest-leverage" per the doc's own ranking, builds on the existing WebSocket implementation. Do before MQTT/CoAP.
23. **MQTT**, then **CoAP** (`TODO.md` robotics) — comparable scope to the WebSocket work already shipped.
24. **Request-batching middleware for inference** (`TODO.md` data/ML) — medium, independent of the Arrow/CSV chain.

## Tier 5 — Large/strategic, needs an explicit decision first

25. **Native gRPC server** (`prost`/`tonic`) and **Arrow/Parquet/.npy support** — both large and both would be this crate's *first* third-party parsing dependencies, breaking the hand-rolled-everything pattern the rest of the codebase follows. Worth a deliberate go/no-go conversation before scoping, not just a backlog slot.
26. **WebSocket `permessage-deflate`** (Medium) then **WebSocket-over-HTTP/2 (RFC 8441)** (Large) — same subsystem, do deflate first.
27. **GraphQL adapter** (Large) — lowest urgency, no stated adoption pressure in the docs.
28. ✅ **Foundation + Phase 1 done** — **WASM/`wasm32-wasip2` shim** (Very large). `WASM_SHIM.md`: not a socket/thread port (infeasible — no `std::thread`, no `aws-lc-rs` on this target), but a `wasi:http/proxy` guest adapter (`rws-wasm-shim/`) reusing the existing `Application`/`Request`/`Response` seam, verified end-to-end against a real `wasmtime serve` process. Phase 2 (stateless middleware parity, streaming bodies, outbound HTTP) and Phase 3 (stateful-middleware-under-per-request-instantiation writeup) remain open.

## Deprioritized

- **HTTP/2+3 server push** — small effort but the docs themselves note low real-world value (cache-interaction problems, most clients ignore it).
