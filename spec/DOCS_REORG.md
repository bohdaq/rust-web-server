# Docs reorganization (2026-10-25)

Goal: one home per fact. The Astro site (`docs/`) is the user documentation;
rustdoc documents the API; root files point there instead of repeating it.

| # | Step | Status |
|---|------|--------|
| 1 | Archive finished plans in `spec/` to `spec/archive/`; one live `ROADMAP.md`, one `IDEAS.md` | todo |
| 2 | `src/**/README.md` → `//!` module docs (rustdoc / docs.rs); delete the READMEs and `src/README.md` | todo |
| 3 | `CONFIGURE.md` → pointer to the configuration pages | todo |
| 4 | `FAQ.md` → `docs/.../reference/troubleshooting.md`; root file becomes a pointer | todo |
| 5 | `DEVELOPER.md` use cases → docs pages; building blocks → `reference/api.md`; DEVELOPER.md = contributor guide | todo |
| 6 | `README.md` slimmed: pitch, quick start, feature list, links | todo |
| 7 | New pages: migrating to 18.0, architecture; `CHANGELOG.md` | todo |
| 8 | `CLAUDE.md`: trim history narrative; new single-source doc rule | todo |
| 9 | `llms.txt` / `llms-full.txt` generated from the docs site; CI fails when stale | todo |
| 10 | CI: internal link check, every `RWS_CONFIG_*` documented, doctest of complete doc pages | todo |

## Decisions

- Root `CONFIGURE.md`, `FAQ.md` stay as short pointer files so old links keep working.
- `llms.txt` keeps its raw GitHub URL; it becomes an index of the site (llmstxt.org style)
  plus the hand-written overview; `llms-full.txt` holds every page.
- Docs site changes appear on rws8.tech only at the next release (site is rebuilt per release).

## Progress log
