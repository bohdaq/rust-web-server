---
title: Traffic insights
description: See who visits your sites (people, AI agents and bots) on the admin UI's Traffic page, counted by rws itself with no tracking script.
---

The admin UI's **Traffic** page (since 18.15.0) shows who visited your sites
over the last day, week or 30 days, in four groups:

- **People**: browsers that load the page's styles, scripts or fonts, as a
  browser does for a person.
- **Apps** (since 18.35): your native apps and browser extensions calling
  the server. They never load a page, so they'd otherwise look like bots.
- **AI agents**: AI crawlers, AI search and assistants fetching a page for
  someone (GPTBot, ClaudeBot, PerplexityBot, ChatGPT-User…).
- **Bots**: search engines, SEO tools, monitors, link previews, scanners
  probing for `/.env` or `/wp-login.php`, libraries like `curl`, and anything
  that claims to be a well-known crawler from outside that crawler's network.

Your own visits are counted apart, so the groups show everyone else (since
18.35, [Your own visits](#your-own-visits)).

rws counts this from the requests it already serves: no JavaScript on your
pages, no cookies, nothing sent anywhere.

## Turning it on

It is on whenever the [admin API](/deployment/admin/) is. Open the admin UI
and pick **Traffic**. To turn counting off, or tune it:

```toml
[traffic]
enabled        = true                       # false: not counted, no page
retention_days = 30                         # 1–90
persist        = "/var/lib/rws/traffic.json" # default: traffic.json beside the certificate; "off": memory only
exclude        = ["/healthz", "/readyz", "/metrics"]
```

The counts are saved every five minutes and at shutdown (written to a
temporary file and renamed, readable by its owner only, and only while
the disk has room), so restarts
and upgrades keep them. Without a certificate and without `persist` they live
in memory only.

## What the page shows

Since 18.32.0 the page is an overview with tabs:

- **The bar at the top**: the period (*Today*, *7 days*, *30 days* or a
  *Custom* range of whole UTC days), **who** — *Humans* (the analytics
  below), *Apps*, *AI agents*, *Bots* or *All* side by side — the site when
  there is more than one (the sites your `[[route]]`s match first, then
  other `Host` names seen; picking one shows only that site's numbers,
  lists and visitors),
  **Compare** (the period before, drawn grey in the chart and as the change
  under each number), and how many people are online now.
- **Overview**: visitors, page views, sessions, bounce rate and session
  length — click one to chart it (per hour for *Today*, else per day) —
  then a line of how sure the count of people is — **Verified** (ran the
  [script](#the-first-party-script-optional)), **Likely** (loaded the page's
  assets), **Page only** and **Server networks** (browser `User-Agent`s
  counted as bots: no asset loaded, or a cloud network) — and how many of
  your own page views were left out, then cards of top pages, sources, a
  **world map** of countries, devices, and *Right now*.
  A card's **All →** opens its tab.
- **Visitors** (since 18.35): the last visits, newest first — country,
  device, how they arrived (channel, referring site, campaign), the pages
  in order, events, **Verified**, and *Visit 2 today* when someone came
  back. Up to 300 are kept across days, with no address or `User-Agent`.
- **Pages**: top, entry and exit pages, paths answered `404`, **landing
  pages by source** (the first page of visits from outside, per channel)
  and **downloads** (installers, packages, archives and `install.sh`
  fetched in full, by who fetched them).
- **Sources**: channels, referring sites and UTM campaigns.
- **Audience**: countries, browsers, operating systems and devices, with
  their share.
- **Behaviour**: what the [first-party script](#the-first-party-script-optional)
  reports — time on page, goals, events, scroll depth, screen sizes.
- **AI agents** (the switch, not a tab): requests, agents and pages they
  fetched with the change from the period before, a chart, the agents
  (with **Verified** when the request came from the network its operator
  publishes), the pages they read and what for (training, search,
  assistants). **Apps**: app users by app and platform, versions, the
  events apps sent and downloads. **Bots**: the same numbers, bots by kind
  and by name, the paths answered `404`, **crawlers** (named crawlers'
  pages and 404s) with the **404s they hit**, **probes** (the paths
  scanners asked for) and where they came from (country and network).
  **All**: humans, apps, AI agents and bots in one chart and their share of
  requests.

**Click a row to filter** the page by it: by site always; by page, country,
channel, campaign, browser, system or device with the
[traffic database](#keeping-every-page-view-in-postgresql-optional), which
can answer any combination (the filters show as chips; ✕ removes one).
Page names drop the site when only one is shown. The map's country shapes
are [Natural Earth](https://www.naturalearthdata.com)'s (public domain),
built into the page; clicking a country filters like its row.

## The first-party script (optional)

What the server can't see — how long a page stays open, how far it's
scrolled, screen sizes, route changes in a single-page app, and your own
events — comes from a small script rws serves itself (since 18.31.0). It
reports to the same server, sets no cookie and stores nothing in the
browser, so it needs no consent banner either. Turn it on:

```toml
[traffic]
script = true
goals  = ["/thanks/", "/docs/*", "event:signup"]   # optional conversions
```

and add it to your pages:

```html
<script defer src="/_rws/t.js"></script>
```

Send an event from your own code, with up to four string properties:

```js
rws.track("signup", { plan: "pro" });
```

rws then answers `GET /_rws/t.js` (about 1.6 KB, 0.8 KB compressed, cached for an hour) and
`POST /_rws/t` (reports of at most 2 KB, from the same origin only); every
other request passes through untouched. `/_rws/` is reserved for rws.
Reports count only for a visitor already counted as a person, and at most
500 per session. A **goal** is a page path (exact, or a prefix ending in
`*`) or `event:<name>`; the page shows how many sessions reached it and the
share of all sessions. The **Behaviour** section shows time on page, scroll
depth (up to 25/50/75/100 %), screen widths, events and their properties,
and goals. Browsers driven by automation (`navigator.webdriver`) don't
report.

## Keeping every page view in PostgreSQL (optional)

The page's counts are kept small on purpose: hourly totals and the top
entries of each list, for `retention_days`. To keep every page view — for
longer history, or filters the counts can't answer ("visitors from Germany
who read `/docs/*` on a phone, from the spring campaign") — give rws a
PostgreSQL database (since 18.31.0):

```toml
[traffic]
database_url_env = "TRAFFIC_DATABASE_URL"   # the variable holding the URL
```

```sh
TRAFFIC_DATABASE_URL=postgres://rws:secret@db.internal/analytics rws
```

rws creates two tables if they're missing, `rws_traffic_pageviews` (one row
per person's page view: time, site, path, the day's visitor hash, session
start, referring site, channel, UTM values, country, browser family, OS,
device class) and `rws_traffic_reports` (the script's reports), and writes
them in batches from a background thread — every 10 seconds or 500 rows.
Rows hold no IP address and no `User-Agent`; the visitor hash changes every
day and at every restart. If the database is slow or down, rows queue (up
to 20,000) and then are dropped rather than slowing requests. rws never
deletes rows: keep or prune them as your policy says.

The Traffic page then filters by any row you click (country, page,
channel, campaign, device… in any combination) over any dates, and so does
the API:

`GET /admin/api/v1/traffic/query?from=2026-03-01&to=2026-05-31&country=DE&path=/docs/*&device=mobile`

Filters: `host`, `path` (exact, or a prefix ending in `*`), `country`,
`channel`, `utm_source`, `utm_campaign`, `browser`, `os`, `device`; `from`
and `to` are UTC days, inclusive (default: the last 30 days). The answer
has page views, visitors (per day), sessions, bounces, page views per day
and the top pages, countries, channels, referrers, campaigns, browsers,
systems and devices. `404` without a database. The tables are plain SQL, so
any other tool can read them too.

The release binaries include PostgreSQL support (the `model-postgres`
feature), except the FIPS build: the database client's TLS brings crypto
outside the validated module.

## Your own visits

Since 18.35 rws leaves the owner's visits out of everyone's numbers (and
shows how many it left out). A client is yours when:

- it made a **successful admin API call** — the admin UI does, signed in,
  whether you opened it on the admin listener or through a route that
  proxies it. From then on that day, its address and `User-Agent` are
  yours on every site;
- its address is in **`owner_networks`**:

  ```toml
  [traffic]
  owner_networks = ["203.0.113.7", "198.51.100.0/24"]
  ```

- its browser has the **owner cookie**. *Exclude a browser…* on the
  Overview lists a link per site, `https://<site>/_rws/me?k=<key>`: open it
  in each browser and on each phone you use. It sets `rws_me` on that site
  for a year (`HttpOnly`); `/_rws/me?off` removes it. The key is derived
  from the admin token, so it changes when the token does (with OIDC
  sign-in only, it's made at start and a restart ends it). Keep the links
  private: anyone opening one is counted as you.

Recognition starts when it happens: what you did earlier that day stays
where it was counted. `GET /admin/api/v1/traffic/owner` returns the link's
path and the sites.

## Apps and extensions

Native apps and browser extensions call your server without loading a
page, so the browser test would count them as bots. Since 18.35 they are
**Apps**, from:

- **`apps`**: `name=pattern` pairs. A pattern with `://` matches the start
  of the request's `Origin` (an extension); any other pattern is looked for
  in the `User-Agent` (case doesn't matter), and after a pattern ending in
  `/` the version is read:

  ```toml
  [traffic]
  apps = [
    "HITOP=HITOP/",                                   # HITOP/2.3.1 CFNetwork/… Darwin/…
    "HITOP=chrome-extension://abcdefghijklmnopabcdefghijklmnop",
    "HITOP=safari-web-extension://",
  ]
  ```

- **the platform**, for anything unnamed: an extension `Origin`
  (`chrome-extension://`, `moz-extension://`, `safari-web-extension://`),
  Apple's networking stack (`CFNetwork/… Darwin/…`, `NetworkingExtension/…`),
  Android's (`Dalvik/`) and Electron. These are counted as *iOS/macOS app*,
  *Chrome extension* and so on: a request can claim any name, so only
  configured names are kept as names. `okhttp` isn't taken as an app —
  scrapers use it too; name your Android app's `User-Agent` in `apps`.

An app user is a client counted once a day, like a visitor. Apps can also
report **events** — no script, any origin, nothing stored on the device:

```sh
curl -X POST https://example.com/_rws/e \
  -d '{"app":"HITOP","v":"2.3.1","p":"ios","n":"request_sent"}'
```

`app` must be one of `apps`' names (else `403`); `n` is the event (≤ 64
characters), `v` the version (≤ 32), `p` the platform (`ios`, `ipados`,
`macos`, `visionos`, `watchos`, `android`, `windows`, `linux`, `chrome`,
`edge`, `firefox`, `safari`, `web`; else `other`). `204` when taken, `400`
when malformed; at most 500 events per client a day count. It answers
`OPTIONS` and sends `Access-Control-Allow-Origin: *`, so an extension or a
web view can post too. `/_rws/e` and `/_rws/me` are answered whenever
traffic is counted, `script` or not.

Referrals from app stores (App Store, Google Play, Chrome Web Store,
Firefox and Edge add-ons, Microsoft Store, Snapcraft, Flathub, F-Droid) are
their own channel, **App stores**. A goal can be any request a person
makes, not only a page: `goals = ["req:/api/proxy"]` counts the visits
that used a web app, not just opened it.

## Downloads, probes and crawlers

- **Downloads**: `GET`s answered `200` for `.dmg`, `.pkg`, `.zip`,
  `.tar.gz`, `.tgz`, `.tar.xz`, `.exe`, `.msi`, `.msix`, `.deb`, `.rpm`,
  `.AppImage`, `.apk`, `.aab`, `.ipa`, `.vsix`, `.xpi`, `.crx`, `.flatpak`,
  `.snap`, `.7z`, `install.sh` and `install.ps1`, per file and by who
  fetched them (people, apps, bots and tools, AI, you).
- **Probes**: the paths scanners asked for, and where from — country and
  network (the hosting provider, or `-`). `probe_addresses = true` adds
  each scanner's address, to block it; no other address is ever kept.
- **Crawlers**: each named crawler's (Googlebot, Bingbot, GPTBot…) page
  fetches and `404`s, and the paths that answered them `404` — the ones
  to fix or redirect.

## Sessions

A person's page views 30 minutes or less apart are one **session**; a
longer pause, or midnight (UTC), starts the next. A **bounce** is a
session of one page view. **Session length** runs from the first page
view to the last, so a one-page session is 0 seconds long — the server
can't see how long the last page stayed open. Sessions still going count
as if they ended now. Sessions are counted on the day they began.

## Sources and audience

Sources are counted once per session, from its first page view. That
page view **arrives from outside** when it has no `Referer` (*direct*),
a `Referer` from another site, or UTM tags; moving between the site's own
pages isn't counted as an arrival. The channel is *campaign* when the URL
has `utm_*` values, else from the referring site: *search* (Google, Bing,
DuckDuckGo, …), *social* (X, Facebook, LinkedIn, Reddit, Hacker News, …),
*AI assistants* (ChatGPT, Perplexity, Claude, Gemini, Copilot, …) or
*other sites*.

Browser, system and device come from the `User-Agent`, and from the
`Sec-CH-UA-Platform` / `Sec-CH-UA-Mobile` client hints Chromium browsers
send, which win. Only the family is kept (Chrome, not Chrome 141). iPad
Safari presents itself as a Mac, so it counts as desktop.

The country comes from a table built into the binary, **IP Geolocation by
[DB-IP](https://db-ip.com)** ("IP to Country Lite", CC BY 4.0, ~700 KB,
refreshed each release): the lookup happens on the server, nothing is sent
anywhere, and only the country is counted — the address isn't kept.
Private and reserved addresses are *Unknown*. To use a newer table, build
one with `ci/gen_geo.py --update` and point `[traffic] geo_db` at it.
Building without the `geo` feature (on by default) leaves the table out;
countries are then *Unknown*, and `geo_db` isn't read either (decoding a
table needs the feature).

Nothing is stored in the visitor's browser — no cookie, no local storage —
so no consent banner is needed for these counts. The cost is that a
person who comes back the next day is a new visitor.

## How visitors are told apart

From each request's address, `User-Agent` and path, in this order:

1. Loopback is this machine (health checks, the admin UI) and isn't counted.
2. An address in a published crawler network (Google, Bing, Apple,
   DuckDuckGo, OpenAI, Perplexity, Common Crawl) is that crawler, verified.
3. A `User-Agent` that claims one of those crawlers from anywhere else is
   **spoofed**, a bot.
4. Paths only scanners ask for are scanners.
5. A browser `User-Agent` from a cloud or server hosting network (AWS,
   Google Cloud, Azure, OVHcloud, Hetzner, DigitalOcean… — since 18.35) is a
   program — a link scanner, a preview, a headless crawler — and a bot of
   kind **hosting**, named after the provider, even when it loads assets and
   runs the script. The networks come from a table built into the binary
   (**[DB-IP](https://db-ip.com)** "IP to ASN Lite", CC BY 4.0, ~60 KB,
   refreshed each release with `ci/gen_hosting.py`). Consumer VPN and CDN
   egress (Cloudflare WARP, iCloud Private Relay, VPN services) isn't
   hosting: people browse from it. `[traffic] hosting_networks = false`
   counts these browsers as people again.
6. Known AI agents ([ai.robots.txt](https://github.com/ai-robots-txt/ai.robots.txt))
   and crawlers ([crawler-user-agents](https://github.com/monperrus/crawler-user-agents))
   by name. Both lists and the networks are bundled and refreshed each release.
7. A browser `User-Agent` waits: once it loads a `.css`, `.js`, `.mjs` or font
   it is a person and its earlier requests count as one. Without one within
   two minutes or 32 requests it is a bot ("browser, no assets").

A visitor is a hash of a per-process secret, the day, the site, the address
and the `User-Agent`. The secret and the hashes are never saved: no addresses
or `User-Agent`s are kept, in memory or in the file. A restart within a day
can count a returning visitor once more.

**Sites and unknown hosts** (since 18.35.2): when your `[[route]]`s match
sites by `host`, those are the sites. `www.<site>` counts as `<site>`, and
every other `Host` — addresses, `localhost`, no `Host`, names no route
serves, and the names in `hide_hosts` — counts as one **Unknown hosts**
entry, so scanners' junk headers don't fill the site list. Counts saved
under such names move there when rws starts.

```toml
[traffic]
hide_hosts = ["mail.example.com", "mta-sts.example.com"]
```

**Sites and unknown hosts** (since 18.35.2): when your `[[route]]`s match
sites by `host`, those are the sites. `www.<site>` counts as `<site>`, and
every other `Host` — addresses, `localhost`, no `Host`, names no route
serves, and the names in `hide_hosts` — counts as one **Unknown hosts**
entry, so scanners' junk headers don't fill the site list. Counts saved
under such names move there when rws starts.

```toml
[traffic]
hide_hosts = ["mail.example.com", "mta-sts.example.com"]
```

Counting costs about 170 ns per request; the country, device and hosting
network are worked out once per visitor per day. Memory and the file are
bounded: every site a `[[route]]` matches by `host` (up to 238) plus 16 other
`Host` names (the rest count as `other`), and 200 entries per list per day
(the rest as `(other)`; 100 are saved). Before 18.35 the first 16 names seen
took the slots, so a site added later could end up in `other`; its counts
from then stay there, while its pages and sessions show under the site.

## From the API or an AI client

`GET /admin/api/v1/traffic?hours=168&host=example.com` returns the page's data
(`hours` 1–2160, default 24; `host` optional); `404` when counting is off.
`hosts` lists the sites to pick from, the configured ones (also in
`configured`) first.
`?from=2026-10-01&to=2026-10-07` (UTC days, inclusive; `to` defaults to
today) asks for those days instead. Views over two days cover whole days.
Besides the totals and lists, the answer has `series` (per hour or day:
requests per group and people's `pages`), `previous_series` (the period
before, at the same times) and `days` (people's visitors, sessions and
bounces per day, and the period before's). The
`GET /admin/api/v1/traffic/live?host=example.com` (since 18.31.0) returns
the live view: `visitors` (people in the last 5 minutes), `views_5m`, a
`series` of page views per minute for 30 minutes, the `pages` and
`countries` of the people active now.

Since 18.35 the answer also has `totals.apps` and `totals.owner` (and in
`series`), `tiers` (`verified`, `page_only`, `hosting` visitors), `apps`
(`users`, `versions`, `events`: `app\tplatform`, `app\tversion`,
`app\tevent`), `probes` (`paths`, `sources`), `crawl` (`name`, `pages`,
`not_found`), `crawl_404`, `entries` (`host/path\tchannel`), `downloads`
(`host/path\tgroup`), `owner_pages` and `visits` (the last 100: `start`,
`end`, `host`, `views`, `pages`, `country`, `browser`, `os`, `device`,
`channel`, `referrer`, `campaign`, `verified`, `number`, `events`).

MCP tool `traffic_summary` `{"hours": 24}` answers the same, so an AI client
connected to the server can tell you who has been visiting; `{"live": true}`
gives the live view, and `{"from": "2026-03-01", "country": "DE", …}` a
database query.
