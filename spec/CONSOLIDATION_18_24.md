[Read Me](../README.md) > [Spec](.) > Consolidation 18.24

# 18.24.0 — Consolidation

Planned 2026-11-01 from a read-only review of 18.20–18.23 (8.8K lines in a
day). No new features: close the gaps the review confirmed, and stop the
class of release mistakes that slipped through (a CI YAML typo that silenced
CI for three commits, lockfile churn that broke bindgen, a Windows-only
compile error found by the release build).

## Decisions (2026-11-01, the user's)

1. Do everything below.
2. `trusted_proxies` defaults to loopback and private ranges (10/8,
   172.16/12, 192.168/16, 100.64/10, fc00::/7); a peer outside them can't set
   `X-Forwarded-Proto/Host` or `Forwarded`. Public CDNs in front of rws must be
   listed — CHANGELOG "Changed".

## Items and tranches

| # | Item | Status |
|---|---|---|
| 1 | Release gating: `--locked` on every cargo call in CI, actionlint job, Windows `cargo check` with the release's Windows features, release.yml refuses a tag whose commit has no green CI run or isn't on main / a `*.x` branch; `18.20.x` LTS branch; RELEASE.md MSRV | **done**: 14 cargo calls `--locked`, jobs `actionlint` (1.7.7, clean locally) and `windows` (`cargo check` with the release's Windows features), release.yml step checks branch + the commit's CI jobs (install.sh excepted; dry-run passes 18.23.0/18.23.1), `18.20.x` pushed at the 18.20.0 tag, RELEASE.md (MSRV 1.99, the gate) |
| 2 | Fleet: signed serial (rollback refused, persisted), restore the previous file when a reload fails, refuse fetched configs that change `[fleet]` | **done**: `.sig` v2 = `rws-fleet-sig v2 <serial> <b64>` over `rws-fleet-v2\n<serial>\n<body>`; sign defaults serial to max(now, previous+1), `--serial N`; serial persisted in `<state_dir>/serial`, `serial <= applied` refused; `[fleet]` keys of fetched vs local file must match; previous file restored + reload when the reload fails; 18.23 sigs accepted as serial 0. Tests: rollback refused across restart, forged serial, settings change refused, parse |
| 3 | WAF: keep MySQL `/*! … */` content, command-injection needs a shell-like argument at paranoia 1, `--check` warns when a WAF route has no `max_body_size` | **done**: `strip_sql_comments` keeps `/*!NNNNN …*/` content; `shell_like` (after `;`/`|` the argument must look like a path/flag/var/number or be absent; backticks and `$(` stay loose); validate warns without route or global body limit. Tests: 2 more attacks, 4 more benign texts, decode expectation updated |
| 4 | PKCS#11: a session pool, PINs masked in output, RFC 3986 percent-decoding | — |
| 5 | `trusted_proxies`; WebSocket upgrades send one `X-Forwarded-For` | — |
| 6 | OIDC `return_to`: reject control characters and whitespace, UTF-8 percent-decoding | — |
| 7 | Cluster: `publish` only enqueues; Redis connect timeout | — |
| 8 | Fuzz targets: fleet signature file, PKCS#11 URI | — |
| 9 | Shared helpers: one percent-decoder, one JSON escaper outside `mcp` | — |
| 10 | FastCGI serves `/.well-known/` | — |

Not in 18.24 (L, later): splitting the crate into a workspace, a
`cargo public-api` gate.
