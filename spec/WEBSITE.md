# Website plan — a marketing front for rws

Written 2026-10-25. The current site (`docs/`, Astro + Starlight, published at
rws8.tech) reads like reference documentation: the landing page is a Starlight
`splash` with a hero and a few tabs, and everything else is sidebar docs. This
plan adds a small marketing/brand layer **in front of** the docs, in the same
Astro project, so one build and one deploy still cover both.

## Goal

Tell rws's actual story before the reader hits the reference:

- **One binary does the stack.** Static server + automatic HTTPS + reverse
  proxy + gateway, configured in one `rws.config.toml` — no separate certbot,
  cron or proxy process.
- **The same engine is a crate you can read.** When config isn't enough, it's a
  Rust library with routing, middleware, an MCP server, an ORM, jobs and SSO —
  no hyper/Tower/Axum.
- **It's fast and small**, and tested in the open.

Audience: developers and operators evaluating a web server / proxy, and Rust
developers looking for a batteries-included framework. The site's job is to earn
the click into the docs, not to replace them.

## Scope decision

**Part of the docs site, not a separate site.** Reasons: Starlight already runs
in `docs/`, the content (config tabs, feature list, benchmark numbers,
CHANGELOG) already lives there, and one deploy keeps rws8.tech in sync per
release. The marketing pages are plain Astro routes alongside the Starlight
docs; the docs sidebar is untouched.

## Information architecture

| Route | What | Source |
|-------|------|--------|
| `/` | Landing page (the creative piece) | new; reuses existing content |
| `/why/` | The argument: one binary, honest comparison, who it's for | README "Why rws" + benchmark headline |
| `/playground/` | Interactive config builder → live `rws.config.toml` + command | new (small vanilla JS) |
| `/benchmarks/` | The numbers as charts, not a table | existing `reference/benchmarks.md` data |
| `/mcp/` | The three agent usages, live, once shipped | roadmap #60/#61/#62 |
| `/docs/…` | The existing Starlight docs, unchanged | as-is |
| `/blog/` (or `/changelog/`) | Release notes as posts | `CHANGELOG.md` |

Nav (top bar, shared): Home · Why · Playground · Benchmarks · Docs · GitHub.

## Landing page sections

1. **Hero — split, not a giant empty band.**
   - Left: the one-line pitch ("One binary. HTTPS, proxy, done."), a sentence of
     tagline, two buttons (Install, Docs).
   - Right: a **looping terminal demo** that types `cd my-site && rws`, shows the
     server starting, then a `curl` returning `200 OK`. This is the 60-second
     story as one animated moment. CSS/JS only; respects
     `prefers-reduced-motion` (falls back to a static framed terminal).
   - Protocol badges under it: HTTP/1.1 · HTTP/2 · HTTP/3/QUIC · Let's Encrypt ·
     TLS (rustls) · WebSocket · gRPC · SSE.
2. **Three modes, three tabs.** Static server / automatic HTTPS / reverse proxy,
   each a real syntax-highlighted config file with its one-line result and a
   copy button. (This content already exists on the splash; make it the
   centerpiece with proper code typography.)
3. **What's in the box — a six-tile grid**, not a `<details>` dump: Protocol &
   transport · Proxy & gateway · Security · Observability & ops · AI & MCP ·
   Database/ORM/storage. Each tile expands to its feature list (the inventory on
   `getting-started/features.md`). Scannable, not a wall.
4. **The library turn.** One section that flips the story: "When config isn't
   enough, it's a crate." A real Rust sample (the `routes!` quick-start), the
   one-`Middleware`-trait pitch, and the genuinely rare batteries in one crate
   (MCP server, OAuth/OIDC/SAML, async ORM, jobs, storage).
5. **AI & MCP — rws is agent-usable** *(add once the 18.0 MCP work ships —
   roadmap #60/#61/#62; until then keep it to one honest line, not a headline)*.
   Three usages, each with a live snippet: **operate rws** from an AI client via
   the built-in `/mcp` tools; **build agent tools** on rws with `app.mcp()`; and
   rws as an **authenticated MCP gateway** in front of other MCP servers. This is
   the positioning wedge — give it a real section, and a `/mcp/` page, not a
   tile. Do not showcase `/mcp` until it requires a token (#44).
6. **Performance strip.** Three big numbers from the latest benchmark run —
   req/s vs nginx, CPU per request, peak memory — each with a small sparkline.
   Links to `/benchmarks/`.
7. **Built in the open.** Trust: fuzzed parsers, h2spec 146/146, ACME tested
   end-to-end against Pebble, MSRV pinned, signed releases with an SBOM (add the
   signing/SBOM line once #5 ships; add a security-policy link once 17.113.1 and
   the advisory are public — do not pre-announce).
8. **Footer.** Real links: Docs, GitHub, crates.io, Docker Hub, Homebrew, the
   `llms.txt` for AI tools.

## Brand

The logo is a server rack whose bottom unit breaks into forward-moving shards —
the speed idea, in the shape. The provided brand assets live in
`docs/src/assets/brand/`:

- `rws-logo-lockup.png` — the horizontal mark + wordmark lockup (`rust-web-server`
  over `RWS`), for the nav and hero.
- `rws-brand-sheet.png` — the full sheet with the three app-icon variants (black,
  orange, black-split).

Replace these PNGs with SVG before shipping (crisp at any size, theme-tintable);
the PNGs fix the exact shapes and palette until then.

**Palette** (sampled from the logo):

| Token | Hex | Use |
|-------|-----|-----|
| Cream | `#F2EBE3` | light background |
| Near-black | `#16120F` | text, dark background, the rack |
| Orange | `#D2551E` | the one accent — the breaking unit, links, buttons |
| Warm grey | `#6B625C` | muted labels, captions (`RWS` subhead) |

## Design direction

- **Identity from the logo: "systems software with momentum."** The brand is
  warm-neutral, not a generic dark dashboard — lean into the cream + near-black +
  single-orange palette above, and carry the "breaking into motion" shard motif
  as a sparing graphic element (section dividers, the hero, hover states), never
  as wallpaper. The wordmark is a geometric sans, so the page is sans + mono, not
  serif — this is the brand's own warm palette, distinct from the generic
  cream-**serif**-terracotta look to avoid.
- **Code is the hero typography.** Config and Rust get the same care as
  headlines — that's what developers come to see. A monospace face for all code;
  a geometric sans matching the wordmark for prose and headings.
- **Motion with restraint.** The typing terminal and the logo's shard motif are
  the orchestrated moments; everything else is quiet hover states. Honor
  `prefers-reduced-motion`.
- **Theme-aware**, like the docs: light is cream/near-black, dark inverts to
  near-black ground with the orange holding as the accent; tokens only.

## New interactive pieces (the only real build work)

- **Terminal demo** — a small self-contained script that types a scripted
  sequence into a styled `<pre>`; pure CSS/JS, no library.
- **Config builder** (`/playground/`) — checkboxes/toggles for static, HTTPS
  (domains), reverse proxy (backends, strategy), auth, rate limit; assembles a
  valid `rws.config.toml` and the matching `rws` command live, with a copy
  button. Validate against the same key names the docs list so it never emits a
  setting the binary rejects.

Both are vanilla JS in the Astro page; no framework needed. Everything else is
static content reused from what already exists.

## Build checklist

1. Decide whether marketing pages live at the site root (`/`, docs move under
   `/docs/`) or the docs stay at root and marketing adds `/why`, `/playground`,
   etc. Root-marketing reads better for newcomers but means redirecting existing
   `/…` doc links — check what rws8.tech already has indexed before choosing.
2. Add a shared top nav and footer component used by both the marketing pages
   and the Starlight docs (Starlight supports a custom header component).
3. Build the landing page (`/`) with sections 1–7.
4. Build `/playground/` (config builder) and `/benchmarks/` (charts from the
   benchmark data — a small chart lib loaded from the CDN allowlist, or inline
   SVG).
5. Wire `/blog/` to render `CHANGELOG.md` entries (or Astro content collection).
6. Keep `ci/check_docs.py` green (internal links) and re-run `ci/gen_llms.py`
   if any page under `docs/src/content/docs/` changes; marketing pages outside
   that tree don't feed llms.txt.
7. `npm run build` passes; check the landing page at phone width.

## Open questions for the user

- Root-marketing vs docs-at-root (checklist #1).
- Blog vs plain changelog page.
- Is a logo/wordmark refresh in scope, or keep the current `logo.svg`?

## Hosting and the docs MCP server

Added 2026-10-27. The new website is **hosted, not GitHub Pages**: rws serves
it (the site is the showcase), which lets it run code — first of all an MCP
server that coding agents query for rws's documentation. GitHub Pages can only
serve the static parts below; until the move, rws8.tech keeps those.

### Why

Agents writing code on rws mostly get the API wrong — names from an older
release, or axum's. `llms.txt` helps but is one big file for one version. A
docs MCP server answers precisely, for the version the project uses, and is
itself a live example of rws's MCP support (#60–#62).

### Serving

- One rws process (the release image) serves the built site (`docs/dist`) as
  static files and `/mcp` as an `McpServer` mounted in front of it
  (`app.mcp(...)`), on the same domain: `https://rws8.tech/mcp`.
- Read-only and public: no token, every tool annotated `readOnlyHint`, a
  per-IP rate limit (`RateLimitLayer`), `Origin` checked (browsers only from
  the site itself), both MCP eras (2025-11-25 sessions and 2026-07-28
  stateless) as `McpServer` already speaks.
- Deployed by the release workflow after the docs build: a new image with the
  site and the docs index baked in, rolled out with a health check
  (`/healthz`, then one `tools/call`) before the old one is stopped.

### Data

- Built per release from `docs/src/content/docs/` (the same Markdown that
  feeds `llms-full.txt`): one record per page and per `##` section — path,
  title, text, code blocks — plus the rows of `reference/api.md`,
  `configuration/env-vars.md` / `config-file.md` (every setting, its key and
  default) and `CHANGELOG.md` split by release.
- Kept for the current release and the previous minor releases of the current
  major (18.x now), each under its version; `latest` follows the newest.
- Loaded into memory at startup (a few MB); search is in-process (BM25 over
  sections), no external index or service.

### Tools

| Tool | Returns |
|---|---|
| `search_docs(query, version?)` | the best-matching sections: page path, heading, an excerpt, a link |
| `get_page(path, version?)` | one page as Markdown |
| `get_example(name, version?)` | an `examples/*.rs` file and how to run it (from `getting-started/examples`) |
| `api_item(name, version?)` | the `reference/api.md` row(s) for a type or function, and its docs.rs link |
| `setting(name, version?)` | an `RWS_CONFIG_*` setting or `rws.config.toml` key: meaning, default, since |
| `check_config(toml, version?)` | what `rws --check` reports for a config file (the same `ProxyConfig::validate` + `check_unknown_keys` + settings checks, run in-process — nothing is written or started) |
| `changes(since, until?)` | CHANGELOG entries between two versions, breaking changes first — for upgrading |
| `versions()` | the versions it has docs for, and `latest` |

Resources mirror the pages (`rws-docs://18.7/building-apps/routing`) for
clients that browse instead of calling tools. A prompt, `start_rws_app`,
gives an agent the project shape from `AGENTS.md` for a version.

`check_config` is the one tool that runs product code on input from the
internet: it uses only the pure validators (no file system, no network, no
`set_var`), with an input size limit (64 KiB) and a time limit, and is fuzzed
like every config parser (`fuzz/fuzz_targets/config_file.rs`,
`proxy_config.rs`).

### Agent setup (what the site documents)

```bash
claude mcp add --transport http rws https://rws8.tech/mcp
```

plus the equivalents for Cursor, Codex and VS Code, on an "AI agents" page
that also links the static `llms.txt` files and the agent skill.

### Static parts (work on GitHub Pages too)

- Per-release `llms.txt` / `llms-full.txt` under `/<major.minor>/` next to the
  root ones (the newest).
- The agent skill/rules pack (`agents/` in the repo) published at
  `/agents/`.
- Register the repository with Context7 as well: agents that already use it
  get versioned rws docs with no setup.

### Done when

- `claude mcp add` against the hosted endpoint, then a coding agent answers
  "add a route with a path parameter and test it" for 18.x correctly, using
  `search_docs` and `get_example`.
- The MCP conformance suites pass against the hosted endpoint (both eras).
- Each tool has unit tests over a fixture docs index, `check_config` matches
  `rws --check` on `ci/e2e/config_reload.sh`'s files, and the release
  workflow's rollout check calls a tool before switching over.
