# rws8.tech landing page — content for review

Written 2026-10-29 for phase 4 of [WEBSITE_REBUILD.md](WEBSITE_REBUILD.md),
in the `landing-page-design` skill's output format. Nothing is built from it
until it is approved. Copy follows the skill: sentence case, no hyphens inside
sentences, no filler words, specific numbers. Numbers come from
`docs/src/content/docs/reference/benchmarks.md` (run 40243377254) and the
Gateway API benchmark (run 40681376798).

**One offer:** a free web server you can run in one minute.
**One audience:** developers and operators choosing a web server or reverse proxy.
**One action:** install rws.

## 1. Page outline

1. Hero: headline, subheadline, Install rws, proof line, terminal demo
2. Three modes: tabs with a real config each (static, HTTPS, proxy)
3. Tagline reveal
4. What's in the box: six tiles
5. How it works: three steps
6. Performance strip: three numbers, link to /benchmarks/
7. Built in the open: the tests rws passes, with links
8. The library turn: when config isn't enough, it's a crate
9. FAQ
10. Final CTA: the same install action

## 2. Hero copy

**Headline** (pick one; line break marked `/`):

- A. **One binary serves, secures / and routes your web traffic**
- B. **HTTPS, proxy and static files / from one binary**
- C. **The web server that gets its own / certificates and routes your apps**

**Subheadline:** Static files, automatic Let's Encrypt certificates, HTTP/3 and
a load balancing reverse proxy, configured in one TOML file. When you need
code, the same engine is a Rust crate.

**Primary CTA:** `Install rws` — opens the install panel with the visitor's OS
selected:

| Tab | Command |
|---|---|
| Script (Linux, macOS) | `curl -fsSL https://rws8.tech/install.sh \| sh` |
| Homebrew | `brew install bohdaq/tap/rws` |
| Cargo | `cargo binstall rust-web-server` |
| Docker | `docker run -p 7878:7878 -v "$PWD:/srv" bohdaq/rws` |
| apt / dnf | `sudo apt install rws` after adding the repository (link) |

**Secondary link:** Read the docs

**Proof line:** Gateway API v1.6.3 conformant · h2spec 146 of 146 · MIT licensed

**Hero visual:** looping terminal: `cd my-site && rws` → startup lines →
`curl -I http://localhost:7878` → `HTTP/1.1 200 OK`. Static frame when
reduced motion is on.

## 3. Benefits (the six tiles, outcome first)

- **One process instead of four.** Static files, certificates, redirects and
  the proxy run in one binary; no certbot, cron job or sidecar.
- **Certificates that look after themselves.** rws orders them on first start,
  renews at two thirds of their lifetime and reloads without dropping
  connections. HTTP-01, TLS-ALPN-01 and DNS-01.
- **Traffic you can steer.** Round robin, least connections, IP hash, weights,
  health checks, retries, circuit breakers, sticky sessions, canary splits.
- **Protection on by config.** Bearer, JWT and basic auth that fail closed,
  rate limits, IP allow and deny lists, request size limits.
- **Answers you can measure.** Prometheus metrics with native histograms,
  OpenTelemetry traces and metrics, JSON access logs.
- **Kubernetes without another controller.** Built with `gateway-api`, rws is
  the Gateway API controller and data plane: HTTP, gRPC, TLS, TCP and UDP
  routes, installed with the Helm chart.

## 4. How it works

1. **Install one binary.** Script, Homebrew, Cargo, Docker, apt or dnf.
2. **Write one file.** `rws.config.toml`; `rws --check` validates it before
   anything starts.
3. **Run `rws`.** It serves and gets certificates, and reloads on `SIGHUP`
   (or by itself when the file changes, with `watch = true`).

## 5. Tagline reveal (two lines)

> One binary serves your files, gets your certificates
> and routes your traffic. One file tells it how.

## 6. Performance strip

Same 4 CPU runner, same load, HTTP/1.1, 64 connections:

| Number | Caption |
|---|---|
| **96,670 req/s** | 1 KiB static file. nginx: 81,161. Caddy: 18,592. |
| **16.9 µs** | CPU per static request. nginx: 25.1 µs. |
| **1.9×** | Gateway API throughput of the next gateway (38,501 vs 20,225 req/s, kgateway). |

Link: **See every scenario and how to reproduce it** → `/benchmarks/`
(the full tables, including the scenarios where nginx uses less memory).

## 7. Built in the open

- h2spec: 146 of 146 HTTP/2 conformance tests
- Gateway API v1.6.3: all five Gateway profiles, core and extended (link to
  the upstream report once merged)
- ACME tested end to end against Pebble, Let's Encrypt's test CA
- Every parser of untrusted input is fuzzed
- Releases carry build attestations and a cosign signature
- MSRV pinned (Rust 1.99), MIT licensed

## 8. The library turn

**When config isn't enough, it's a crate.** The same engine as
`rust-web-server` on crates.io: routes with typed extractors, one
`Middleware` trait, an MCP server, OAuth, OIDC and SAML, an async ORM and
background jobs, without hyper, Tower or Axum underneath. One short
`routes!` sample, link to Building apps.

## 9. FAQ

1. **How is rws different from nginx or Caddy?** One binary covers static
   files, certificates, the reverse proxy and a Kubernetes gateway, configured
   in TOML. When config runs out, the same engine is a Rust library, so you
   don't switch tools.
2. **Is it ready for production?** It runs the HTTP/2 conformance suite, the
   Gateway API suite and ACME against Pebble on every change, and fuzzes its
   parsers. It is a younger project than nginx; read the changelog and pin a
   version.
3. **Does it run on Windows?** Yes. Prebuilt binaries for Windows, Linux
   (static, any distribution) and macOS, and it can run as a Windows service.
4. **What does it cost?** Nothing. rws is MIT licensed.
5. **How do I upgrade?** Replace the binary and restart, or reload. Breaking
   changes are listed in the changelog with a migration guide per major
   version.
6. **Does it support HTTP/3?** Yes, over QUIC on the same port as HTTPS,
   alongside HTTP/2 and HTTP/1.1.
7. **Can I run it on Kubernetes?** Yes, as a plain Deployment or as a Gateway
   API controller with the Helm chart.
8. **Can my AI coding agent use these docs?** Yes. `llms.txt` and
   `llms-full.txt` cover every page, and an agent skill pack lives at
   `/agents/`.
9. **What if I outgrow the config file?** Use the `rust-web-server` crate:
   the config file's features are library types you can compose in code.
10. **How do I remove it?** Delete the binary and the config file. rws keeps
    nothing else, except certificates under `.rws/acme/` if you used ACME.

## 10. Final CTA

**Run it in one minute.** `Install rws` (same panel as the hero). Under it:
MIT licensed. One binary. Remove it by deleting one file.

## 11. SEO / AEO

**Index:** yes (evergreen product page).

- **Title:** `rws: web server, reverse proxy and automatic HTTPS in one binary`
- **Meta description:** `rws serves static files, gets Let's Encrypt
  certificates and load balances your apps from one TOML file. HTTP/3, auth,
  rate limits and a Gateway API controller. MIT licensed.`
- FAQ as `FAQPage` schema; `SoftwareApplication` schema with the version and
  license.
- `og:image`: 1200 × 630, the mark and the headline on cream (drawn in this phase).

## 12. Layout

**A, classic hero plus sections.** The product is understood from one terminal
moment and one config file; visitors are technical and arrive from search,
GitHub and crates.io knowing roughly what a web server is, so the page shows
rather than argues. The comparison story lives on `/why/` (layout D) and
`/benchmarks/`, not here.

## Decisions (2026-10-29)

- Headline **C**: "The web server that gets its own / certificates and routes your apps"
- Competitor names stay in the performance strip
- The rest is approved as written
- The script tab shipped with `install.sh` (2026-10-29); it is the default on
  Linux, Homebrew on macOS, Cargo on Windows
