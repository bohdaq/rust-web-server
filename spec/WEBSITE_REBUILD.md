# Website rebuild — rws8.tech on lit-material

Written 2026-10-29. Supersedes the "Scope decision" in [WEBSITE.md](WEBSITE.md)
(marketing pages inside Starlight): the whole site, docs included, is rebuilt on
[lit-material](https://github.com/bohdaq/lit-material) and designed with the
`landing-page-design` skill. WEBSITE.md still holds the content plan (sections,
brand, palette, interactive pieces) and the hosting + docs MCP design; this file
is how to build it.

## Decisions

| | Decision |
|---|---|
| Site | rws8.tech, the rust-web-server product site |
| Docs | rebuilt in lit-material (Starlight removed at cutover) |
| Build | static prerender with Lit SSR; components hydrate in the browser |
| Hosting | served by rws itself (release image), with `/mcp` next to the site |
| Visual rules | `landing-page-design` skill wins; lit-material tokens are rethemed to it |
| Primary CTA | Install rws (copyable install command); Docs is a quiet secondary link |
| Code home | `rust-web-server/site/`; Markdown stays in `docs/src/content/docs/` |
| Benchmarks | landing shows three rws numbers; named comparison on `/benchmarks/` with method |
| Docs versions | versioned from day one, per minor (`/docs/18.8/…`), `/docs/…` = latest |

## Constraints found while planning

- `docs/src/content/docs/` (107 pages) feeds `ci/gen_llms.py`, `ci/check_docs.py`
  and the planned docs MCP index. **The content path does not move.**
- `ci/gen_llms.py` reads the sidebar order out of `docs/astro.config.mjs`. The
  sidebar moves to `docs/sidebar.json`, read by both the site and `gen_llms.py`.
- Content uses Starlight syntax: 128 asides (`:::note` 94, `:::caution` 30,
  `:::tip` 4), Starlight components in one file (`index.mdx`, replaced by the new
  landing). The renderer must support asides; nothing else is Starlight-specific.
- rws8.tech also serves the signed **apt and dnf repositories** (`/apt`, `/rpm`),
  `llms.txt`, `llms-full.txt`, `/<release>/llms*.txt` and `/agents/`
  (release.yml "rws8.tech" job). All must keep working through cutover.
- Existing indexed doc URLs live at the root (`/getting-started/quick-start/`).
  With the landing at `/` and docs under `/docs/`, every old path gets a 301.
- lit-material is at 0.0.x on npm. Pin exact versions; fix component gaps
  upstream in lit-material rather than patching in `site/`.

## Stack

- `site/` is a Vite + TypeScript app. Pages are Lit components rendered to HTML
  at build time by a Node prerender script using `@lit-labs/ssr`; the client
  bundle hydrates only interactive islands (nav menu, theme toggle, tabs, code
  copy, search, playground, terminal demo).
- Markdown: unified (remark-gfm, remark-directive for asides, rehype-slug,
  rehype-autolink-headings) and Shiki for highlighting at build time (TOML,
  Rust, bash, YAML, JSON), so code ships as static HTML.
- Search: Pagefind over the built HTML, per version, rendered in a
  lit-material `search` view.
- Icons: Phosphor (SVG, inlined at build). No Material Icons.
- No framework beyond Lit and lit-material.

## Theme (landing-page-design Part B on lit-material tokens)

One `site/src/theme.css` overriding `@lit-material/tokens` custom properties:

- Type: Geist for everything, Geist Mono for code and numbers. No Roboto, no
  italics, max weight bold. Sizes only on the Tailwind scale steps (B1).
- Color: rws palette from WEBSITE.md (cream `#F2EBE3`, near-black `#16120F`,
  orange `#D2551E`, warm grey `#6B625C`) mapped to MD3 roles; dark mode on the
  skill's allowed backgrounds (`#131209` ground, `#181818`/`#1F1F1F` surfaces)
  with orange as the only accent. Flat backgrounds; the hero heading gets the
  B5 text gradient.
- Shape and spacing: MD3 shape tokens snapped to Tailwind radii, nested-radius
  rule (B3); spacing only from the B2 table; main buttons 8px × 12px, 16px
  semibold.
- Motion: MD3 motion tokens replaced by `cubic-bezier(0.22,1,0.36,1)`, 500ms
  layout / 300ms color; `prefers-reduced-motion` falls back to opacity.

A `/theme/` dev-only page renders every lit-material component used, in both
themes, as the visual check for this step.

## Information architecture

| Route | Page |
|---|---|
| `/` | landing |
| `/why/` | the argument: one binary, honest comparison, who it is for |
| `/benchmarks/` | rws numbers and the named-gateway comparison, method, harness, CI run links |
| `/playground/` | config builder → live `rws.config.toml` + command |
| `/docs/…` | latest docs |
| `/docs/<major.minor>/…` | older docs, current major only |
| `/changelog/` | release notes from `CHANGELOG.md` |
| `/mcp` | docs MCP server (WEBSITE.md "Hosting and docs MCP server") |
| `/apt`, `/rpm`, `/agents/`, `llms*.txt` | unchanged |
| `/404` | branded 404 |

Shared shell: floating top nav (B7) with Home · Why · Benchmarks · Playground ·
Docs · GitHub, current page marked, full-height mobile menu; footer with Docs,
GitHub, crates.io, Docker Hub, Homebrew, `llms.txt`, license, privacy.

## Landing page (skill Part A)

Layout **A, classic hero plus sections**: the product is understood from one
terminal moment and one config file. Sections, in build order:

1. **Hero** — split. Left: headline (outcome + audience, two lines, ≤680px),
   subheadline, primary CTA "Install rws" (opens copyable install command per
   OS), secondary text link "Read the docs", one proof line (h2spec 146/146 and
   Gateway API conformance, all five profiles). Right: looping terminal demo
   (`rws` starts, `curl` returns 200).
2. **Three modes** — tabs: static server / automatic HTTPS / reverse proxy, each
   a real config with copy button and one-line result.
3. **Tagline reveal** (B11) — two-line benefit statement, words light up in
   reading order on scroll.
4. **What's in the box** — six tiles (protocol, proxy & gateway, security,
   observability, AI & MCP, database), each expanding to the feature list.
5. **The library turn** — "When config isn't enough, it's a crate": `routes!`
   sample, `Middleware` trait, the rare batteries.
6. **Performance strip** — three rws numbers from the latest run, link to
   `/benchmarks/`.
7. **Built in the open** — fuzzing, h2spec, Gateway API conformance reports
   (link the upstream PR once merged), ACME tested against Pebble, MSRV.
8. **FAQ** — 6–12 real questions (vs nginx/Caddy, production readiness,
   Windows, license, upgrade path, MCP security), with FAQ schema.
9. **Final CTA** — same install action as the hero.

Risk reversal is "MIT licensed, single binary, uninstall = delete one file".
Copy follows A5 and B1/B8: no hyphens in sentences, no banned filler, sentence
case, specific numbers. Before writing code for each section, produce the
skill's output list (outline, hero copy, benefits, how it works, FAQ, SEO) for
review.

## Docs app

- Layout from lit-material `page` + `sidebar` + `navigation` (rail on mobile):
  sidebar from `docs/sidebar.json`, on-page TOC from `##`/`###`, prev/next,
  "Edit on GitHub", version switcher, theme toggle, search.
- Components mapping: asides → `alert`; fenced code → `code-block` with copy;
  tables → styled native tables (data-table is overkill for prose tables);
  tabs (if any are added later) → `tabs`.
- **Versions.** The build checks out each version's Markdown with
  `git archive <tag> docs/src/content/docs` for: latest patch of every minor in
  the current major (18.0 … 18.8 today), newest as `/docs/`. Older tags have no
  `docs/sidebar.json`; for those the build derives the sidebar from that tag's
  `docs/astro.config.mjs` with the same parser `gen_llms.py` uses now. Each
  older-version page links "You are reading 18.x; latest is 18.y".
- SEO: canonical of every versioned page points at the latest equivalent if it
  exists; older versions are `noindex`.

## Serving

- The release image gains the built site (`site/dist`). rws serves it as static
  files, with the docs MCP server mounted at `/mcp` (WEBSITE.md).
- Redirects in the rws config: every old root doc path → `/docs/<path>`
  (301), generated from the sidebar so none is missed; `index.html` handling and
  trailing slashes match the old site.
- `/apt` and `/rpm` move from the GitHub Pages branch into the hosted site's
  data volume, still written by release.yml.
- DNS cutover only after the hosted site passes the checks below; GitHub Pages
  stays up until then as the fallback.

## Phases

1. **Theme spike** — `site/` scaffold, `theme.css`, `/theme/` page, SSR + hydrate
   of five components (button, tabs, code-block, alert, navigation). Exit:
   renders correctly in both themes, JS off still readable, phone width OK.
2. **Docs renderer** — Markdown pipeline, asides, sidebar.json (+ `gen_llms.py`
   switched to it), docs layout, Pagefind. Exit: all 107 pages render; old site
   and new site have the same page set; `ci/check_docs.py` green.
3. **Versions** — multi-tag build, switcher, canonical/noindex. Exit: 18.0–18.8
   all build.
4. **Landing** — skill outputs reviewed, then sections 1–9 in order.
5. **Other pages** — `/why/`, `/benchmarks/` (inline SVG charts from the bench
   CI output), `/playground/` (emits only keys `rws --check` accepts), `/changelog/`,
   404.
6. **Hosting** — image with `site/dist`, redirects, `/apt` `/rpm` move, health
   check; staged on a temporary hostname.
7. **CI and release** — ci.yml builds `site/` instead of `docs/`; release.yml
   deploys the image instead of pushing the Pages branch; remove Starlight.
8. **Cutover** — DNS to the hosted site, keep Pages as fallback for a week, then
   retire it.

## Status

| Phase | State |
|---|---|
| 1. Theme spike | **done 2026-10-29**, not committed: `site/` (Vite 8.3.4 + `@lit-labs/ssr` 4.1.0 prerender, plain JS modules), `src/theme.css`, `/theme/` page, SVG brand (`src/brand/mark.js` → `favicon.svg`, `brand/mark*.svg`, `brand/icon-{black,orange}.svg`), floating nav + mobile menu, theme toggle. Checked in headless Chromium: light, dark, 390px (no horizontal scroll), 20 declarative shadow roots hydrate, tabs/toggle/menu work, no console errors |
| 2. Docs renderer | **done 2026-10-29**: `docs/sidebar.json` is the one sidebar (Starlight imports it, `ci/gen_llms.py` reads it — `llms.txt` byte-identical, Starlight still builds 107 pages); `site/src/docs/` renders all 106 sidebar pages + `/docs/` index under `/docs/…` (128 asides → `lit-material-alert`, 873 fences → `lit-material-code-block` with Shiki dual-theme highlighting at build time), sidebar/TOC/prev-next/edit link, Pagefind search (106 pages, `/` shortcut). Checks: page set equals the sidebar, 0 broken internal links or anchors, all 108 pages free of horizontal scroll and console errors at 390px, `ci/check_docs.py` green; full build 12 s |
| 3. Versions | **done 2026-10-29**: `site/scripts/versions.js` builds the working tree as latest (`/docs/`) and the newest patch tag of every older minor of the current major (18.0–18.7, from `git archive` into the gitignored `site/.versions/`, 8 MB) under `/docs/<minor>/`; tags without `docs/sidebar.json` have their sidebar read from that tag's `astro.config.mjs`. Version switcher keeps the reader on the same page (else that version's index), older pages carry a "you are reading 18.x" notice, `noindex` and a canonical to the latest copy when it exists; search is filtered to the version being read (Pagefind filter `version`). Checks: 937 pages, 0 broken links or anchors across all versions, all 937 clean at 390px; build 47 s, 900 MB peak |
| 4. Landing | **done 2026-10-29**: copy approved in [WEBSITE_LANDING.md](WEBSITE_LANDING.md) (headline C, named comparison in the strip), built in `site/src/pages/home.js` + `src/landing.css`: split hero with install panel (tab picked from the visitor's OS) and a terminal replaying real `rws` 18.8.2 output, three config tabs (the README's configs), tagline reveal word by word, six tiles, three steps, performance strip, proof list, library sample, 10 FAQs (`FAQPage` + `SoftwareApplication` JSON-LD), final CTA. Also `/privacy/`, branded `/404.html`, `og.png` (1200 × 630), and `llms*.txt` + `/agents/` copied into the build. Checks: light, dark, 390px; no-JS shows everything; 0 broken links (except `/why/`, `/benchmarks/`, `/playground/`, phase 5); all 939 pages clean at 390px |
| 5–8 | not started |

Phase 1 findings:
- Brand orange `#D2551E` is 4.15:1 under white text, so interactive orange is
  `#B4461A` light / `#E2672D` dark; `#D2551E` stays for graphics.
- lit-material gaps handled in `theme.css`, worth fixing upstream:
  `lit-material-alert` sets its colour roles on `:host` (only an element
  selector overrides them); button height/padding are fixed (overridden via
  `::part(button)`); alert's built-in icons are Material-style (Phosphor
  passed in the `icon` slot instead).
- Page styles beat `::slotted`, so slotted Phosphor icons need explicit sizes.

Phase 2 findings:
- Lit SSR renders lit-material elements inside `unsafeHTML` Markdown too, so
  docs code blocks and asides arrive as declarative shadow DOM and read fine
  with JS off; no fallback styles needed.
- The docs sidebar and TOC are plain semantic `<nav>`/`<details>`, not
  lit-material `navigation`: they work without JS, and a rail/drawer is for
  a handful of app destinations, not 106 links.
- Content fixes the renderer makes: `Arc<Mutex<T>>`-style text that parsed as
  raw HTML (dropped by Starlight) is shown; links without a trailing slash get
  one; `rust,no_run` → Rust, `promql` → plain text.
- Vite's CSS minifier keeps only `-webkit-backdrop-filter` when both spellings
  are written, which Chromium ignores: write only the standard property.
- `color-mix()` over a `light-dark()` value resolves to transparent in
  Chromium; spell translucent colours per scheme.
- Pagefind's `debouncedSearch` drops the last query when several arrive while
  the index loads; the client debounces itself and calls `search()`.

Phase 3 findings:
- `dist/` is 136 MB: 123 MB of docs HTML across nine versions (about 160 KB
  per page, mostly the declarative shadow DOM of each code block), 12 MB
  gzipped. Over the wire that is ~19 KB a page since rws compresses text;
  for disk, phase 6 can write `.br` siblings (rws serves precompressed files)
  or older versions can drop SSR of code blocks if it matters.
- The release workflow must fetch tags (`fetch-depth: 0` or `git fetch
  --tags`) for the older versions to build.

Phase 4 findings:
- `rws` prints its full `--help` text on every start before the useful lines
  ("No TLS certificate configured — serving plain HTTP/1.1.", "Started 8
  per-core runtimes on 0.0.0.0:7878."), and "MCP tools not mounted" twice.
  The terminal demo shows the real lines condensed; the startup output itself
  is worth a separate look.
- Setting `lit-material-tabs.selected` moves the indicator but leaves the old
  tab highlighted; click the tab instead (upstream fix candidate).
- The Gateway API proof links to the open upstream PR #5430; switch it to the
  merged report once accepted.

New work found:
- `install.sh` for `curl -fsSL https://rws8.tech/install.sh | sh` does not exist
  yet: detect OS/arch, download the release asset, verify its `.sha256`,
  install to `~/.local/bin` (or `/usr/local/bin` with sudo). Lives in the rws
  repo, published by release.yml next to `llms.txt`.
- `og:image` (`/brand/og.png`) is referenced by the shell but not drawn yet
  (phase 4).

## Checks before cutover

- Every URL in the old site's sitemap answers 200 or 301 to a 200.
- `/apt` and `/rpm` install still work (`apt update`, `dnf makecache`).
- `llms.txt`, `llms-full.txt`, per-release copies and `/agents/` byte-equal to
  what release.yml would have produced.
- Lighthouse ≥ 95 performance and 100 accessibility on `/` and a docs page;
  keyboard-only navigation, skip link, focus rings.
- Skill quick checklist (Part B) passes on every page; no Roboto, no Material
  Icons, no off-scale sizes or spacing (a CI script greps the built CSS).
- Phone width: no horizontal scroll, 16px gutters.

## Answered (2026-10-29)

- Install on the hero: `curl | sh` is fine. Order: `curl | sh`, Homebrew,
  cargo, Docker, apt/dnf; the tab follows the visitor's OS where it can tell.
- Logo: redraw the lockup and brand sheet PNGs as SVG (phase 1), replacing
  `logo.svg`.
- No cookies, no analytics: no consent banner; the privacy page says so.
