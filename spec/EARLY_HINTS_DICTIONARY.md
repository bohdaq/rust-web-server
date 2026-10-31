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
  `[route.middleware.early_hints] links = ["</app.css>; rel=preload; as=style", …]`
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
  fields dropped), by the sync and async forwarders; Gateway API routes keep
  dropping them (no filter asks for it).
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

- `h3` 0.0.8 `send_response` may refuse a second HEADERS on a stream; then
  HTTP/3 hints wait for an `h3` change (documented, not faked).
- Chromium's Compression Dictionary Transport needs HTTPS (or localhost) and
  may sit behind a flag in the Playwright build; the e2e pins a version and
  enables the feature if needed.

## Status

| # | State |
|---|---|
| 1 | — |
| 2 | — |
| 3 | — |
| 4 | — |
| 5 | — |
