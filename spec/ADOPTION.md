# Adoption: developers and enterprises (18.32–18.33)

What rws still needs for adoption by individual developers / startups and by
enterprises, beyond the framework work in
[AGENT_FIRST_FRAMEWORK.md](AGENT_FIRST_FRAMEWORK.md). Written 2026-11-04 from
a review of the tree at 18.27.2 (advisor review, spot-checked). Roadmap
items: #96–#107 in [ROADMAP.md](ROADMAP.md).

## Developers and startups

Already there: `install.sh`, cargo, signed deb/rpm repos, winget, nix,
Homebrew (`bohdaq/homebrew-tap`, updated by `release.yml`), Docker Hub/GHCR,
Helm; quick start, 13 `examples/`, 3 starters, a cookbook; a SemVer promise
(`support-policy.md`) with the public-API gate (`ci/public_api.sh`); issue
templates and Discussions; benchmarks against nginx and Caddy; the agent
pack, per-release llms files and the docs MCP server.

| Need | Missing | Size |
|---|---|---|
| API docs | no `[package.metadata.docs.rs]`: docs.rs builds the default features only, so `model`, `sso`, `openapi`, `jobs`, `storage-*` items are invisible there | S |
| Feature flags | 37 features, no presets; no compile-time numbers per preset | S |
| First 10 minutes | a library quick start that goes `cargo add` → route → database → template → test, run by CI | S |
| Discoverability | README leads with the server, the library comes fifth; crate `rust-web-server` vs brand `rws` | S |
| Migration guides | "Coming from axum / actix", nginx / Caddy config → `rws.config.toml` | M |
| Deploy | Fly.io, Cloud Run, Railway, Render recipes; a Dockerfile template for library apps (only the binary has `Dockerfile.release`) | S |
| Stability | a written deprecation policy (how long a deprecated item stays) | S |
| Community | a chat channel, CODEOWNERS, a showcase of users | S |

## Enterprises

Already there: `SECURITY.md` (private advisories, response targets), weekly
`cargo audit`/`cargo deny`, 43 fuzz targets in CI, CycloneDX SBOM,
cargo-auditable, build-provenance attestations, keyless cosign images,
GPG-signed repos, `deny.toml` license allow-list; FIPS and PKCS#11 builds;
OIDC/SAML, admin roles + audit log; cluster state over Redis; 12-month LTS;
Gateway API, h2spec, MCP and A2A conformance runs.

| Need | Missing | Size |
|---|---|---|
| Security monitoring (SIEM) | OTLP logs, syslog/CEF export; IP anonymisation and header redaction in access logs (none in `log/`); a hash-chained (tamper-evident) audit log; a retention statement | M |
| Independent validation | a third-party audit of the hand-written parsers (HTTP/1, h2/HPACK, SAML XML, WAF), published; an advisories page | M (funding) |
| Supply chain | OpenSSF Scorecard, reproducible builds (both in IDEAS until now), a stated SLSA level, a signed Helm chart | S–M |
| HA / DR / upgrades | a runbook: Redis loss, config rollback, `/data` backup, zero-downtime upgrade, rollback of the jobs/session/authz schemas; an N-1 → N compatibility e2e | S–M |
| Identity | SCIM provisioning and LDAP/AD for admin users; admin permissions scoped per host/tenant | M |
| Platforms | a UBI-based image, Chocolatey/MSI | S |
| Observability | recipes for Datadog, Splunk, Elastic, New Relic | S |
| Maturity | a production / preview tier per module, with its conformance and e2e status | S |

## Outside the code

These are the owner's to decide; listed so they are not lost.

1. **Bus factor.** A second maintainer with release and advisory rights,
   written custody of the release keys, CODEOWNERS, a "if the maintainer is
   unavailable" clause in the support policy; possibly a fiscal host.
2. **No independent evidence.** Fund one audit, publish the advisories page
   even when empty, name two adopters in the README, submit to TechEmpower.
3. **Breadth vs trust.** Module maturity tiers (#106) and an LTS line that
   freezes scope.
4. **Commercial support.** An offer with response times, which procurement
   asks for.

## Releases

### 18.32.0 — Developer adoption

- **#96 docs.rs and feature presets** — `[package.metadata.docs.rs]
  all-features = true` (with `doc_cfg` labels); presets such as `web`, `api`,
  `proxy`, `full`; compile times per preset in `installation.md`.
- **#97 Library quick start** — `cargo add` → route → database → template →
  test, on the docs front page, run by CI.
- **#98 Migration guides** — from axum and actix; nginx / Caddy config to
  `rws.config.toml`.
- **#99 README for two audiences** — library first, server second.
- **#100 Deploy recipes** — Fly.io, Cloud Run, Railway, Render; a library-app
  Dockerfile template.
- **#101 Deprecation policy** in `support-policy.md`.

Done when: docs.rs shows every feature's items; the quick start and each
recipe's Dockerfile build in CI.

### 18.33.0 — Enterprise readiness

- **#102 Logs for security monitoring** — OTLP logs, syslog/CEF, IP
  anonymisation and header redaction, hash-chained audit log.
- **#103 Supply chain** — OpenSSF Scorecard, reproducible builds, SLSA
  statement, signed Helm chart.
- **#104 HA/DR and upgrade runbook** — with an N-1 → N compatibility e2e.
- **#105 SCIM and LDAP for admin users**, admin roles scoped per host.
- **#106 Module maturity tiers** — production / preview per module, on the
  docs home with conformance status.
- **#107 Observability recipes** — Datadog, Splunk, Elastic, New Relic.

Done when: an OTLP collector receives logs in `ci/e2e/otlp.sh`; a release
build reproduces bit for bit; the compatibility e2e passes for 18.31 → 18.33.
