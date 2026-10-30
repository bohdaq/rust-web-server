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
| 1 | **done 2026-10-30**: `admin` module: `AdminConfig::from_env` (`RWS_CONFIG_ADMIN_LISTEN`/`_TOKEN_ENV` (default `RWS_ADMIN_TOKEN`)/`_ALLOWED_ORIGINS`, `[admin]` in the file), `AdminApi` (`Application`: Origin check → 403, constant-time bearer → 401 + `WWW-Authenticate` on every path, `GET /admin/api/v1/status`, `no-store`), `start` (own std listener thread, thread per connection, max 32, `Server::process` — same code in async and `http1` builds), `start_from_env` in all three mains (missing token: logged, server keeps serving). `rws --check`: bad `listen`, non-loopback warning, unset token error (online), bad origins, settings without `listen`. Tests: 7 unit (incl. TCP) + `ci/e2e/admin.sh` (6 checks) in the CI matrix as `admin`; docs `deployment/admin.md`, config-file, env-vars, api.md. 2137 default / 1961 http1 |
| 2 | **done 2026-10-30**: `admin::ops` (shared JSON ops; app state via `register_cache`/`register_routes`); routes `config` (all `RWS_CONFIG_*`, `shown()` hides secrets), `reload` (request_reload = SIGHUP path; waits on new `config_reload::last_reload()` count, reports `report_error`s from hooks — proxy reload errors now reported; 503 before ready, 504 on timeout; `reloader()` override for tests), `metrics`, `build`, `rate-limit`(+`/check?ip=`), `cors`, `maintenance` GET/PUT, `blocklist` GET/POST/DELETE (IPs validated), `flags` GET/PUT, `cache` GET/DELETE, `routes`, `requests`/`errors`; 405 with `Allow`. Binary's 20 MCP tools rewired onto `ops` (names/inputs unchanged; `server_config` now every setting, secrets hidden). Maintenance tests moved to the shared test lock. Tests: 12 admin unit; `ci/e2e/admin.sh` now 10 checks incl. a real SIGHUP reload applying a CORS change. 2142 default / 1966 http1 |
| 3 | **done 2026-10-30**: `proxy_config::upstream`: drain set (`set_drained`/`is_drained`, one atomic load when nothing is drained; `DynamicProxy::next_backend` skips drained → all drained `Err(true)` = 503), `snapshot()` (file order, weight, healthy = in live list, breaker state, drained). `metrics::tls::certificates()`; ACME renewal loop selects on a `Notify` (`acme::request_renewal`) and records each outcome (`acme::renewal_status`). Routes `GET /upstreams`, `PUT /upstreams/<name>/drain`, `GET /certificates`, `POST /certificates/renew` (202; 404 without ACME). TLS reload = `POST /reload` (SIGHUP path). Tests: 14 admin unit (drain through a real ConfigDrivenApp); `ci/e2e/admin.sh` 12 checks (drain with Python backends + health checks); `ci/e2e/acme_pebble.sh` step 6b: inventory + renew now hot-swaps the served cert (run locally against Pebble 2.10.1). Not covered: Gateway API routes (own forwarder) don't honour drain. 2144 default / 1968 http1 |
| 4 | **done 2026-10-30**: `log::tail` (off until the admin API starts): the access-log sink's `write_out` (flush thread every 100 ms, or a full shard) also hands each batch to `record` — 500-line ring, lines cut at 8 KiB, followers are `SseSender`s fed with `try_send` (a slow one misses lines; at most 8). Routes `GET /logs?n=` (lines as JSON strings) and `GET /logs/stream` (SSE). Tests: 2 unit (incl. reading the live stream) + e2e check 13 (a request appears in the SSE tail and in recent lines) |
| 5 | **done 2026-10-30**: `admin-ui/` (Vite 8.3.4 + lit 3.3.3 + `@lit-material/*`, the rws8.tech theme imported from `site/src/theme.css`, Geist latin subsets inlined, 16 Phosphor icons) → `src/admin/ui/{index.html,admin.js,admin.css}` (133 KB + 90 KB, reproducible: `admin-ui/build.sh --check` in CI), embedded by `src/admin/ui.rs` and served before the token check with CSP `script-src 'self'; connect-src 'self'; frame-ancestors 'none'` (+ `style-src 'unsafe-inline'` for component style attributes). Views: overview (counters, req/s chart with crosshair tooltip), upstreams (drain switches), certificates (+ renew), controls, config (filter), logs (fetch-read SSE, pause). Token in `sessionStorage`. Found while testing: maintenance mode never applied to proxy mode → binary adds `MaintenanceLayer` there, now `is_passthrough` while off. Tests: unit (UI files public, CSP, redirect, nothing else public) + `ci/e2e/admin_ui.sh` (Playwright: wrong token refused, sign-in, drain/undrain round-trip to the API, maintenance reaches the public port, live log, no CSP/script errors), CI matrix `admin-ui`. Screens checked light/dark, 1280 and 390 wide |
| 6 | **done 2026-10-30**: feature `wasm-plugins` (wasmtime 49.0.5: cranelift, runtime, std, anyhow; no OSV advisories; MSRV 1.96 ≤ 1.99), on in release/packaging builds. `wasm_plugin::WasmLayer`: core module, refused if it imports anything; exports checked (`memory`, `rws_alloc`, `rws_on_request`, optional `rws_on_response`/`rws_init`); `InstancePre` + fresh `Store` per request with fuel and `StoreLimits` (`trap_on_grow_failure`); text directive ABI (set/add/remove/uri/status/respond, CR/LF/NUL refused); errors → 500 + `rws_wasm_plugin_errors_total{kind=fuel|memory|trap|output}`. `[route.middleware.wasm]` (module, fuel, memory_mb, config; `rws --check` loads it; load failure → `PluginMisconfigured` 503). Bugs found: `MiddlewareConfig::is_empty` ignored the new field (route took the middleware-free async path); wasmparser's `Borrow` impl made four `map[key]` lookups ambiguous (now `[*key]`). Tests: 11 unit (WAT modules: pass/modify/respond, response hook, config, fresh instance, fuel, memory grow + initial, traps, bad output, imports/shape) + stack-depth + fuzz `wasm_plugin_output` (1M runs) + `examples/wasm_plugin` (Rust → wasm32, 19.6 KB) through the binary in `ci/e2e/wasm_plugin.sh` (5 checks), CI matrix `wasm-plugins`. Static musl release build with the feature checked in Docker (rust:1.99 + musl-tools, `acme auth mimalloc gateway-api wasm-plugins`: static-pie, 49 MB unstripped) and `ci/e2e/wasm_plugin.sh` passes against it. 2148 default / 2296 with features / 1972 http1 |
| 7 | **tagged 2026-10-30**: CHANGELOG dated, Cargo/helm/site lock/llms bumped to 18.10.0 after CI green on main (incl. the new admin, admin-ui, wasm-plugins jobs); no breaking API changes (the binary's maintenance mode now also covers proxy routes, in Changed). Release workflow: every job green (release, crates, 5 binaries, Docker, packages, Homebrew, site, chart). **18.10 done.** VM bohdan-prod-donotdelete upgraded 18.8.4 → 18.10.0 (sha256 checked, old binary kept as `rws.bak-18.8.4`, config backup `rws.config.toml.bak-18.10`), `[admin] listen = "127.0.0.1:9901"` with the token in root-only `/etc/rws/admin.env` (systemd drop-in `rws.service.d/admin.conf`); site, mail cert, PHP demo and docs MCP verified. Found: `/status` didn't list `wasm-plugins` (fixed after the tag) |
