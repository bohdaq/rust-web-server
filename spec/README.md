# spec/

Plans and design notes for work on `rws` itself. User documentation lives in
the docs site ([docs/src/content/docs/](../docs/src/content/docs/), published
at https://rws8.tech); API docs are rustdoc.

| File | What |
|------|------|
| [ROADMAP.md](ROADMAP.md) | Open work, in order |
| [IDEAS.md](IDEAS.md) | Unordered ideas, not yet planned |
| [PROXY_STATUS_PROBLEM_JSON.md](PROXY_STATUS_PROBLEM_JSON.md) | Proxy-Status, Cache-Status, problem+json (18.17.0) |
| [A2A_SERVER.md](A2A_SERVER.md) | A2A 1.0 server and gateway (18.18.0) |
| [OPENAPI_TO_MCP.md](OPENAPI_TO_MCP.md) | MCP tools from OpenAPI documents and routes (18.19.0) |
| [OPERATIONS_AND_TRUST.md](OPERATIONS_AND_TRUST.md) | systemd notify/socket activation, Forwarded + CRLs, key rotation, rwsctl (18.20.0) |
| [TEAMS_AND_ENTERPRISE.md](TEAMS_AND_ENTERPRISE.md) | Local HTTPS, FastCGI, config SSO, cookbook, admin roles/audit, cluster state, WAF, FIPS, PKCS#11, fleet config, LTS (18.21–18.23) |
| [DEMOS_TODO.md](DEMOS_TODO.md) | Planned demo applications |
| [KAFKA_ROADMAP.md](KAFKA_ROADMAP.md) | Kafka integration plan (unbuilt) |
| [WASM_SHIM.md](WASM_SHIM.md) | `wasm32-wasip2` guest component (mostly shipped) |
| [DOCS_REORG.md](DOCS_REORG.md) | Documentation reorganization (2026-10) |
| [WEBSITE.md](WEBSITE.md) | Plan for a marketing front on the docs site |
| [archive/](archive/) | Finished or superseded plans, kept for history |

When a plan is done, move it to `archive/` and drop its items from `ROADMAP.md`.
