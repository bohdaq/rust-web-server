---
title: AI coding agents
description: Starters, an agent skill, and per-release llms.txt so Claude Code, Codex, Cursor and Copilot write correct rust-web-server code.
---

Coding agents mostly go wrong on rws by guessing its API — from an older release, or from axum. Three things give them the right context for the release your project uses.

## Start from a starter

Each starter is a standalone project that builds and passes its own tests (CI checks them on every change), with an `AGENTS.md` that tells an agent how the project is laid out and how rws works:

```sh
cargo generate --git https://github.com/bohdaq/rust-web-server starters/api   # JSON API + OpenAPI 3.1, TestClient tests
cargo generate --git https://github.com/bohdaq/rust-web-server starters/mcp   # MCP server: typed tools, a resource, a bearer token
```

([cargo-generate](https://github.com/cargo-generate/cargo-generate): `cargo install cargo-generate`.) Without it, copy the directory and replace `{{project-name}}` in `Cargo.toml`.

## Give the agent the rws skill

[`agents/`](https://github.com/bohdaq/rust-web-server/tree/main/agents) in the repository (also at `https://rws8.tech/agents/`) has the same guidance in each tool's format — the project shape, recipes for routes, JSON, middleware, OpenAPI, MCP tools, tests, and the pitfalls:

| Tool | File | Put it at |
|---|---|---|
| Claude Code, Codex and other Agent Skills readers | `agents/skills/rws/SKILL.md` | `.claude/skills/rws/SKILL.md` in the project, or `~/.claude/skills/rws/` for every project |
| Cursor | `agents/cursor/rws.mdc` | `.cursor/rules/rws.mdc` |
| GitHub Copilot | `agents/copilot-instructions.md` | `.github/copilot-instructions.md` |

## Point it at the docs of your release

Every docs page in one file, for pasting into a chat or a system prompt, or for an agent to fetch:

- `https://rws8.tech/llms-full.txt` (and the index, `llms.txt`) — the newest release;
- `https://rws8.tech/<major.minor>/llms-full.txt`, e.g. `/18.7/` — the release your `Cargo.toml` names, kept for every release from 18.7 on.

The library's own conventions for agents working **on** the crate (rather than with it) are in the repository's `CLAUDE.md`.
