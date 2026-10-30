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
cargo generate --git https://github.com/bohdaq/rust-web-server starters/fullstack   # lit frontend + API + PostgreSQL, Dockerfile, GitHub Actions deploy to kube-dc with PR previews
```

([cargo-generate](https://github.com/cargo-generate/cargo-generate): `cargo install cargo-generate`.) Without it, copy the directory and replace `{{project-name}}` in `Cargo.toml`.

## Give the agent the rws skill

[`agents/`](https://github.com/bohdaq/rust-web-server/tree/main/agents) in the repository (also at `https://rws8.tech/agents/`) has the same guidance in each tool's format — the project shape, recipes for routes, JSON, middleware, OpenAPI, MCP tools, tests, and the pitfalls:

| Tool | File | Put it at |
|---|---|---|
| Claude Code, Codex and other Agent Skills readers | `agents/skills/rws/SKILL.md` | `.claude/skills/rws/SKILL.md` in the project, or `~/.claude/skills/rws/` for every project |
| Cursor | `agents/cursor/rws.mdc` | `.cursor/rules/rws.mdc` |
| GitHub Copilot | `agents/copilot-instructions.md` | `.github/copilot-instructions.md` |

## Connect the docs MCP server

`https://rws8.tech/mcp` is an MCP server that answers questions about rws for the release your project uses, so an agent looks things up instead of guessing from an older release or another framework. It is public and read-only (no token).

```bash
claude mcp add --transport http rws https://rws8.tech/mcp
```

Other clients take the same URL as a remote (Streamable HTTP) server, e.g. VS Code's `.vscode/mcp.json`: `{"servers": {"rws": {"type": "http", "url": "https://rws8.tech/mcp"}}}`.

| Tool | Answers |
|---|---|
| `versions` | the releases it has docs for, and every page path |
| `search_docs` | the best-matching sections, with page path and link |
| `get_page` | one page as Markdown |
| `get_example` | an `examples/*.rs` program and how to run it |
| `api_item` | the API reference entry for a type or function, with a docs.rs link |
| `setting` | an `RWS_CONFIG_*` setting or `rws.config.toml` key: meaning, default, type |
| `check_config` | what `rws --check` says about a config file (offline: file paths and secrets aren't checked) |
| `changes` | changelog entries between two versions, breaking changes first |

Every tool takes an optional `version` (`"18.7"` or `"18.7.2"`; default: the newest), covering every minor release of the current major. Pages are also resources (`rws-docs://18.8/building-apps/routing`), and the `start_rws_app` prompt gives an agent the project shape for a release. Requests are rate limited per IP.

## Point it at the docs of your release

Every docs page in one file, for pasting into a chat or a system prompt, or for an agent to fetch:

- `https://rws8.tech/llms-full.txt` (and the index, `llms.txt`) — the newest release;
- `https://rws8.tech/<major.minor>/llms-full.txt`, e.g. `/18.7/` — the release your `Cargo.toml` names, kept for every release from 18.7 on.

The library's own conventions for agents working **on** the crate (rather than with it) are in the repository's `CLAUDE.md`.
