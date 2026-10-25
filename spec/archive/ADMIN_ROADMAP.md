# Admin UI Roadmap

A password-protected web UI served at `/admin` by the server itself. No external
process, no build step — one embedded HTML file with vanilla JS, backed by a
small REST API layer. Every use case below is achievable without restarting the
server.

---

## Use Cases

| # | Use case | What the user can do |
|---|----------|----------------------|
| 1 | Live config view | See all current settings (rate limits, timeouts, CORS, TLS paths) in one place |
| 2 | Rate limit tuning | Change `max_requests` / `window_secs` without restarting |
| 3 | IP filter management | Add / remove IPs and CIDR ranges from the allow or deny list at runtime |
| 4 | Reverse proxy backends | Add / remove / reorder backends in the `ReverseProxy` pool |
| 5 | CORS settings | Update allowed origins, methods, and headers live |
| 6 | Metrics dashboard | Real-time `rws_requests_total`, `rws_errors_total`, `rws_active_connections` |
| 7 | Session inspector | List active sessions with their TTLs; force-expire any session |
| 8 | Access log tail | Last N access log lines streamed via SSE |
| 9 | Graceful drain | Trigger shutdown drain (sets `/readyz` to 503, drains in-flight requests) |

---

## Architecture

```
GET /admin            → embedded single-page HTML (vanilla JS, no build step)
/admin/api/*          → AdminController — JSON REST API
                        guarded by AdminAuthLayer (Bearer token or Basic auth)

RuntimeConfig         → Arc<RwLock<…>> global singleton, analogous to rate_limit::global()
                        holds mutable overrides that components consult at request time
```

### New modules

```
src/admin/
  mod.rs          RuntimeConfig struct + global()
  api.rs          REST handlers for config, ip-filter, proxy, metrics endpoints
  ui.rs           serve embedded HTML (include_str! at compile time)
  auth.rs         AdminAuthLayer middleware (token from RWS_ADMIN_TOKEN env var)
  session_api.rs  list / expire sessions
  log_sse.rs      SSE tail of access log ring-buffer
```

### Key design decision

Today every component reads from `env::var` at startup or request time. The new
pattern: check `RuntimeConfig` first, fall back to env. This is additive and
non-breaking.

Components that cache their config at construction time (`RateLimiter`,
`IpFilter`, `ReverseProxy`) need a small refactor to poll `RuntimeConfig` on
each check. That is scoped to those three files.

---

## Phase 1 — Foundation

Prerequisites for everything else.

- `RuntimeConfig` backed by `Arc<RwLock<RuntimeConfigInner>>` — fields:
  `rate_limit_max_requests: Option<u64>`, `rate_limit_window_secs: Option<u64>`,
  `cors_allow_origins: Option<String>`, `ip_allow_list: Vec<String>`,
  `ip_deny_list: Vec<String>`, `proxy_backends: Vec<String>`
- `RuntimeConfig::global()` — process-wide singleton (lazy init via `OnceLock`)
- `RWS_ADMIN_TOKEN` env var; `AdminAuthLayer` rejects requests without
  `Authorization: Bearer <token>`; returns `401` with `WWW-Authenticate: Bearer`
- `GET /admin` → serves embedded `admin.html` (placeholder page in Phase 1)
- `GET /admin/api/config` → current effective config as JSON (env values merged
  with any runtime overrides)

**Deliverable:** curl-accessible config endpoint, auth protection, project
skeleton for the remaining phases.

---

## Phase 2 — Mutable Config

- `PATCH /admin/api/config/rate-limit` body `{"max_requests":500,"window_secs":60}`
  → writes into `RuntimeConfig`; `RateLimiter::check` reads it per-window
- `PATCH /admin/api/config/cors` body `{"allow_origins":"https://example.com"}`
  → CORS controller consults `RuntimeConfig` before env
- `GET  /admin/api/ip-filter` → current allow + deny lists as JSON
- `POST /admin/api/ip-filter/allow` body `{"cidr":"10.0.0.0/8"}` → append
- `DELETE /admin/api/ip-filter/allow/:cidr` → remove
- `POST /admin/api/ip-filter/deny` body `{"cidr":"1.2.3.4"}` → append
- `DELETE /admin/api/ip-filter/deny/:cidr` → remove

All mutations are in-memory; they survive for the life of the process but are
reset on restart. A `GET /admin/api/config/export` endpoint returns a
`rws.config.toml` snippet that can be pasted into the config file to make
changes permanent.

---

## Phase 3 — Reverse Proxy Management

- `GET    /admin/api/proxy/backends` → list current backends with index and URL
- `POST   /admin/api/proxy/backends` body `{"url":"http://host:8080"}` → append
- `DELETE /admin/api/proxy/backends/:index` → remove by position
- `PUT    /admin/api/proxy/backends/:index` body `{"url":"..."}` → replace

`ReverseProxy` must hold its backend list behind an `Arc<RwLock<Vec<Backend>>>`
so the admin API can mutate it without touching the `Middleware` trait boundary.

- `GET /admin/api/proxy/health` → for each backend, attempt a `HEAD /` with a
  short timeout and return `{"url":"...","status":"ok"|"unreachable"}`

---

## Phase 4 — Metrics

- `GET /admin/api/metrics` → same counters as `/metrics` (Prometheus text
  format) but returned as JSON for the UI:
  ```json
  {
    "requests_total": 12450,
    "errors_total": 3,
    "active_connections": 7,
    "uptime_secs": 3601
  }
  ```
- Counters are the existing `src/metrics` atomics — no new state needed.
- `uptime_secs` requires storing `Instant::now()` at startup in `RuntimeConfig`.

---

## Phase 5 — Session Inspector

- `GET /admin/api/sessions` → list all live sessions:
  ```json
  [{"id":"abc123","created_at":"...","expires_at":"...","keys":["user_id","cart"]}]
  ```
- `DELETE /admin/api/sessions/:id` → force-expire (removes from `SessionStore`)

Requires `SessionStore` to expose an iteration method (`sessions()`) alongside
the existing `get` / `set` / `remove` API.

---

## Phase 6 — Access Log Tail (SSE)

- A fixed-capacity ring buffer (`VecDeque<String>`, capacity 200) captures the
  last 200 access log lines in memory alongside the existing file/stdout writer.
- `GET /admin/api/log/stream` → `text/event-stream` SSE response; pushes new
  log lines to connected clients as `data:` events. Uses the existing `Sse`
  builder.
- `GET /admin/api/log/recent` → returns the ring buffer snapshot as a JSON
  array for the initial page load.

---

## Phase 7 — Admin UI

A single `admin.html` file embedded at compile time via `include_str!`.

**Layout:**

```
┌─────────────────────────────────────────────┐
│  rws admin          v17.x.x    [Drain ⏹]   │
├──────────┬──────────────────────────────────┤
│ Config   │                                  │
│ IP Filter│   main panel (changes per tab)   │
│ Proxy    │                                  │
│ Metrics  │                                  │
│ Sessions │                                  │
│ Log      │                                  │
└──────────┴──────────────────────────────────┘
```

**Tech:** vanilla JS `fetch()` + `EventSource` for SSE. No framework, no CDN,
no build step. All CSS inline. Works offline.

**Tabs:**

| Tab | Content |
|-----|---------|
| Config | Read-only table of all env + runtime values; inline edit fields for rate-limit and CORS |
| IP Filter | Two editable lists (allow / deny); add-by-input, delete-by-row |
| Proxy | Ordered list of backends; add / remove / drag-reorder; health-check badge per row |
| Metrics | Auto-refreshing counters (every 5 s); sparkline charts via Canvas API |
| Sessions | Table of live sessions; "Expire" button per row |
| Log | Auto-scrolling log tail via SSE; pause button |

---

## Summary Table

| Phase | Feature | Status |
|-------|---------|--------|
| 1 | RuntimeConfig + AdminAuthLayer + `/admin` skeleton | Pending |
| 2 | Mutable rate-limit, CORS, IP filter via API | Pending |
| 3 | Reverse proxy backend management | Pending |
| 4 | JSON metrics endpoint | Pending |
| 5 | Session inspector | Pending |
| 6 | Access log SSE tail | Pending |
| 7 | Admin UI (embedded HTML) | Pending |
