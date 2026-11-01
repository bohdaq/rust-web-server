---
title: Fleet config
description: Many rws instances, one source of truth — instances pull a signed config from HTTPS, S3 or git, check it, apply it and report back.
---

Since 18.23.0 an instance can fetch its configuration instead of having it
copied onto it. Point it at a source, give it the public key the configs
are signed with, and it keeps itself up to date:

```toml
[fleet]
source        = "https://config.example.com/rws/{node}.toml"   # or s3://bucket/key, git+https://…#branch:path
public_key    = "kJ0pQ…base64…="       # from `rws fleet keygen` (or a path to the .pub file)
interval_secs = 30                     # how often to look (at least 5)
report_url    = "https://config.example.com/rws/report"   # optional: each outcome is POSTed here
# s3_endpoint = "https://minio.internal:9000"              # S3-compatible storage other than AWS
# insecure    = true                   # skip signatures (only for testing)
```

`{node}` is the node name (`[cluster] node`, else the host name), so each
instance can get its own file, or all can share one.

## What happens each round

1. Fetch the config — HTTP with `If-None-Match`, so an unchanged file costs
   a `304`; S3 with the standard AWS credentials (`AWS_ACCESS_KEY_ID`,
   `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`); git with a shallow clone in
   `.rws/fleet/git`, fetched and reset each round (needs `git` installed).
2. Fetch the signature next to it (`<source>.sig`) and check it against
   `public_key`. A missing or wrong signature: refused. The signature
   carries a serial, and a config whose serial isn't above the applied one
   is refused — so whoever can serve files (a bucket, a mirror) can't roll
   instances back to an older signed config (since 18.24.0; the serial is
   kept in `.rws/fleet/serial` across restarts).
3. Check the config as `rws --check` does, on this machine (certificate
   files, secrets in the environment), and that its `[fleet]` table is the
   same as the local file's: the fleet can't change where it pulls from, the
   key, or `insecure`. Errors: refused.
4. Write it over the local config file (atomically) and reload, as `SIGHUP`
   does. A reload that reports errors: refused, and the previous file is put
   back, so a restart doesn't load what the running server refused.
5. POST `{"node":…,"rws":…,"applied":true,"config":"<sha-256>"}` — or
   `"applied":false` with the `error` — to `report_url`.

A refused config never replaces the running one. `GET
/admin/api/v1/status` (and `rws ctl status`) shows the source, the
applied config's hash, the last check and the last error.

## Signing

```sh
rws fleet keygen ops            # ops.key (secret, 0600) and ops.pub
rws fleet sign web-1.toml --key ops.key   # writes web-1.toml.sig — refuses a config that fails rws --check
# the serial defaults to the current Unix time; --serial N sets it
aws s3 cp web-1.toml s3://ops-config/rws/ && aws s3 cp web-1.toml.sig s3://ops-config/rws/
```

Signatures are Ed25519 over a domain tag, the serial and the file's exact
bytes; the `.sig` file is one line, `rws-fleet-sig v2 <serial> <base64>`.
18.23's format (the body alone, no serial) is still accepted until an
instance has applied a v2 file, but it can't stop a rollback — re-sign with
18.24. Keep `ops.key` in your
CI's secret store and sign as the last step of the pipeline that reviewed
the change; instances only need `ops.pub`.

## What reloads and what doesn't

The fetched file replaces the whole config file, so it carries the same
`[fleet]` table as the instance's file (or put the fleet settings in the
environment as `RWS_CONFIG_FLEET_*` and leave them out of both). A reload applies what `SIGHUP` applies: routes,
upstreams, middleware, CORS, rate limits, log format and certificates.
Listen addresses, the admin listener, `[fleet]` itself and switching
between serving files only and config-driven routes (the first `[[route]]`)
take a restart — start instances with at least one route if their configs
will have routes.

The fleet config is the declared layer. Runtime changes through the admin
API — blocks, drains, maintenance — are the live layer, shared across
replicas by [`[cluster]`](/deployment/cluster/), and aren't overwritten by
a fleet update.
