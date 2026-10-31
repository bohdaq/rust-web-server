# 103 Early Hints and compression dictionaries (#38)

Written 2026-10-31. Roadmap item #38 ([ROADMAP.md](ROADMAP.md)). Built
tranche by tranche; this file is updated as each lands. Target release:
**18.12.0**.

## What exists

- **Interim responses**: only `100 Continue`, written by the HTTP/1.1 loops
  themselves (`Server::continue_response`). An `Application` returns one
  `Response`; nothing lets a handler send a `1xx` first. The proxies read
  past a backend's `1xx` heads and drop them (`read_final_response_head`,
  `async_upstream` — `MAX_INTERIM_RESPONSES`).
- **Compression**: `gzip`, `br`, `zstd` (`compression::{negotiate, encode}`);
  static files are compressed once per `(len, mtime)` and coding and cached in
  the open-file cache (`OpenFile::encoded`), or taken from precompressed
  siblings. No dictionary support.
- Both codec crates can compress with a raw dictionary: `brotli` 9.1
  (`set_custom_dictionary`) and `zstd` 0.14 (`with_ref_prefix`) — no new
  dependency.

## Decisions

User, 2026-10-31:
- **Hints come from config and from upstreams**: per route
  `[route.middleware] early_hints = ["</app.css>; rel=preload; as=style", …]`
  sent before the handler runs; a backend's own `103` is forwarded instead
  of dropped; `EarlyHintsLayer` (and `early_hints::send`) for library apps.
- **HTTP/2 and HTTP/3 by default, HTTP/1.1 opt-in**
  (`early_hints_http1 = true`, `RWS_CONFIG_EARLY_HINTS_HTTP1`): browsers act
  on `103` only over h2/h3, and some old HTTP/1.1 clients mishandle extra
  `1xx` responses.
- **Dictionary compression for static files**: files matching configured
  patterns are sent with `Use-As-Dictionary`; rws keeps their bytes by hash
  and sends the next version as a `dcb` / `dcz` delta against the copy the
  browser has. Proxied and dynamic responses are out of scope for now.

Implementation (defaults):
- **One interim channel per request**, `server::interim`: the connection
  layer installs a sink around running the app (a thread-local for the
  blocking pool and the sync server, a tokio task-local for `execute_async`),
  and `interim::send(headers)` queues a `103`. Drained concurrently with the
  handler: `http1_async::serve` selects on the handler and the queue and
  writes each head; the sync loop writes straight to its socket; `h2_server`
  gets an event and sends HEADERS without END_STREAM; `h3_handler` sends the
  head before the final one. Without a sink (TestClient, library
  `execute`), `send` is a no-op. No new `ConnectionInfo` field (it is built
  by struct literal outside rws). Never sent after the final head, never on
  HTTP/1.0, at most `MAX_EARLY_HINTS` (4) per request. Values checked for
  CR/LF/NUL.
- **Upstream `103`s** are forwarded with their `Link` headers only (other
  fields dropped), by the sync and async forwarders (shared with Gateway
  API routes, which pass them on too).
- **Dictionaries** (RFC 9842): setting `compression_dictionaries` = URL
  patterns (`/js/app.*.js`, `*` only, same-origin paths) →
  `RWS_CONFIG_COMPRESSION_DICTIONARIES`. A static file matching one gets
  `Use-As-Dictionary: match="<pattern>"`; its bytes are kept in a dictionary
  store keyed by SHA-256 (memory, `CONTENTS_BUDGET`-style bound, default
  32 MiB, oldest out; files ≤ 1 MiB). A request whose path matches the
  pattern, with `Available-Dictionary: :<b64 sha256>:` for a stored
  dictionary and `dcz`/`dcb` in `Accept-Encoding`, gets the file encoded
  against it: `dcz` = zstd with the dictionary as a ref prefix (window ≥
  the dictionary), header `5e 2a 4d 18 20 00 00 00` + hash; `dcb` = brotli
  with it as custom dictionary, header `ff 44 43 42` + hash. Preference
  `dcz`, then `dcb`. Results cached per `(len, mtime, dictionary, coding)`
  in the open-file cache's budget. `Vary: Accept-Encoding,
  Available-Dictionary`, ETag suffix `-dcz-<8 hex of hash>`. Restart = empty
  store = plain `br`/`zstd` until browsers fetch a dictionary again
  (persisting it is a later option).

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | Interim plumbing | `server::interim` sink + `early_hints::send` + `EarlyHintsLayer`; HTTP/1.1 async and sync (opt-in setting), HTTP/2, HTTP/3; limits and value checks; unit tests per protocol (a `103` arrives before the final head, none without a sink, none on HTTP/1.1 by default, none on HTTP/1.0); h2spec still 146/146 |
| 2 | Config hints + upstream 103 | `[route.middleware.early_hints] links`; `rws --check` (bad link value); the forwarders pass a backend's `103` `Link` headers on; `rws_early_hints_total{source}`; e2e `ci/e2e/early_hints.sh` (curl over h1 opt-in, h2 and h3 sees `103` then `200`, from config and from a Python backend) |
| 3 | Dictionary codec | `compression::Encoding::{Dcz, Dcb}`, `encode_with_dictionary`, `Available-Dictionary` parser (structured-field byte sequence), headers; round trips through the zstd and brotli decoders; fuzz target `available_dictionary` |
| 4 | Static-file dictionaries | `compression_dictionaries` setting (env, config file, `check_docs`), dictionary store, `Use-As-Dictionary`, negotiation on the static fast and general paths, cache, ETag/Vary; tests; e2e in Playwright Chromium (v1 then v2 of a bundle: v2 arrives as `dcz`/`dcb`, smaller than `br`, and runs) |
| 5 | Release | CHANGELOG, docs (`features/early-hints.md` or a section in proxy/static pages, `features/compression` section, env vars, api.md rows), 18.12.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Risks

- ~~`h3` 0.0.8 `send_response` may refuse a second HEADERS on a stream~~ —
  it keeps no response state; a `103` then the final head works (tested).
- ~~Chromium's Compression Dictionary Transport may sit behind a flag~~ — on
  by default in Playwright 1.64's Chromium, over `http://localhost`.

## Status

| # | State |
|---|---|
| 1 | **done 2026-10-31**: `early_hints` module — `send(links)`, `available()`, `valid()`, `EarlyHintsLayer` (runs inline), `MAX_PER_REQUEST` 4; crate-private `Sink` (closure + per-request count, `reset` for reuse), thread-local `with_sink` (restored on panic) and task-local `scope` (http2). `execute_request` / `execute_blocking` take the sink and install it for the async (`pin!`, no allocation), inline and blocking paths. HTTP/1.1 async: only when `http1_wanted` (HTTP/1.1 + `RWS_CONFIG_EARLY_HINTS_HTTP1`, `ServerConfig::early_hints_http1`) — a per-request channel, `select!` writes each `103` as it comes, leftovers before the final head; streamed-upload requests get none (the pump holds the socket). Sync server: same gate, hints written after the handler returns, before the response. HTTP/2: inline requests reuse one per-connection sink and write hints before `respond`; spawned and streamed-body handlers send `Outcome::Hints` over the response channel (so order is kept), `send_early_hints` = HEADERS without END_STREAM, skipped once responded. HTTP/3: per-request channel, `select!`, `send_response(103)`. Tests: 8 unit + h1 async (off by default, on: inline and blocking, never HTTP/1.0) + sync server + h2 via the `h2` crate's `poll_informational` (inline and spawned, none for a plain page) + h3 with the quinn/h3 client. h2spec 146/146. Docs: new `features/early-hints.md` (sidebar), env var, config file, api.md row, CHANGELOG. 2183 default / 2003 http1 |
| 2 | **done 2026-10-31**: `[route.middleware] early_hints = [...]` (a key, not a sub-table: `MiddlewareConfig.early_hints`, ignored by `is_empty`); proxy routes send them from `DynamicProxy` in both execute paths (async forwarder kept), other actions get an outermost `EarlyHintsLayer`. Upstream `103`: `early_hints::forward_upstream` in `read_final_response_head` (sync, also used by `ReverseProxy`) and the async forwarder's interim loop — `Link` values only; the shared forwarder code means Gateway API routes pass them on too. `rws --check`: empty / line-break values are errors, no `<URL>` a warning. `rws_early_hints_total{source=handler|configured|upstream}`. Tests: upstream head parsing, config respond + proxy route with a hinting backend through the sync and the async forwarder, metric, check errors. `ci/e2e/early_hints.sh` (5 checks: h2c config + backend 103, none on HTTP/1.1 by default, metric, HTTP/1.1 opt-in) in CI as `early-hints`; HTTP/3 left to the unit test (no QUIC curl). 2186 default / 2006 http1 |
| 3 | **done 2026-10-31**: `compression::dictionary` (separate module: adding variants to the public `Encoding` enum would break exhaustive matches) — `DictionaryEncoding::{Dcz, Dcb}` (`token`, `available`, `header_len`), `PREFERENCE` dcz then dcb, `hash`, `parse_available_dictionary` (`:<44 base64>:`, parameters ignored), `available_dictionary`, `negotiate` (by name only, `*` doesn't count, highest q), `encode` (zstd `bulk::Compressor` + `ref_prefix` + `WindowLog` covering dictionary + body; brotli `BrotliCompressCustomIoCustomDict` with `lgwin` likewise ≤ 24; inputs ≤ `MAX_SIZE` 1 MiB), `decode` (magic + hash checked, output capped; zstd `window_log_max(23)`), `mark_encoded` (`Vary: Accept-Encoding, Available-Dictionary`). No new dependency (brotli 9.0.0, zstd 0.14.3 as locked). Interop: the `zstd` CLI decodes rws's `dcz` output (whole, and the stream with `--patch-from`) — 36 580 → 397 bytes vs 965 plain zstd / 2 224 gzip; `dcb` gets its independent check from Chromium in tranche 4 (no brotli CLI here). Tests: 8 (round trips beating gzip 4×, wrong dictionary / magic / size cap / other coding, size limits, empty dictionary, header parsing, negotiation, Vary, window logs). Fuzz `available_dictionary` (header + both decoders): 1.0M runs, clean. 2194 default / 2014 http1 |
| 4 | **done 2026-10-31**: `ServerConfig::compression_dictionaries` (`RWS_CONFIG_COMPRESSION_DICTIONARIES`, comma list / config-file array; entries must start with `/`); `static_resource::dictionaries` — `glob` (`*` any run incl. `/`), `matching_pattern`, `use_as_dictionary` (sf-string escaping), store keyed by hash (32 MiB, oldest out). Fast path (`serve_regular_file`) only, for compressible files ≤ 1 MiB matching a pattern: `OpenFile::dictionary_hash` (hash + `remember` once per settled version), `OpenFile::delta` (one cached delta per version/dictionary/coding, contents budget, released on drop); ETag `-dcz-<4 bytes hex>`; `Use-As-Dictionary` + `Vary: Accept-Encoding, Available-Dictionary` on 200, 304 and streamed responses. Empty setting = one `is_empty` check. Tests: glob/pattern/escaping; v1 → v2 delta in both codings (decoded, < 600 B, ETag, 304 revalidation, new version offered too), unknown dictionary → br, no setting → unchanged (build-aware for http1). `ci/e2e/dictionary.sh` in CI as `dictionary`: Chromium (Playwright 1.64) gets v1 with `use-as-dictionary`, then sends `available-dictionary` for v2 and decodes `dcz` to the exact file (102 KiB → 85 B; br 13.7 KiB); curl forces each coding — `dcz` decoded by the zstd CLI, `dcb` (73 B) by the system libbrotlidec 1.1 via ctypes (Chromium prefers dcz, and a Playwright route can't change its `Accept-Encoding`); unknown dictionary → br. Chromium needed no flag on localhost. 2196 default / 2016 http1 |
| 5 | — |
