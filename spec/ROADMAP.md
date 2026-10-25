# Roadmap

Open work, in the order to tackle it. Carried over from `archive/TODO_FINAL.md`
(2026-07-17) with everything since shipped removed — CI, `SECURITY.md`,
`CHANGELOG.md`, `Response::json`/`text`, upstream private-CA trust
(`tls_ca_file`), the config `static` action and most of the WASM shim.
Unordered ideas live in [IDEAS.md](IDEAS.md); finished plans in [archive/](archive/).

## Small

1. **HPA custom-metrics example** — a `deployment/kubernetes.md` section scaling on `rws_*` metrics via prometheus-adapter.
2. **cargo-deny policy** (`deny.toml`) next to the existing `cargo audit` job.
3. **`examples/` directory** — Cargo examples; the scaffold the demo apps below need.
4. **Upstream client certificates** — `[[upstream]] client_cert` / `client_key` (mTLS to the backend); CA trust is done.
5. **Access log rotation** — size/time-based rotation for a file log target.

## Medium

6. **`storage-gcs`** — third cloud backend, same shape as `storage-s3` / `storage-azure`.
7. **`Csv<T>` extractor/responder** — prerequisite for content negotiation (#12).
8. **Admin UI phase 1** — `RuntimeConfig` + `AdminAuthLayer` (plan: `archive/ADMIN_ROADMAP.md`).
9. **`RbacLayer`** — role-based authorization middleware.
10. **i18n** — message catalogs + `Accept-Language` negotiation.
11. **Demo apps** — [DEMOS_TODO.md](DEMOS_TODO.md), after #3.

## Large / needs a decision first

12. **Content negotiation** (JSON / CSV / Arrow) — after #7.
13. **Kafka** — [KAFKA_ROADMAP.md](KAFKA_ROADMAP.md), entirely unbuilt.
14. **Admin UI phases 2–7** — sequential, after #8.
15. **`SqliteRateLimiter`** — blocked on async `DbPool` vs sync `Middleware`.
16. **ROS 2 bridge, then MQTT, then CoAP.**
17. **Native gRPC server, Arrow/Parquet** — would be the first third-party parsers in the crate.
18. **WebSocket `permessage-deflate`, then WebSocket over HTTP/2 (RFC 8441).**
19. **WASM shim remainder** — [WASM_SHIM.md](WASM_SHIM.md).
20. **GraphQL adapter.**

Deprioritized: HTTP/2 and HTTP/3 server push (little real-world value).
