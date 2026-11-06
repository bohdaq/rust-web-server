# Answer-first openings (draft for review)

AI answers and search snippets quote a page's first plain sentences. Each
proposal below is the first paragraph of the page: what it is, who it's for,
the one fact people ask. Keep the current text after it.

## rws8.tech

**Home** (above the hero copy, or as the hero lede):
> rws is a free, open-source (MIT) web server and reverse proxy in one
> binary: it serves static files, gets and renews Let's Encrypt certificates
> on its own, load balances your apps and adds auth and rate limits — all from
> one `rws.config.toml`. It runs on Linux, macOS and Windows, speaks HTTP/1.1,
> HTTP/2 and HTTP/3, and is also a Rust crate for building apps.

**Docs › Quick start** (replace "Pick the path that matches your goal."):
> To run rws, install it (`brew install bohdaq/tap/rws`, `cargo binstall
> rust-web-server` or `docker run -p 7878:7878 bohdaq/rws`) and start `rws` in
> a folder: it serves that folder on http://localhost:7878. The four paths
> below add a route, a proxy and automatic HTTPS.

**Docs › Automatic HTTPS (ACME)**:
> rws gets HTTPS certificates from Let's Encrypt (or any ACME CA) by itself:
> list your domains under `[acme]`, open ports 80 and 443, and it orders,
> installs and renews them with no restart. HTTP-01, TLS-ALPN-01 and DNS-01
> challenges are supported, including wildcards and on-demand TLS.

**Docs › Reverse proxy**:
> rws proxies requests to your apps from config alone: a `[[route]]` matches
> host and path, an `[[upstream]]` lists the backends, and rws load balances,
> health checks and retries them — no code.

**FAQ additions (home FAQPage):**
- *Is rws a replacement for nginx?* — For most small and medium sites, yes:
  static files, proxy, HTTPS, auth and rate limits in one file. See rws vs
  nginx vs Caddy.
- *Does rws work in Kubernetes?* — Yes: a Gateway API controller (v1.6.3
  conformance), a Helm chart and a container image.
- *How much does rws cost?* — Nothing: MIT licensed, no paid tier.

## lit.rws8.tech

**Home lede**:
> lit-material is a free (MIT) library of Material 3 web components for
> consoles and admin UIs: buttons and fields, plus data tables, log viewers,
> charts and navigation shells. Each component is its own npm package, works
> in any framework or none, and renders on the server with Lit SSR.

**Components index**:
> lit-material has N components (count at build time), grouped by what
> they're for: actions, inputs, navigation, data, feedback and layout. Every
> page shows a live example, the install line and the full API.

## hitop.rws8.tech

**Home lede** (the comparison guides already answer "HITOP vs X" well):
> HITOP is a free, open-source (MIT) API client — a Postman alternative that
> needs no account and has no CORS errors. Send HTTP requests, keep them in
> collections as plain files in git, script them in JavaScript and run them
> in CI; in the browser, as a CLI, and as native Mac and iPhone apps.

**Guides index**: rename the section from "/terms/" to "Guides" in the
visible heading and breadcrumb (it is already "Guides"); consider moving the
URLs to `/guides/<name>/` with 301s from `/terms/<name>.html` — "terms"
reads as legal terms to people and to search engines.
