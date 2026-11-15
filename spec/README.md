# spec/

Plans and design notes for work on `rws` itself. User documentation lives in
the docs site ([docs/src/content/docs/](../docs/src/content/docs/), published
at https://rws8.tech); API docs are rustdoc.

| File | What |
|------|------|
| [ROADMAP.md](ROADMAP.md) | Open work, in order |
| [IDEAS.md](IDEAS.md) | Unordered ideas, not yet planned |
| [ADOPTION.md](ADOPTION.md) | Developer and enterprise adoption: docs.rs, migration guides, deploy recipes, SIEM logs, supply chain, HA/DR, SCIM/LDAP (18.32–18.33) |
| [AGENT_FIRST_FRAMEWORK.md](AGENT_FIRST_FRAMEWORK.md) | Agent-first framework: gap review vs Rails/Spring, dev MCP, compile-checked routes, generators, ORM, auth flows (18.28–18.31) |
| [DEMOS_TODO.md](DEMOS_TODO.md) | Planned demo applications |
| [KAFKA_ROADMAP.md](KAFKA_ROADMAP.md) | Kafka integration plan (unbuilt) |
| [WASM_SHIM.md](WASM_SHIM.md) | `wasm32-wasip2` guest component (mostly shipped) |
| [WEBSITE.md](WEBSITE.md) | Plan for a marketing front on the docs site |
| [archive/](archive/) | Finished or superseded plans, kept for history |

When a plan is done, move it to `archive/` and drop its items from `ROADMAP.md`.
