# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub:
**[Report a vulnerability](https://github.com/bohdaq/rust-web-server/security/advisories/new)**
(Security → Advisories → Report a vulnerability). Don't open a public issue
for a security problem.

Include what you can of: the affected version, the configuration or code
involved, a request or input that reproduces it, and the impact you expect.

## What happens next

| Step | Target |
|---|---|
| First response, acknowledging the report | 3 working days |
| Assessment: severity (CVSS 4.0), affected versions, a fix plan | 7 days |
| Fix released for **critical** and **high** issues | 14 days |
| Fix released for **medium** and **low** issues | the next release, at most 90 days |

Fixes ship as a new release on every [supported line](#supported-versions)
at the same time, with a GitHub security advisory and a CVE (requested
through GitHub) published alongside them, crediting you unless you'd rather
stay anonymous. Disclosure is coordinated: details stay private until the
fixed releases are out, or 90 days after the report, whichever comes first,
unless we agree otherwise.

## Supported versions

| Line | Status | Security and critical fixes until |
|---|---|---|
| Latest 18.x minor | current | the next minor release |
| 18.20 | long-term support (LTS) | 2027-11-01 |
| 17.113 | security fixes ended | 2026-11-01 |
| older | not supported | — |

Every six months a minor release is named LTS and gets security and
critical bug fixes for twelve months on a `<minor>.x` branch (for example
`18.20.x` → `18.20.1`, `18.20.2`). Other releases are supported until the
next minor: releases are additive within a major version, so upgrading to
the newest 18.x is the supported path. The policy in full:
[Support policy](https://rws8.tech/reference/support-policy/).

## Verifying a release

Release binaries carry GitHub build-provenance attestations and container
images are signed keyless with Sigstore cosign; each release has a
CycloneDX SBOM:

```sh
gh attestation verify rws-<version>-x86_64-unknown-linux-musl.tar.gz --repo bohdaq/rust-web-server
cosign verify ghcr.io/bohdaq/rws:<version> \
  --certificate-identity-regexp '^https://github.com/bohdaq/rust-web-server/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## How rws is tested

- `cargo audit` and `npm audit` on every push and weekly.
- Fuzzing of every parser that sees untrusted input — HTTP requests and
  responses, HTTP/2 frames and HPACK, Range headers, JSON, MCP JSON-RPC,
  forms, URLs, JWTs, SAML XML, FastCGI records, the WAF's decoders, the
  config files — on every push and weekly. See [fuzz/README.md](fuzz/README.md).
- Every such parser also runs on a 256 KB stack (`src/stack_depth_tests.rs`),
  so no input can overflow it.
- End-to-end checks against real implementations on every push (`ci/e2e/`):
  Let's Encrypt's test CA (Pebble), h2spec (146/146), gRPC, php-fpm,
  Keycloak, Redis, Prometheus, the OpenTelemetry collector, HAProxy, the
  Kubernetes Gateway API conformance suite, and more.
