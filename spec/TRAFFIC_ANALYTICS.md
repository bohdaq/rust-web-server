# Traffic analytics: the Traffic page as a self-hosted Google Analytics

Written 2026-11-05. Extends the 18.15 Traffic page (`spec/TRAFFIC_INSIGHTS.md`,
`crates/rws-proxy/src/traffic/`) from "who visits — people, AI, bots" to
the reports people open Google Analytics for: where visitors come from,
what they use, how long they stay, which campaigns bring them, who's on
the site now. rws computes everything itself; it makes no outbound calls
and sends nothing to a third party. Target: one release once all six tranches
are done (user, 2026-11-05: order sessions → real-time → script →
Postgres; no deploy to bohdan-prod before the release).

## What exists (18.15–18.30)

- `traffic::record` gets every request from `Log::log_access` (all
  protocols, any log format): hourly `(hour, host) → [people, ai, bots] ×
  (requests, pages)`, daily unique visitors (SipHash of a per-process
  secret, day, host, IP, UA; never saved), daily top lists of 200 (pages,
  AI agents, bots, referrer hosts, 404s), people confirmed by loading
  assets. 16 shards, per-thread counts for decided visitors, ~160 ns a
  request.
- `GET /traffic?hours=&host=` (admin API), MCP `traffic_summary`, the
  admin UI Traffic view, `[traffic] enabled / retention_days / persist /
  exclude`, a bounded JSON file (≤ ~2 MB).

## Decisions

User, 2026-11-05:
- **Server-side for everyone, plus an optional first-party script.** The
  2026-10-31 "no JavaScript beacon" decision stands as the default; a
  site can opt in to a small script rws serves itself (`/_rws/t.js`,
  posting to `/_rws/t` on the same origin) for what the server can't
  see.
- **Features**: sessions, bounce rate, entry/exit pages; browser, OS,
  device; country from a bundled offline database; UTM campaigns; a
  real-time view.
- **Storage**: the bounded JSON file by default; **Postgres opt-in** (a
  server the user runs) for raw page views and longer history.
- **Cookieless**: no cookie or local storage, ever; the daily salted hash
  stays the visitor identity, so no consent banner is needed. Returning
  visitors across days are not counted — accepted.

Mine, within those (change them if you disagree):
- **Geo database compiled in** (behind a default-on `geo` feature): a
  sidecar file would miss `install.sh`, Linux packages, Nix, winget and
  `cargo install`, which copy only the binary. DB-IP Lite Country (CC BY
  4.0, monthly; 2026-11: 362,998 IPv4 ranges, 369,490 IPv6 /64 ranges,
  251 countries) → `ci/gen_geo.py` → `src/traffic/geo.bin`: range starts
  delta-varint encoded + a country byte each, deflated: **~700 KB**
  committed, refreshed each release (pinned month). Decoded on first use
  into sorted arrays (~5 MB RAM, only when counting is on); lookup is a
  binary search. `[traffic] geo_db = <path>` loads a newer file of the
  same format instead; `--no-default-features` builds report "unknown".
  The page shows the CC BY attribution.
- **The IP is used for the lookup and dropped**; only the country code is
  counted, like the visitor hash.
- **UA parsing is hand-rolled** (no `woothee`/`uaparser` dependency):
  browser family (Chrome, Safari, Firefox, Edge, Opera, Samsung Internet,
  other), OS (Windows, macOS, iOS, Android, Linux, ChromeOS, other),
  device (desktop, mobile, tablet) — parsed once per visitor per day
  (classification cache), versions not kept.
- **Script is opt-in per site**: `[traffic] script = true` makes rws
  answer `/_rws/t.js` and `/_rws/t`; the site adds `<script defer
  src="/_rws/t.js"></script>`. No automatic HTML injection (it would
  touch every HTML response's hot path and break CSP/SRI setups).

## Proposed design

### Server-side dimensions (tranche 1)

Per day, new top lists for **people only** (bots and AI already have
their own lists): `countries`, `browsers`, `os`, `devices`,
`utm_source`, `utm_medium`, `utm_campaign` (from the page's query
string), and the existing `referrers` gets a "channel" grouping
(direct, search, social, referral, campaign) computed at read time.
Each is counted per visitor once (visitors) and per page view (views).
Same bounds: ≤ 200 entries a day, rest `(other)`.

Mechanics: country, browser, OS and device are worked out once, when a
visitor is first classified, and kept in the classification cache beside
the `Class` (small enums + a country index). They are **counted when the
visitor is decided** — a browser waits in `pending` until it loads an
asset, so a bot that never does can't pollute people's Audience lists.
Chromium's default low-entropy client hints (`Sec-CH-UA-Mobile`,
`Sec-CH-UA-Platform`) win over the UA string when present. Known limit:
iPad Safari sends a macOS desktop UA, so it counts as desktop. UTM values
are read from the query string on page hits only, before the path is
stripped, capped like other top-list keys.

### Sessions (tranche 2)

A session = one visitor's page views with gaps < 30 minutes (GA's
default). Per shard, per visitor today: `last_seen`, `pages`, `entry`.
A session closes on the next view after 30 minutes or at `settle`; then
it counts `sessions`, `bounces` (1 page), `duration` (last − first view),
`entry_pages`, `exit_pages`. Bounded: ≤ 8192 open sessions per shard
(oldest closed early); `Shard::roll` closes every open session into the
old day before clearing it. Hourly buckets gain `sessions` and `bounces` for
people; the summary adds bounce rate, pages/session, average duration.

Implementation notes from tranche 1: `Deferred` keeps the hour only —
sessions need each deferred request's `now`, and a session starts at the
earliest deferred request, not at the asset that decided the person;
`persist::from_json` takes `hours` rows of exactly 8 numbers, so adding
session counts must accept both lengths to keep old files loadable.

### Real-time (tranche 3)

A per-shard ring of the last 5 minutes' people page views (visitor hash,
minute, page, country, referrer channel) — ≤ 4096 entries/shard. `GET
/traffic/live` → visitors now, per-minute views for 30 min (from the
hourly data's finest grain plus the ring), current pages, countries,
sources. UI polls it every 5 s on its own tab section.

### First-party script (tranche 4, opt-in)

`/_rws/t.js` (< 1.5 KB, no dependencies, served with a long cache and
an ETag) sends `navigator.sendBeacon('/_rws/t', …)` with: page URL,
`document.referrer`, screen width bucket, time on page (on
`visibilitychange`/`pagehide`), max scroll depth bucket, SPA route
changes (`history.pushState`/`popstate`), and custom events
(`window.rws.track('signup', {plan: 'pro'})` — names ≤ 64 chars, ≤ 4
string props). `/_rws/t` takes ≤ 2 KB JSON, rejects other origins,
is rate-limited per visitor, and records into the same shards (the
visitor hash from the request's IP + UA — still no cookie). Reports:
time on page, scroll depth, screen sizes, events, and **goals**
(`[[traffic.goal]]` = a page path or event name) with conversion rate.
Served by a `TrafficScriptLayer` that `main.rs` wraps outermost-but-one
in the static, proxy and gateway apps. It must report `is_passthrough`
(like `GlobalRateLimit`) when `script` is off, or config proxy routes
lose the async forwarder; when on, it answers only `/_rws/t*` and
`runs_inline`. `/_rws/t` itself is not counted as a request (`exclude`).
`/_rws/` is new (rws has no reserved-path convention yet) and is
documented as reserved.

### Postgres (tranche 5, opt-in)

`[traffic] postgres = "postgres://…"` (secret from env/file like other
`*_url` settings): a background thread batches page views and events
(no IP, no UA string: day-salted visitor hash, time, host, path,
referrer host, UTM, country, browser, OS, device, session id, duration)
and inserts them every 10 s, creating `rws_traffic_pageviews` and
`rws_traffic_events` on first use. A full queue drops rows (counted),
never blocks requests. With it set, `/traffic` can answer ranges past
`retention_days` and any filter combination (country + page, etc.) from
SQL; the JSON rollups stay the fast default.

**Decided (user, 2026-11-05):** release binaries get `model-postgres`
(sqlx, ~+2 MB) and the writer uses `DbPool`.

### API, UI, MCP (with each tranche)

`/traffic` grows fields (additive; old UI keeps working); the Traffic view
gets tabs: **Overview** (tiles: visitors, page views, bounce rate,
session duration; chart), **Sources** (channels, referrers, UTM),
**Pages** (top, entry, exit, 404), **Audience** (countries with a table
— no map tiles from a CDN — browsers, OS, devices, screen sizes),
**Behaviour** (time on page, scroll, events, goals; script only),
**Live**, and the existing **AI & bots**. Filters by host now; by
country/page/source only with Postgres. MCP `traffic_summary` gains a
`section` argument.

### Budgets

- CPU: the whole server-side tranche set ≤ +100 ns/request over today's
  ~160 ns in a same-run A/B (geo lookup and UA parse once per visitor per
  day; per-request work is a hash lookup).
- Memory: bounded per shard as above; persisted file ≤ ~4 MB.
- Disk: the JSON file goes through `disk_guard::has_room` as today.

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | Audience + sources | UA parser (browser/OS/device), `ci/gen_geo.py` + `geo-country.bin` + loader, UTM + channels; new day lists, persisted; `/traffic` fields; UI Audience + Sources; tests on real UAs, v4/v6 lookups, UTM parsing; fuzz target for UA parser and geo file loader; A/B cost |
| 2 | Sessions | sessions, bounce, duration, entry/exit pages; hourly session counts; UI Overview tiles + Pages; tests |
| 3 | Real-time | ring, `GET /traffic/live`, UI Live; tests |
| 4 | Script | `TrafficScriptLayer`, `/_rws/t.js`, `/_rws/t`, events, goals, `[traffic] script`, `[[traffic.goal]]`; UI Behaviour; Playwright e2e (real browser loads the script) |
| 5 | Postgres | client choice (open item), writer thread, schema, SQL-backed ranges/filters; e2e against a real Postgres in CI |
| 6 | Release | docs polish, release notes, release |

Every tranche ships its own tests, docs and checklist (CLAUDE.md): new
`RWS_CONFIG_TRAFFIC_*` in `config_file::SETTINGS_WITHOUT_FLAG` and the
`check_unknown_keys` schema, `configuration/env-vars.md` +
`config-file.md`, `deployment/traffic.md`, `reference/api.md` rows for new
public types, `ci/public_api.sh --update`, CHANGELOG "Unreleased",
`python3 ci/gen_llms.py`, `admin-ui/build.sh` (the bundle is committed),
a fuzz target and `stack_depth_tests` case for each new parser.

## Status

| # | State |
|---|---|
| 1 | **done 2026-11-05**: `ci/gen_geo.py` (DB-IP Lite 2026-11 → `geo.bin`, 698,713 bytes; `ZZ` = no country; `--check` in CI), `traffic::geo` (decoded on a background thread at start; bounded parser; fuzz `traffic_geo`), `traffic::device` (browser/OS/class; `Sec-CH-UA-*` hints read only on a visitor's first classification — reading them per request had cost ~20 ns), UTM + channels (direct/search/social/ai/referral/campaign; same-site navigation isn't an arrival), 8 `host\tvalue` day lists (≤ 400 in memory, 300 saved; additive in the file), `/traffic` fields + `geo_source`, UI Sources + Audience sections (bar charts, country names via `Intl.DisplayNames`, DB-IP attribution), `[traffic] geo_db`, default feature `geo`. A/B: decided visitor ~165 ns both builds. Tests: 6 new unit tests (with and without `geo`), `ci/e2e/admin_ui.sh` checks the lists and sections; public API +32 (additive); docs: traffic, env-vars, config-file, installation features, api, mcp/operate, CHANGELOG |
| 2 | **done 2026-11-05**: per-shard `open` sessions (gap 30 min, ≤ 8192/shard, quietest closed early; closed by `settle`, at a new day, or on the next view after the gap); per-day per-host `Sessions {sessions, bounces, duration, pages}` + `entry_pages`/`exit_pages` (counted on the start day; open sessions added to snapshots as if ending now); deferred requests keep their time, so a session starts at the first page even when the asset decides later; sources counted once per session (GA-style) — before, every page view without a Referer counted as *direct*. Saved as an extra `sessions` array per day (old files load). UI: four session tiles with change vs previous period, entry/exit tables. Tests: 2 new (sessions/bounces/length/entry/exit/save-load/host filter; bounded open sessions), e2e checks the tiles and `/traffic` |
| 3 | **done 2026-11-05**: per-shard `recent` ring of people's page views (`Recent {at, visitor, page, country}`, 30-min window, ≤ 4096/shard, memory only), `summary::live_json` (visitors + views in 5 min, 30 one-minute buckets, top pages and countries of the active), admin `GET /traffic/live`, MCP `traffic_summary` gains `live` (no new tool: the count of 25 stays), UI "Right now" tile with per-minute sparkline + current pages, Traffic view polls every 10 s. Tests: live unit test (window, buckets, bots excluded, host filter), admin route 200/404/405, MCP call; e2e checks `/traffic/live` and the tile |
| 4 | **done 2026-11-05**: `traffic::script` (`SCRIPT`, `TrafficScriptLayer`: `is_passthrough` for every other path, 304 by ETag, 405/413/403 cross-origin/204), `record::beacon` (top-level members only, person-only, ≤ 500 reports/session, `leave` needs a valid `ms`; `view` = a page view with a same-site Referer, so no new arrival), 7 `BEHAVIOUR` day lists (time on page as seconds + views, scroll buckets, width buckets, events, props, goals), goals per session (bitmask over ≤ 32 `[traffic] goals`, counted at close/snapshot), `[traffic] script`/`goals`, `/_rws/t` not counted as a request; wrapped in all four `main.rs` app chains. UI Behaviour section. Tests: layer (off/on, 304, 405, 413, 403, 204, passthrough), beacon (time, scroll, screens, events, props, two goals in one session, junk ignored, bots/undecided ignored, route change = page view not arrival, save/load); e2e: a real browser loads the script from `www/app.html`, sends an event, two route changes → events, props, both goals, time on page, screens. Found: headless Chromium sends unload-time beacons with its own UA (ignores the context override) — the e2e reports through route changes instead. Fuzz: `traffic_classify` also feeds `beacon` |
| 5 | **done 2026-11-05**: `traffic::store` (`model-postgres`): `[traffic] database_url_env` → `DbPool::connect`, `CREATE TABLE IF NOT EXISTS rws_traffic_pageviews / rws_traffic_reports` + `at` indexes; `sync_channel(20_000)` queue (`try_send`, drops counted), writer thread `rws-traffic-db` (500 rows or 10 s, multi-row INSERT, `to_timestamp`, `::bigint` casts because a NULL binds as text); page views carry their session's channel/UTM (kept on `Session`), audience from the cache; no IP/UA. `Query::filter` (parameterized, `prefix*` → escaped `LIKE`), `query_json` (views, visitors per day, sessions, bounces, daily series, 9 top lists), admin `GET /traffic/query` (404 no db, 400 bad dates, 500 db error), MCP `traffic_summary` `from`/`to`/filters, `store` flag in `/traffic`, UI Explore (form + tiles + chart + tables). Release binaries + `model-postgres` (not FIPS: sqlx TLS crypto outside the validated module). CI `model-databases` runs `traffic` tests on PostgreSQL 16. Tests: SQL builder/day parser, integration against real PostgreSQL (own schema via `search_path`; rows, totals, path prefix, channel and country filters, report props, no addresses), admin 404/405, MCP; manual: binary + PostgreSQL 17 (rows written after the 10 s flush, `device=mobile` query, Explore clicked in Chromium) |
| 6 | **ready 2026-11-05**: tranches 1–5 on main, CI fully green on 93be588a (fuzz included; fixed on the way: `fuzz/seeds/traffic_geo` was missing, and four admin tests wrote audit entries without the test lock, breaking the audit-chain test now and then). Waiting for the go-ahead to release (version, tag) |
