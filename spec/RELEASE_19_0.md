# rws 19.0.0 — Proxy response pass-through

Written 2026-10-30. Roadmap item #63 ([ROADMAP.md](ROADMAP.md)), built
tranche by tranche; this file is updated as each lands.

## Why

On the 18.0.0 benchmarks rws led nginx in most scenarios but was level or
slightly behind on proxy-1k (EPYC 7763: 48.7 vs 45.3 µs CPU per request). The
remaining cost was rws parsing every upstream response into a `Response` and
writing it out again, about 7% of a proxied request's CPU. The idea: carry a
simple upstream head through unparsed, keeping what the parse does for safety
(dropping hop-by-hop and duplicate `Content-Length`, rejecting CR/LF).

`Response::parse_upstream` (2026-10-25, `e5d5c4b4`) already made that parse
lighter, at about the time the 7% was measured, so tranche 0 measures again
before anything is built.

## Decisions

- **Non-breaking first** (user, 2026-10-30, after tranche 0): the fresh
  profile shows rws already ahead of nginx on proxy-1k throughput, and part of
  the parse/serialize cost can go without an API change. Cut that in 18.x,
  A/B it, then decide whether pass-through still earns a major version.

- **19.0.0** because carrying the raw head needs a new field on `Response`,
  whose fields are all public and built by struct literals (user, 2026-10-30:
  "19.0").
- In the same release `Response` becomes `#[non_exhaustive]` with
  constructors, so later fields don't break anyone again. `headers` and the
  other fields stay public (no wider break).
- Pass-through only where nothing between the forwarder and the HTTP/1.1
  writer looks at headers: the config file's async proxy path, which already
  requires no route middleware and no global `[middleware]`. HTTP/2 and
  HTTP/3 encode headers one by one anyway, so they keep the parse.
- Behaviour must not change, only speed: everything rws adds to a proxied
  response today (Date, CORS, security headers, HSTS, Alt-Svc, Connection,
  compression, access log, metrics) still happens on the raw path.
- Shipped only on a same-run A/B win on proxy-1k against 18.10.1. Without one,
  19.0 ships tranche 1 alone (or not at all: the user's call).

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 0 | Measure | Benchmark workflow on main with `profile=true`: share of `parse_upstream`, `generate_response_coalesced`, header allocations and the body copy in proxy-1k recorded here. Under ~3%: ask the user before going on |
| 1 | `#[non_exhaustive]` `Response` | constructors for what struct literals did; every literal migrated (`src`, `examples`, `starters`, `tests`, docs code, `agents/skills/rws/SKILL.md`); `reference/migrating-19.md`; `ci/e2e/starters.sh`, `check_docs.py`, `gen_agents.py --check` green |
| 2 | Pass-through | the async forwarder keeps the sanitized upstream head and body without parsing headers; the HTTP/1.1 writer emits head + rws's own headers + body in one write; compression, log and metrics read what they need from a scan |
| 3 | Safety | output byte-identical to parse → normalize → write over the reference-parser corpus and deterministic mutations; fuzz target comparing the two; `stack_depth_tests` case; vectors: CL+TE, differing CLs, CR/LF in a value, bare LF, obs-fold, non-canonical reason phrase |
| 4 | A/B | Benchmark workflow with `base_ref=18.10.1`: a same-run win on proxy-1k, else revert 2–3 |
| 5 | Release | CHANGELOG (breaking first), migration guide, 19.0.0, `starters` minimum version, docs site version |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Status

| # | State |
|---|---|
| 0 | **done 2026-10-30**: Benchmark run 41057365457 (main, profile): proxy-1k rws 37,887 req/s vs nginx 36,872; CPU 45.4 vs 43.0 µs/req (+5.6%). rws userspace 26.9% of samples, kernel 61%. Pass-through-relevant: `parse_upstream` 1.37%, `Header::single_line` 1.12% (char-wise CR/LF scan on every header written), `from_utf8` 0.81%, `generate_response_coalesced` 0.54%, `str::trim` 0.54%, plus a share of mimalloc (~1.3%): about 4–5% at most |
| 0b | **done 2026-10-30**, non-breaking: `memchr` (byte searches) in `Header::single_line`, `without_cr_lf`, `split_name_value` and the parsers' line splits; `is_blank` (first-byte decision) instead of `trim().is_empty()`. `Response::with_status` and `impl Default for Response` added (additive). Tests: reference-parser equivalence + `is_blank` vs `trim` cases; 2150 default / 1974 http1. A/B vs 18.10.1: pending |
| 1 | on hold: `#[non_exhaustive]` tried and reverted pending the A/B (examples and `tests/` had no literals; docs neither) |
| 2 | — |
| 3 | — |
| 4 | — |
| 5 | — |
