---
title: Support policy
description: Which rws releases get fixes and for how long, the long-term-support (LTS) lines, security response targets and how fixes are released.
---

## Release lines

rws follows [semver](https://semver.org/). Minor releases (18.21, 18.22, …)
are additive: a configuration and an app that work on one 18.x work on
every later 18.x. Breaking changes wait for a major release and are listed
on a migration page ([Migrating to 18](/reference/migrating-18/)).

| Line | Gets | For how long |
|---|---|---|
| **Current** — the newest minor | every fix | until the next minor |
| **LTS** — one minor every six months | security fixes and critical bug fixes | 12 months from its release |
| Everything else | nothing | — |

LTS lines so far:

| Line | Released | Supported until | Branch |
|---|---|---|---|
| 18.20 | 2026-11-01 | 2027-11-01 | `18.20.x` |

"Critical" means data loss, a crash or hang reachable by clients, a
regression that stops a documented configuration from working, or a broken
certificate renewal. New features never go to an LTS line.

## Security fixes

Reports go through GitHub's private advisories (see
[SECURITY.md](https://github.com/bohdaq/rust-web-server/blob/main/SECURITY.md)).
Targets: an answer within 3 working days, an assessment within 7 days, and
fixed releases for critical and high issues within 14 days — on the current
line and every supported LTS line at once — published with a GitHub
security advisory and a CVE.

## Upgrading

Within a major version, upgrade to the newest minor: read the changelog's
**Changed** and **Security** entries, run `rws --check` on your
configuration with the new binary, then replace it (the
[packages](/deployment/systemd/) and the container images make this one
step). Staying on an LTS line trades features for fewer changes: patch
releases on it (`18.20.1`, `18.20.2`) contain only fixes.

## Verifying what you run

Release binaries have build-provenance attestations, container images are
signed with Sigstore cosign, and each release ships a CycloneDX SBOM — see
[Installation](/getting-started/installation/) for the commands.
