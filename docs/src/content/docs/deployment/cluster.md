---
title: Several replicas
description: Run rws on several machines behind a load balancer and keep runtime changes — the IP blocklist, maintenance mode, drained backends, feature flags and cache purges — the same on all of them through Redis.
---

Most of rws's configuration is the config file, so replicas agree by
running the same file. Changes made at runtime through the
[admin API](/deployment/admin/), `rws ctl` or the MCP tools are different:
on their own they stay on the replica that took them. Point the replicas at
one Redis (since 18.22.0) and those changes reach every replica within
about a second:

```toml
[cluster]
redis  = "10.0.0.20:6379"     # defaults to [acme] redis when that's set
# redis_password = "…"        # or RWS_CONFIG_CLUSTER_REDIS_PASSWORD (never printed)
# prefix = "rws:cluster:"     # key prefix, when several clusters share a Redis
# node   = "web-1"            # this replica's name (default: the host name)
```

Shared: the IP blocklist, maintenance mode, drained upstream backends,
feature flags, and purges of named caches (each replica purges its own).
Not shared: anything in the config file (deploy the same file), ACME
certificates (use [`[acme] redis`](/features/acme/) for those, which can be
the same Redis), rate-limit counters, sessions.

```sh
rws ctl --admin web-1:9901 block 203.0.113.9     # refused on every replica
rws ctl --admin web-2:9901 status
# cluster   node web-2 · Redis up · 3 node(s): web-1, web-2, web-3
```

How it works: one JSON document in Redis (`<prefix>state`) holds the shared
state and a version number. A change applies on the replica that took it,
then a Lua script applies the same change to the document inside Redis, so
changes made at the same moment on different replicas don't overwrite
each other. Every replica reads the document once a second and makes its own
state match when the version moved. Each replica also writes a heartbeat
(`<prefix>nodes`); `GET /admin/api/v1/status` lists the replicas seen in
the last minute.

When Redis is unreachable, changes still apply on the replica that took
them, `rws ctl status` shows them as waiting, and they're sent when Redis is
back. A Redis that comes back empty is filled again from the replicas' own
state, so nothing they agreed on is undone. A replica started later adopts
the document. Audit-log entries name the replica (`node`).

Redis needs Lua scripting (every Redis since 2.6, Valkey, KeyDB, managed
Redis services); run it on a private network or with a password.
