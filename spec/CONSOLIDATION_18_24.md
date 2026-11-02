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
| 4 | PKCS#11: a session pool, PINs masked in output, RFC 3986 percent-decoding | **done**: idle pool of ≤ 8 sessions (lock only to take/return), cleared + fresh session on error; `entry_point::shown` masks `pin-value=` in any value; own byte-level `%HH` decoder. Tests + SoftHSM e2e (container) green. Not done: signing off the accept task (C_Sign still runs inside rustls). **Follow-up after 18.24.0**: `pkcs11::off_the_worker` = `block_in_place` on the multi-threaded runtime (inline on current-thread, where it would panic); the binary defaults to the shared runtime with a `pkcs11:` key, warns on explicit per-core |
| 5 | `trusted_proxies`; WebSocket upgrades send one `X-Forwarded-For` | **done**: `ip_filter::TrustedProxies` (default list, `*`, `none`), `ServerConfig::trusted_proxies`, `proxy::from_peer` drops XFF/XFP/XFH/Forwarded from untrusted peers in build_request (h1 sync+async), h2/h3 upstreams and upgrades; upgrade XFF chained once; `--check` errors on bad entries, warns on `*`; docs reverse-proxy/env-vars/config-file/migrating-18. Tests: trust matrix, header replacement, `*`/`none` |
| 6 | OIDC `return_to`: reject control characters and whitespace, UTF-8 percent-decoding | **done**: `oidc_auth::local_path`; local Latin-1 decoder replaced by new shared `URL::form_decode` (UTF-8 lossy, `+`, lone `%` kept). Tests: 6 hostile `return_to`s, decoding |
| 7 | Cluster: `publish` only enqueues; Redis connect timeout | **done**: `ClusterSync.queue` + `Condvar` wakes the thread (round at once, else every second; heartbeat by elapsed time); `RespConn::connect` uses `connect_timeout` 2 s per address (all Redis users); pending changes kept after seeding an empty store. Tests: 100 publishes with the store down < 50 ms; real-Redis e2e green |
| 8 | Fuzz targets: fleet signature file, PKCS#11 URI | **done**: `fuzz_api::fleet_signature` (never verifies against a fixed key), `pkcs11_uri` (+ masking), seeds, README rows; fuzz crate gets `pkcs11`; stack-depth case. Found: `core::base64::Base64::decode` is very slow on long input (200 KB took minutes) → signature files over 256 characters are refused before decoding; the decoder itself is a follow-up |
| 9 | Shared helpers: one percent-decoder, one JSON escaper outside `mcp` | **done (partly)**: `core::json::escape` used by every module (mcp keeps a thin alias; sso/server + aws secrets had no control-char escaping); `URL::form_decode` added (OIDC); PKCS#11 has its own byte decoder. Left as is on purpose: the WAF's decoder (also `%u`), otel baggage's (strict, `Option`), the static-path decoder; 7 private base64 decoders (different alphabets/padding rules) — later, with tests per caller. **Follow-up done after 18.24.0**: `core::base64::Decoder` replaced all seven and the quadratic `Base64::decode`; fuzz target `base64_decode` |
| 10 | FastCGI serves `/.well-known/` | **done**: dot segments other than `.well-known` still refused; test |

Not in 18.24 (L, later): splitting the crate into a workspace, a
`cargo public-api` gate (being added after 18.24.0); PKCS#11 signing off
the worker done after 18.24.0 (item 4).
