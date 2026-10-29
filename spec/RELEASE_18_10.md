# rws 18.10.0 — Admin and extensibility

Written 2026-10-29. The two 18.10 items from [ROADMAP.md](ROADMAP.md),
built tranche by tranche; this file is updated as each lands.

## Decisions

- One release, **18.10.0**, with both items (user, 2026-10-29).
- **Admin UI on lit-material** (user, 2026-10-29), the same component
  library as rws8.tech. It replaces the archived plan's single vanilla-JS file
  ([archive/ADMIN_ROADMAP.md](archive/ADMIN_ROADMAP.md)).
- **API first**: every UI action is an API call that `curl` and the MCP tools
  can make too. The binary's 20 MCP tools move onto the same `admin` module,
  so the two can't drift apart.
- **Its own listener** (user, 2026-10-29), off by default: `[admin] listen = "127.0.0.1:9901"`
  (Envoy's model). It is never mixed into the public routes, and a bearer
  token is required even on loopback (`token_env`). Without a token the admin
  listener refuses to start, and `rws --check` says why.
- **UI build**: sources in `admin-ui/` (Vite + lit + `@lit-material/*`,
  pinned). The built bundle is committed under `src/admin/ui/` and embedded
  with `include_bytes!`, so `cargo build` never needs Node. CI rebuilds it
  and fails on a diff, like `gen_llms.py --check`.
- **WASM plugins** (user, 2026-10-29): an optional `wasm-plugins` feature
  (wasmtime with cranelift), off in the default build (it adds several MB
  and build time) and on in the release binaries.
  Core modules, not the component model, behind a small ABI. CPU is limited
  by fuel and memory by `StoreLimits`. There are no WASI imports, so plugins
  get no ambient capabilities: they see only what the host passes in.

## Constraints

- `POST /admin/config/reload` on the main listener stays as it is; the
  admin API adds to it. The 20 MCP tool names in `main.rs` are a contract
  (`mcp/agent-tools.md`): refactor them onto `admin`, never rename them.
- The sync (`http1`) build gets the admin listener too (`Server::run_redirect`
  is the precedent for a second listener there).
- The UI bundle embeds no fonts (system font stack), and the published crate
  stays well under 10 MB; check `Cargo.toml`'s include list.
- CSP: check what lit-material needs (inline `style=` from components) before
  locking `style-src`; use a nonce or `'unsafe-inline'` for styles only if
  required, never for scripts.
- wasmtime: check its `rust-version` against MSRV 1.99, whether the `msrv`
  job builds all features, and `cargo audit` as soon as it's added. Disk:
  `cargo clean` before its first build, `du -sh target` after.

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | Admin core | `[admin]` (`listen`, `token_env`, `allowed_origins`) and `RWS_CONFIG_ADMIN_*`; listener started by `main` (async and sync builds); bearer check in constant time; `rws --check` errors on a listener without a token. `GET /admin/api/v1/status` (version, uptime, features, listeners, ready). Unit tests + e2e skeleton |
| 2 | Admin API: config and state | effective config (secrets hidden), reload (`POST …/reload` → `config_reload::request_reload`, result reported), rate limits, CORS, maintenance, IP blocklist, feature flags, cache stats/clear, routes, recent requests/errors, metrics as JSON. The MCP tools in `main.rs` call the same functions |
| 3 | Admin API: upstreams and certificates | list upstreams, backends and their health/breaker state; drain/undrain a backend (taken out of rotation, in-flight requests finish, health checks keep running); certificate inventory (names, notAfter, source: file/ACME/on-demand); ACME renew now; TLS reload |
| 4 | Admin API: logs | `GET …/logs/stream` (SSE tail of access-log lines via a bounded ring buffer next to `access_sink`) and `…/logs/recent` |
| 5 | Admin UI | `admin-ui/` app: login (token kept in `sessionStorage`), Status, Config, Upstreams (drain toggles, health), Certificates (renew), Rate limits / blocklist / maintenance / flags, Metrics (charts per the dataviz rules), Logs (live tail, pause). Embedded bundle served at `/admin/`, CSP locked to self. Playwright e2e against the binary |
| 6 | WASM plugins | `wasm-plugins` feature: `WasmLayer` (`Middleware`) + `[route.middleware.wasm]` (`module`, `fuel`, `memory_mb`, `config`); ABI `rws_on_request` / `rws_on_response` with pass / modify headers / respond; module compiled once, instance per request from a pre-instantiated pool; trap, fuel or memory exhaustion → 500 and a metric, never a crash. Example plugin crate (`examples/wasm_plugin/`, `wasm32-unknown-unknown`) that rewrites a header; tests for the CPU and memory limits; fuzz the host's ABI decoding |
| 7 | Release | CHANGELOG, docs (`deployment/admin.md`, `features/wasm-plugins.md`, api.md rows, env vars), 18.10.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Status

| # | State |
|---|---|
| 1 | — |
| 2 | — |
| 3 | — |
| 4 | — |
| 5 | — |
| 6 | — |
| 7 | — |
