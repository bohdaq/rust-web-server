# rws for coding agents

Context that makes coding agents (Claude Code, Codex, Cursor, Copilot, …)
write correct rust-web-server code. Published at https://rws8.tech/agents/
with every release.

| File | For | Install |
|---|---|---|
| `skills/rws/SKILL.md` | Agent Skills: Claude Code, Codex and other tools that read `SKILL.md` | copy `skills/rws/` to `.claude/skills/rws/` in a project (or `~/.claude/skills/rws/` for every project) |
| `cursor/rws.mdc` | Cursor | copy to `.cursor/rules/rws.mdc` |
| `copilot-instructions.md` | GitHub Copilot | copy to `.github/copilot-instructions.md` (or append to it) |

`cursor/rws.mdc` and `copilot-instructions.md` are generated from `SKILL.md`
by `ci/gen_agents.py` — edit `SKILL.md` only. The starters in `starters/`
carry their own `AGENTS.md`, and the docs of every release are at
`https://rws8.tech/<major.minor>/llms-full.txt`.
