---
title: Operate rws from an AI client
description: Point Claude Code, Claude Desktop or Cursor at a running rws and let it read metrics and logs, block an abusive IP, or switch on maintenance mode.
---

The `rws` binary has an MCP endpoint with 20 tools for looking at and steering
the running server: its configuration, metrics, recent requests and errors,
the response cache, the IP blocklist, maintenance mode, runtime feature flags
and config reload. An AI client connected to it can answer "why are we
throwing 500s?" from the server's own request log and act on the answer.

## Turn it on

The endpoint exists only with a token (since 17.113.1 — its tools change the
running server):

```bash
export MCP_TOKEN="$(openssl rand -hex 32)"
rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```

- `/mcp` then requires `Authorization: Bearer $MCP_TOKEN`. Without
  `MCP_TOKEN`, `/mcp` is `404`.
- Serve it over HTTPS (or keep the port private): the token travels in every
  request.
- Browser-based clients send an `Origin` header; list theirs in
  `MCP_ALLOWED_ORIGINS` (comma-separated), any other origin gets `403`.
  Desktop and CLI clients send none and need nothing here.
- `/mcp` sits behind the access log, the IP blocklist and the global rate
  limit, like every other path.

In config-driven proxy mode (`[[route]]` / `[[upstream]]` in
`rws.config.toml`) the built-in tools aren't mounted.

## Connect a client

**Claude Code:**

```bash
claude mcp add --transport http rws https://www.example.com/mcp \
  --header "Authorization: Bearer $MCP_TOKEN"
```

**Claude Desktop** (`claude_desktop_config.json`) and **Cursor**
(`.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "rws": {
      "url": "https://www.example.com/mcp",
      "headers": { "Authorization": "Bearer <MCP_TOKEN>" }
    }
  }
}
```

Each tool carries behaviour hints: the 14 that only read are marked
read-only, and the ones that change something (`set_maintenance`, `block_ip`,
`unblock_ip`, `cache_clear`, `feature_set`, `reload_config`) are not, so a
client that asks before acting asks for those.

## The tools

| Look | Change |
|---|---|
| `server_config` — every active `RWS_CONFIG_*` value, secrets hidden | `reload_config` — re-read config and certificates |
| `server_metrics` — request, error, connection counters | `set_maintenance` `{"enabled": true}` — `503` for everything but health checks |
| `recent_requests` `{"n": 50}` — last requests (max 100) | `block_ip` / `unblock_ip` `{"ip": "…"}` — runtime blocklist (`403`) |
| `recent_errors` `{"n": 50}` — last 4xx/5xx | `cache_clear` — empty the response cache |
| `list_blocked_ips`, `maintenance_status` | `feature_set` `{"name": "…", "enabled": false}` |
| `cache_stats`, `rate_limit_config`, `check_rate_limit` `{"ip": "…"}` | |
| `cors_config`, `list_routes`, `list_static_files` | |
| `feature_flags`, `feature_list` | |

The same operations are on the [admin API](/deployment/admin/) (since
18.10.0), which runs the same code: a tool and its admin endpoint answer
alike.

A call with missing or mistyped arguments comes back as an error the model
reads (`missing required property "ip"`), not a silent default.

## Playbooks

**A spike of errors.** Ask: *"Look at the last 100 errors. Is one client
responsible? If an IP is sending most of them and they look like scanning,
block it and confirm."* The client calls `recent_errors`, groups by `ip`, and
calls `block_ip`; follow-up requests from that address get `403`.
`list_blocked_ips` and `unblock_ip` undo it.

**A bad deploy.** Ask: *"Errors started a minute ago. Put the site in
maintenance mode, then show me the config."* `set_maintenance` with
`{"enabled": true}` makes every non-health path answer `503` while health
checks still pass; fix the config, ask for `reload_config`, check
`recent_errors`, and turn maintenance off again.

**Before a change.** Ask: *"What would change if I disabled the `beta-ui`
feature?"* — `feature_list`, then `feature_set` once you agree.

## What it can't do

The tools act on this process only — a second replica has its own
blocklist, maintenance switch and request log. They don't restart the
server, edit `rws.config.toml` or read files outside the static root. The
blocklist and maintenance mode are in memory: a restart clears them.
