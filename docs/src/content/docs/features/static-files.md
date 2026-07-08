---
title: Static Files & Directory Listing
description: How rust-web-server serves files from disk, and the default directory listing page for directories without an index.html.
---

## Serving files

Any file under the process's working directory is served automatically — no configuration, no routes to register. `StaticResourceController` handles range requests, sets `ETag` / `Last-Modified` headers, negotiates gzip, and streams files larger than 8 MB without loading them fully into memory.

```bash
mkdir www && echo "<h1>Hello</h1>" > www/index.html
cd www
rws
curl http://localhost:7878/          # serves index.html
curl http://localhost:7878/index.html # same file, explicit path
```

## Directory listing

**Default, always-on behavior — not gated by a feature flag or config setting.** A requested directory that has no `index.html` renders a directory listing page (`200 OK`) instead of falling through to `404 Not Found`. A directory that *does* have an `index.html` is served exactly as before — this only changes what happens for the previously-404 case.

```bash
mkdir -p www/reports && touch www/reports/q1.pdf www/reports/q2.pdf
curl http://localhost:7878/reports/
# <!DOCTYPE html> ... directory listing page ...
```

The page includes:

- A breadcrumb (`~ / reports`) linking back to each ancestor directory.
- A parent-directory link (omitted at a static root, since there's nowhere to go up to).
- A table of entries — directories first, then files, both sorted case-insensitively by name — with size and last-modified columns.
- A client-side search box that filters rows without a round-trip to the server.

Entry names are HTML-escaped for display and percent-encoded in `href` attributes, so filenames containing `<`, `&`, spaces, or other special characters render and link correctly. Dotfiles (names starting with `.`) are omitted from the listing.

```rust
use rust_web_server::app::App;
use rust_web_server::test_client::TestClient;

let client = TestClient::new(App::new());
let response = client.get("/reports/").send();
assert_eq!(200, response.status());
```

## Why the listing's CSS/JS aren't inlined

The framework's default `Content-Security-Policy: default-src 'self'` header (see [CORS & Security](/features/cors-security/)) silently blocks inline `<style>`/`<script>` blocks — a browser enforcing that policy would render the listing completely unstyled if the CSS were embedded directly in the page.

Instead, `DirectoryListingAssetsController` serves the listing's stylesheet and filter-box script as same-origin assets:

- `GET /rws-directory-listing.css`
- `GET /rws-directory-listing.js`

Both are `'self'`-origin `<link>`/`<script src>` references, fully compliant with the default CSP — no policy relaxation needed. Just like `StyleController` (`/style.css`) and `ScriptController` (`/script.js`), a file on disk at that same relative path overrides the compiled-in default:

```bash
# Restyle the directory listing without recompiling — drop a file named
# exactly "rws-directory-listing.css" in the working directory.
echo "body { font-family: monospace; }" > rws-directory-listing.css
```

If your own CSP is stricter than the default (via `RWS_CONFIG_CSP`) and excludes `'self'` from `style-src`/`script-src`, add an allowance for it or the listing will render unstyled the same way any other same-origin asset would under that policy.
