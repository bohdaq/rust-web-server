# Gaps for Using rws with React

This document tracks what's missing for `rws` to be a smooth backend/host for a React frontend — either serving a built React app's static output, or acting as the JSON API a React app (dev server or SPA in production) talks to.

---

## What already works

### CORS ✅ Done

Full config surface (`RWS_CONFIG_CORS_ALLOW_ALL`, `_ALLOW_ORIGINS`, `_ALLOW_CREDENTIALS`, `_ALLOW_HEADERS`, `_ALLOW_METHODS` in `src/entry_point/mod.rs`) lets a React dev server (Vite/webpack on `localhost:5173`/`3000`) call an rws API running on another port, including credentialed requests and preflight (`OPTIONS`) handling (`src/cors/mod.rs`).

### JSON APIs ✅ Done

`serde`/`serde_json` are optional deps (`Cargo.toml:31`) gated behind the `serde` feature. `BodyText`/`Body`/`Query` extractors (`src/extract/mod.rs`) plus `serde_json::from_str`/`to_string` cover typical fetch/axios request-response shapes.

### Static file serving ✅ Done

`StaticResourceController` (`src/app/controller/static_resource/mod.rs`) serves a built `dist/`/`build/` folder with ETags, `Last-Modified`, range requests, gzip, and directory listing when no `index.html` is present.

### Auth for SPAs ✅ Done

`sso` (OIDC client), `sso-server` (rws as its own OAuth 2.0 issuer — explicitly documented for single-page apps, see `DEVELOPER.md:4511`), `JwtLayer`, `BasicAuthLayer`, cookie/session store (`src/session/`, `src/cookie/`).

### Real-time ✅ Done

WebSocket (`src/ws_proxy/`, built-in WS handling) and SSE (`McpServer`'s `GET /mcp` SSE stream is one example; SSE is also usable standalone) for live UI updates.

### File uploads ✅ Done

`src/body/multipart_form_data` handles `multipart/form-data`, covering React file-upload UI.

---

## Gaps

### ✅ No SPA fallback for the static file server — Done

`StaticResourceController::is_matching` (`src/app/controller/static_resource/mod.rs:20-80`) only matched an existing file, an existing directory, or `path.html` — it never fell back to `index.html` for an arbitrary unmatched path. A React Router (`BrowserRouter`) deep link like `GET /dashboard/settings` had no file on disk and 404'd instead of being served `index.html` so client-side routing could take over.

The only documented SPA-fallback pattern in this codebase (`docs/src/content/docs/features/rewrite.md:126`, `.response_status(200, "OK")`) rewrites a *reverse-proxied upstream's* 404 response — it doesn't apply to the local static file server, which never proxies anywhere.

**Closed exactly as suggested:** `RWS_CONFIG_SPA_FALLBACK` (unset by default — disabled) names a file (e.g. `index.html`) that `StaticResourceController` now serves for a `GET`/`HEAD` request matching no real file/directory/`path.html`, instead of falling through to `404`. Read fresh on every request via new `entry_point::get_spa_fallback()`/`get_spa_fallback_exclude_prefixes()` helpers (mirroring `get_max_body_size`'s pattern), so it takes effect via `rws.config.toml` + `SIGHUP` with no restart, and needs no CLI-flag registration (consistent with how `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`/`RWS_CONFIG_CSP` are also env-var/config-file-only, not part of the original hardcoded CLI-arg list).

**Scoped to avoid swallowing real API 404s** via two independent, automatic checks: (1) an extension heuristic — only a request whose *last path segment* has no `.` gets the fallback (`/dashboard/settings` qualifies; `/assets/logo.png`, a missed static asset, still 404s) — the same heuristic webpack-dev-server's `historyApiFallback`/`sirv`/`vite preview` use; (2) new `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES` (comma-separated path prefixes, e.g. `/api,/healthz`) that never receive the fallback even when configured. The fallback file must actually exist on disk — a typo'd config value is a silent no-op, not a broken response. A file/directory that does exist is always served as itself, unchanged.

**A real, hard-to-reproduce test-isolation bug found and fixed along the way:** `RWS_CONFIG_SPA_FALLBACK` is the first `RWS_CONFIG_*` var `StaticResourceController::is_matching` depends on, and roughly a dozen pre-existing tests across `static_resource`, `async_state`, `state`, `test_client`, and `server` assert `404`/`!is_success()` for an unmatched no-extension path *without* holding `test_env::lock()` — because until now, nothing in that code path ever wrote a `RWS_CONFIG_*` var, so no lock was needed. Verified this was a real, reproducible race (not theoretical) via `cargo test --lib -- --test-threads=16` run ~15–20 times, which failed intermittently until every affected test took the lock. One unrelated pre-existing flake (`server::tests::no_expect_header_never_sends_100_continue`, racing on `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` against a neighboring test) surfaced during the same stress testing and was fixed as a drive-by.

8 new tests covering: disabled-by-default, serving the configured file for an unmatched route, a real file still winning over the fallback, the extension heuristic rejecting an asset-shaped path, exclude-prefix scoping, a missing configured file being a no-op, and the root path (`/`) staying excluded regardless (`IndexController`'s territory). Verified end-to-end against a real running server for all four scenarios (deep link → 200 shell, missing asset → 404, real asset → 200, root → 200) plus exclude-prefix behavior. Verified across default, `http1`-only — 1544 tests green (up from 1536), zero regressions, and stable under repeated high-thread-count stress runs.

### No typed `Json<T>` extractor/responder

JSON support exists via the `serde` feature, but there's no `Json<T>` wrapper analogous to Axum's — extracting a typed request body or returning a typed JSON response means hand-writing `serde_json::from_str::<T>(&BodyText::from_request(request)?.0)` and building the `Content-Type: application/json` response manually each time.

**Remaining:** a `Json<T>` extractor (`FromRequest` impl deserializing the body, 400 on parse failure) and a `Json<T>` responder (serializes to a `200 application/json` `Response`), mirroring the pattern of `Body`/`BodyText` in `src/extract/mod.rs`.

### ✅ No dev-mode asset proxying / HMR — Done

rws is a production HTTP server, not a bundler — there's no built-in "proxy `/api/*` to rws, everything else to the Vite/webpack dev server" story for local development. `ReverseProxy`/`RewriteLayer` can be wired manually to achieve this, but it isn't documented as a React-specific recipe.

**Closed via a new docs page**, `docs/src/content/docs/getting-started/frontend-dev-proxy.md` ("Frontend Dev Server Proxy (React / Vite / webpack)"), linked from Getting Started and cross-linked from the SPA-fallback section of the static-files page. The recommended recipe is the *inverse* of what the gap's own sketch proposed: point Vite's/webpack's own `server.proxy` (or CRA's `package.json` `"proxy"` key) at `rws` for `/api/*`, rather than wrapping `rws` in `RewriteLayer`+`ReverseProxy` to front the dev server. Investigating `ReverseProxy::path_prefix` (`src/proxy/mod.rs`) while writing the recipe surfaced why: its prefix match proxies *matching* paths and falls through to the wrapped app otherwise — the right shape for production (serve the built frontend locally, proxy `/api` out to a real backend), but the **opposite** of what "rws fronts the dev server" would need (proxy everything *except* `/api`, which has no built-in exclude-prefix, unlike `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`). The doc documents that direction too, as an "alternative," but is explicit that it needs a hand-written `Application` and — regardless of which direction the proxying goes — that `ReverseProxy` strips the `Upgrade` header (confirmed in source), so Vite's HMR WebSocket never survives being proxied through it either way. That combination of extra composition work plus a real functional regression (HMR degrading to full-page reloads) is exactly why the recommended direction is the dev server proxying *to* rws instead: no CORS needed, and the dev server's own HMR socket never leaves its own process. A third alternative (direct cross-origin fetch from the dev server's own origin to `rws`, using its `RWS_CONFIG_CORS_*` variables) is documented for the case where you're pointing the dev server at a shared/staging backend instead of a local `rws`.

### ✅ No React-specific CSRF recipe — Done

The general `csrf` module exists and works, but there's no documented double-submit-cookie or header-token pattern specifically for a fetch/axios-based SPA (as opposed to a traditional server-rendered form).

**Closed via a new "React (or any fetch/axios-based SPA)" section** in `docs/src/content/docs/features/csrf.md`, covering the one real wrinkle a traditional server-rendered-form recipe doesn't have to think about: *when* the `_csrf` cookie actually gets set. In production, `rws` itself serves the built SPA, so the very first `GET` (loading `index.html`) already goes through `CsrfLayer` and sets the cookie before any React code runs — no extra step. In local dev using the recommended dev-server-proxy setup above, the HTML shell comes from the dev server, not `rws`, so nothing sets the cookie until the SPA's first request through the proxy — the doc shows priming it explicitly with one safe `GET` call on app mount (a `useEffect` example), plus an axios request interceptor that reads the cookie and attaches `X-CSRF-Token` to every mutating request afterward, so no per-call boilerplate is needed beyond that. Cross-links to `CsrfLayer::http_only` are included so a reader doesn't accidentally pick the HTML-form-only variant that would make the cookie unreadable from JS.

---

## Summary table

| # | Gap | Area | Effort |
|---|---|---|---|
| 1 | SPA fallback (`index.html`) for unmatched static routes | Static file server | ✅ Done |
| 2 | Typed `Json<T>` extractor/responder | Extractors | Small |
| 3 | Dev-mode proxy recipe (rws + Vite/webpack dev server) | Docs | ✅ Done |
| 4 | CSRF recipe for fetch/axios SPAs | Docs | ✅ Done |

## Suggested implementation order

1. **SPA fallback** — ✅ done. Was the highest-friction gap in practice: any React Router app with deep links was broken under `cargo run`/`rws` without a hand-rolled workaround.
2. **`Json<T>` extractor/responder** — small, mechanical, and removes boilerplate from every JSON handler a React frontend talks to.
3. **Dev-mode proxy recipe** and **CSRF recipe** — ✅ both done. Documentation-only, no code changes required, closing the remaining "how do I actually wire this up" gap for a React consumer. Only `Json<T>` remains open.
