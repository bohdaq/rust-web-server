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
| 1 | — |
| 2 | — |
| 3 | — |
| 4 | — |
| 5 | — |
