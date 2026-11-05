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
- **Sources** (since 18.31.0): how people arrived, by channel, plus the
  sites they came from and UTM campaigns (`utm_source`, `utm_medium`,
  `utm_campaign`).
- **Audience** (since 18.31.0): people's countries, browsers, operating
  systems and devices (desktop, phone, tablet), each person counted once a
  day.

## Sources and audience

A page view **arrives from outside** when it has no `Referer` (*direct*),
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
MCP tool `traffic_summary` `{"hours": 24}` answers the same, so an AI client
connected to the server can tell you who has been visiting.
