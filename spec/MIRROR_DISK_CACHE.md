# Request mirroring and a disk cache with a purge API (#39)

Written 2026-10-31. Roadmap item #39 ([ROADMAP.md](ROADMAP.md)); its third
part, backend drain, shipped in 18.10.0 (admin API). Built tranche by
tranche; this file is updated as each lands. Target release: **18.11.0**.

## What exists

- **Mirroring**: only Gateway API routes (`RequestMirror` filter,
  `gateway_api::router::send_mirror`: a thread per copy, the response
  dropped). Nothing for config-file routes or library apps.
- **Cache**: `CacheLayer` keeps whole responses in memory (`HashMap` + FIFO
  order, one TTL per layer, `Vary` by chosen request headers), config file
  `[route.middleware.cache] ttl_secs / vary_by`. The admin API can only clear
  it all, and only the binary's own cache (`admin::ops::register_cache`).

## Decisions

User, 2026-10-31:
- **Upstream `Cache-Control` honoured by default**: `s-maxage`, then
  `max-age`, capped by the layer's `ttl` (which stays the default when the
  response says nothing). `max-age=0` is no longer cached. Listed under
  Changed. `Age` on hits.
- **Purge through the admin API and an HTTP `PURGE` method**: `PURGE` on a
  cached route, authenticated by a bearer token from `purge_token_env`
  (nginx/Varnish style; no token configured → `PURGE` isn't accepted).

Implementation (defaults):
- **Mirroring is fire-and-forget**, engine in `proxy::mirror` (bounded queue
  + workers, not a thread per copy; full queue → dropped and counted), called
  from `DynamicProxy` after backend selection in both `execute` and
  `execute_async`, so mirrored routes keep the async fast path. Config under
  `[route.action.proxy]`: `mirror = "<upstream>"`, `mirror_percent`. Bodies
  over 64 KiB and streamed bodies aren't mirrored. Mirror results never feed
  the primary's breaker or retries. `MirrorLayer` for library `.wrap()` users.
  Gateway API's `send_mirror` stays as is.
- **Disk cache = a second store behind `CacheLayer`**:
  `CacheLayer::disk(dir, max_bytes)`. One file per entry (name = hash of the
  key): a metadata line (key, stored-at, ttl) then the response as
  `Response::generate` wire bytes, read back with `Response::parse` (already
  fuzzed and reference-tested). Temp file + rename; file I/O outside the
  index lock; index rebuilt by scanning the directory at start (bad files
  removed); total-size bound, oldest evicted. `runs_inline` false for a disk
  store. No default `dir`; `rws --check` checks it's writable. Two layers
  from overlapping reload generations may share a dir briefly: safe with
  atomic renames.
- **Admin API stays compatible**: `GET/DELETE /cache` keep their 18.10 shape
  for the binary's cache; new `GET /caches` lists every named cache and
  `DELETE /caches/<name>?path=&prefix=` purges. The registry is rebuilt per
  config generation.

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | Mirroring | `MirrorLayer` (`proxy::mirror`): backends, sampled share (`percent`), bounded queue + workers, `X-Forwarded-*`, response dropped; config file `[route.mirror]` (`upstream` or `backends`, `percent`) on proxy routes; `rws --check`; `rws_mirror_requests_total{route,outcome}` (sent, failed, dropped); unit tests + e2e (a mirror backend sees the copies, the client is unaffected when the mirror is down or slow) |
| 2 | Response TTL | `s-maxage` / `max-age` honoured by default, capped by `ttl`; `Age` header on hits; CHANGELOG Changed; tests |
| 3 | Disk store | `CacheLayer::disk(dir, max_bytes)`; atomic write + rename; index rebuilt at start (corrupt files skipped and removed); size bound + eviction; config `[route.middleware.cache] dir`, `max_size`; fuzz the entry decoder; stack-depth case; tests incl. restart persistence |
| 4 | Purge API | `CacheLayer::purge(Exact/Prefix/All)` for both stores; named registry (binary + config routes, rebuilt per generation); admin `GET /caches`, `DELETE /caches/<name>?path=&prefix=` (18.10 `/cache` kept); HTTP `PURGE` with `purge_token_env`; `cache_clear` MCP tool takes optional `path`/`prefix`; admin UI lists caches with a purge field; e2e |
| 5 | Release | CHANGELOG, docs (`features/caching.md`, `proxy/` mirroring section, admin page), 18.11.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Status

| # | State |
|---|---|
| 1 | **done 2026-10-31**: `proxy::mirror` (`Mirror`: live backend list, even `percent` sampling, 1024-slot queue + 4 workers, bodies ≤ 64 KiB and in memory, `forward_upstream` with a 2 s / 10 s timeout, streamed answers drained; `MirrorLayer`); config `[route.middleware.mirror]` (`upstream` = declared upstream's live list or an address, or `backends`, `percent`) — on proxy routes `DynamicProxy::with_mirror` sends once per request in both `execute` and `execute_async`, and `MiddlewareConfig::is_empty` ignores the mirror so the async forwarder stays; other actions get a `MirrorLayer`. `rws --check`: neither/both targets, bad address, percent outside 0–100 (warn at 0). `rws_mirror_requests_total{mirror,outcome}`. Tests: 6 unit (sampling, delivery with body, skips, dead mirror never delays, config route with 50% on the async path, check errors) + `ci/e2e/mirror.sh` (5 checks incl. a 3 s mirror not delaying 3 requests) in CI as `mirror`. Note: config key is `[route.middleware.mirror]`, not `[route.action.proxy]`, so it works on every action. 2156 default / 1980 http1 |
| 2 | **done 2026-10-31**: `cache::freshness` (s-maxage, then max-age, smaller wins when repeated, quoted values accepted, bad values ignored; capped by `ttl`; 0 → not stored); `CachedEntry.ttl` per entry (`get`/`purge_expired` use it). `Age` on hits already existed. Tests: freshness table + max-age=0 not stored + max-age=1 expiring under ttl 60 (41 cache tests). Docs table in features/caching.md; CHANGELOG Changed |
| 3 | **done 2026-10-31**: `cache::disk::DiskStore` (file per entry named by sha256(key)[..16]; header `RWSCACHE1 <ms> <ttl> <keylen>\\n<key>\\n` + `Response::generate` bytes, read back with `Response::parse_upstream`; temp + rename; index rebuilt at open, removing expired/unreadable/`.tmp-` files; total-bytes bound, expired then oldest evicted; files deleted outside the lock; damaged entry = miss + removed). `CacheLayer::disk`, `stored_bytes`; `runs_inline` false with disk; wasm32 stub. Config `[route.middleware.cache] dir`, `max_size` (`parse_size`, default 256M); a dir that fails to open logs and falls back to memory; `rws --check`: bad size, max_size without dir (warn), unwritable dir (online). Tests: 9 (round trip with Age and no stored Content-Length, restart, eviction + file deletion, corrupt/temp cleanup, damaged entry, TTL/max-age, clear, header parser, config + check). Fuzz target `cache_entry` + seeds. No stack-depth case: the header parser is loop-free and the response parser is already covered |
| 4 | **done 2026-10-31**: `cache::Purge` (`All`, `Exact`, `Prefix`; matches the key's target, so every `Vary` variant; `from_target` maps a trailing `*` to a prefix) and `CacheLayer::purge` for memory and disk; `.purge_token()` / `purge_token_env` → the layer answers `PURGE` itself (200 `{"purged":n}`, 401 + `WWW-Authenticate` on a bad token, constant-time compare; no token = passthrough); `PURGE` added to `Request::is_supported_method` and the wire check `framing::validate_head` (found by the UI e2e: it was a 400). `cache::registry`: `app` (binary, via `admin::ops::register_cache`) and `route:<name>` / `route:#N` replaced per config generation (`replace_group`). Admin `GET /caches`, `DELETE /caches/<name>?path=&prefix=` (404 unknown); `/cache` unchanged. `cache_clear` MCP tool: optional `path`/`prefix`. Admin UI: Controls → Response caches table + purge field (the single cache stat removed). `rws --check`: unset `purge_token_env` is an error. Tests: purge exact/prefix/all with Vary, PURGE token/no token, disk purge, config → registry → admin list/purge/404 → PURGE through the route → check error; framing + request method tests; `ci/e2e/admin_ui.sh` purges from the page and with HTTP PURGE |
| 5 | — |
