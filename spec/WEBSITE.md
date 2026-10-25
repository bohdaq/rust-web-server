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
5. **Performance strip.** Three big numbers from the latest benchmark run —
   req/s vs nginx, CPU per request, peak memory — each with a small sparkline.
   Links to `/benchmarks/`.
6. **Built in the open.** Trust: fuzzed parsers, h2spec 146/146, ACME tested
   end-to-end against Pebble, MSRV pinned, signed releases with an SBOM (add the
   signing/SBOM line once #5 ships; add a security-policy link once 17.113.1 and
   the advisory are public — do not pre-announce).
7. **Footer.** Real links: Docs, GitHub, crates.io, Docker Hub, Homebrew, the
   `llms.txt` for AI tools.

## Design direction

- **Identity: "systems software, well made."** Dark-first palette built from the
  existing `logo.svg` colors, one warm accent, a monospace face for all code
  (most of the page is code), a crisp sans for prose. Deliberately avoid the
  generic cream-serif-terracotta AI landing look.
- **Code is the hero typography.** Config and Rust get the same care as
  headlines — that's what developers come to see.
- **Motion with restraint.** The typing terminal is the one orchestrated moment;
  everything else is quiet hover states. Honor `prefers-reduced-motion`.
- **Theme-aware**, like the docs: both light and dark, tokens only.

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
