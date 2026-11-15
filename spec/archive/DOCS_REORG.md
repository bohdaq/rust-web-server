# Docs reorganization (2026-10-25)

Goal: one home per fact. The Astro site (`docs/`) is the user documentation;
rustdoc documents the API; root files point there instead of repeating it.

| # | Step | Status |
|---|------|--------|
| 1 | Archive finished plans in `spec/` to `spec/archive/`; one live `ROADMAP.md`, one `IDEAS.md` | done |
| 2 | `src/**/README.md` → `//!` module docs (rustdoc / docs.rs); delete the READMEs and `src/README.md` | done |
| 3 | `CONFIGURE.md` → pointer to the configuration pages | done |
| 4 | `FAQ.md` → `docs/.../reference/troubleshooting.md`; root file becomes a pointer | done |
| 5 | `DEVELOPER.md` use cases → docs pages; building blocks → `reference/api.md`; DEVELOPER.md = contributor guide | done |
| 6 | `README.md` slimmed: pitch, quick start, feature list, links | done |
| 7 | New pages: migrating to 18.0, architecture; `CHANGELOG.md` | done |
| 8 | `CLAUDE.md`: trim history narrative; new single-source doc rule | done |
| 9 | `llms.txt` / `llms-full.txt` generated from the docs site; CI fails when stale | done |
| 10 | CI: internal link check, every `RWS_CONFIG_*` documented, doctest of complete doc pages | done |

## Decisions

- Root `CONFIGURE.md`, `FAQ.md` stay as short pointer files so old links keep working.
- `llms.txt` keeps its raw GitHub URL; it becomes an index of the site (llmstxt.org style)
  plus the hand-written overview; `llms-full.txt` holds every page.
- Docs site changes appear on rws8.tech only at the next release (site is rebuilt per release).

## Progress log

- 2026-10-25 — all ten steps done. Commits: 6e36d763 (spec archive), 563846cf (rustdoc module
  docs), a9bd6da9 (CONFIGURE/FAQ → site), 98e4bbb0 (CHANGELOG, migrating-18, architecture),
  c91fd2d9 (README), then one commit for DEVELOPER.md → site, CLAUDE.md, llms generation and CI.
- DEVELOPER.md 5,126 → 183 lines (contributor guide); its use cases merged into ~20 pages, 65 dropped
  as already covered; building blocks → reference/api.md (202 rows). README 415 → 193 lines.
- llms.txt is now an index (154 lines) and llms-full.txt every page (~17.6k lines), both generated.
- CI `docs-check`: internal links + anchors, repo-relative links, every RWS_CONFIG_* on env-vars.md,
  llms freshness; README's Rust example compiles as a doctest (`ReadmeDoctests`). `npm run build` passes.
- Not done: compiling the docs-site snippets (most are fragments, not programs).
