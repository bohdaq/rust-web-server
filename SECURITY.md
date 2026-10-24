# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub:
**[Report a vulnerability](https://github.com/bohdaq/rust-web-server/security/advisories/new)**
(Security → Advisories → Report a vulnerability). Don't open a public issue
for a security problem.

Include what you can of: the affected version, the configuration or code
involved, a request or input that reproduces it, and the impact you expect.
You should get a first response within a few days. Fixes ship in a new
release with the advisory published alongside it, crediting you unless you'd
rather stay anonymous.

## Supported versions

Security fixes go into the latest release only. `rws` releases are additive
within a major version, so upgrading to the newest `17.x` is the supported
path.

## How rws is tested

- `cargo audit` and `npm audit` on every push and weekly.
- Fuzzing of every parser that sees untrusted input — HTTP requests and
  responses, Range headers, JSON, MCP JSON-RPC, forms, URLs, JWTs, SAML XML,
  and the config files — on every push and weekly. See [fuzz/README.md](fuzz/README.md).
- End-to-end checks against Let's Encrypt's test CA (Pebble) and HTTP/2
  conformance with h2spec on every push (`ci/e2e/`).
