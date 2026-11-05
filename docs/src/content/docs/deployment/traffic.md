---
title: Traffic insights
description: See who visits your sites (people, AI agents and bots) on the admin UI's Traffic page, counted by rws itself with no tracking script.
---

The admin UI's **Traffic** page (since 18.15.0) shows who visited your sites
over the last day, week or 30 days, in three groups:

- **People**: browsers that load the page's styles, scripts or fonts, as a
  browser does for a person.
- **AI agents**: AI crawlers, AI search and assistants fetching a page for
  someone (GPTBot, ClaudeBot, PerplexityBot, ChatGPT-User…).
- **Bots**: search engines, SEO tools, monitors, link previews, scanners
  probing for `/.env` or `/wp-login.php`, libraries like `curl`, and anything
  that claims to be a well-known crawler from outside that crawler's network.

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

- Visitors, page views and requests per group, with the change from the
  period before.
- Requests over time: per hour for a day, per day for a week or 30 days.
- The pages people read most, the AI agents that came (with **Verified** when
  the request came from the network its operator publishes), the pages AI
  agents read, bots by kind, the sites people came from, and paths answered
  `404`.
- With more than one site (virtual hosts, `Host` names) a button per site.
  Visitor counts are for all sites together.
- **Right now** (since 18.31.0): people on the site in the last 5 minutes,
  page views per minute for the last 30 and the pages being read;
  refreshed every 10 seconds.
- **Sessions** (since 18.31.0): sessions, bounce rate, session length and
  pages per session, with the change from the period before; the pages
  sessions began and ended on.
- **Sources** (since 18.31.0): how people arrived, by channel, plus the
  sites they came from and UTM campaigns (`utm_source`, `utm_medium`,
  `utm_campaign`).
- **Audience** (since 18.31.0): people's countries, browsers, operating
  systems and devices (desktop, phone, tablet), each person counted once a
  day.

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

The **Explore** section of the Traffic page then takes any dates and any
combination of filters, and so does the API:

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
5. Known AI agents ([ai.robots.txt](https://github.com/ai-robots-txt/ai.robots.txt))
   and crawlers ([crawler-user-agents](https://github.com/monperrus/crawler-user-agents))
   by name. Both lists and the networks are bundled and refreshed each release.
6. A browser `User-Agent` waits: once it loads a `.css`, `.js`, `.mjs` or font
   it is a person and its earlier requests count as one. Without one within
   two minutes or 32 requests it is a bot ("browser, no assets").

A visitor is a hash of a per-process secret, the day, the site, the address
and the `User-Agent`. The secret and the hashes are never saved: no addresses
or `User-Agent`s are kept, in memory or in the file. A restart within a day
can count a returning visitor once more.

Counting costs about 160 ns per request; the country and device are worked
out once per visitor per day. Memory and the file are bounded: at
most 16 sites (the rest count as `other`) and 200 entries per list per day
(the rest as `(other)`; 100 are saved).

## From the API or an AI client

`GET /admin/api/v1/traffic?hours=168&host=example.com` returns the page's data
(`hours` 1–2160, default 24; `host` optional); `404` when counting is off. The
`GET /admin/api/v1/traffic/live?host=example.com` (since 18.31.0) returns
the live view: `visitors` (people in the last 5 minutes), `views_5m`, a
`series` of page views per minute for 30 minutes, the `pages` and
`countries` of the people active now.

MCP tool `traffic_summary` `{"hours": 24}` answers the same, so an AI client
connected to the server can tell you who has been visiting; `{"live": true}`
gives the live view, and `{"from": "2026-03-01", "country": "DE", …}` a
database query.
