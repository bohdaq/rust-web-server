---
title: Static Files, Directory Listing & SPA Fallback
description: How rust-web-server serves files from disk, the default directory listing page for directories without an index.html, and the opt-in SPA fallback for client-side routers.
---

## Serving files

Any file under the process's working directory is served automatically — no configuration, no routes to register. `StaticResourceController` handles range requests, sets `ETag` / `Last-Modified` headers, negotiates gzip, and streams files larger than 256 KiB without loading them fully into memory.

### Browser caching

Static files are sent with `Cache-Control: no-cache` (since 18.0.0; it was
`no-store`): the browser keeps the file and asks again on the next use with
`If-None-Match` (the `ETag`) or `If-Modified-Since` (the `Last-Modified`
date), and gets a `304 Not Modified` without a body while the file is
unchanged. `If-None-Match` decides when both are sent (RFC 9110 §13.1.3).
Set `RWS_CONFIG_STATIC_CACHE_CONTROL` to change it — e.g.
`public, max-age=31536000, immutable` for files with a content hash in their
name — or to an empty value for the old `no-store` behavior. Responses from
your handlers keep `no-store, …` unless they set their own `Cache-Control`.

Files larger than 256 KiB are streamed from disk with an exact
`Content-Length` instead of being read into memory, so serving a 1 GB file
uses a few MiB. The static root is the working directory `rws` was started
in.

The request path is percent-decoded once before the lookup, so
`/my%20file.txt` serves `my file.txt` and `%2F` is a `/` (17.113.2); `+` is a
literal `+`. A path that isn't UTF-8 once decoded gets `404`.

Only files inside the root are served — checked on the decoded path, so
`/%2e%2e/` is refused like `/../`: a path with a `..` segment is refused
(`404`), and so is a symlink in the root that points outside it. Files and
directories whose name starts with `.` (`.git/`, `.env`) are not served unless
`RWS_CONFIG_SERVE_DOTFILES=true`; `.well-known/` always is (17.113.1).
Private-key and config files — `*.pem`, `*.key`, `rws.config.toml` — are
never served.

Range requests follow RFC 9110 over HTTP/1.1, HTTP/2, and HTTP/3: `bytes=2-5`,
open-ended `bytes=7-`, suffix `bytes=-500`, and multiple ranges get
`206 Partial Content` with an exact `Content-Range` (`bytes 7-9/10`); a range
ending past the file is clamped to its last byte; a range starting past the
end can't be satisfied: it is skipped, and only a request none of whose ranges can be satisfied gets `416`, with `Content-Range: bytes */<length>` (17.113.2). Full `200` responses carry no `Content-Range`. Overlapping
ranges are merged (and then sorted); more than 16 ranges are answered with
the whole file as one range, so a response never holds more than the file
(17.113.1).

Conditional requests: every file response carries an `ETag`. `If-None-Match`
takes a list of tags or `*` and compares weakly (`W/"x"` matches `"x"`), as
RFC 9110 §13.1.2 requires; a match is a `304 Not Modified` (17.113.2).
A `Range` with `If-Range` is honored only while the `If-Range` tag strongly
matches the current `ETag`, or is a date equal to the file's `Last-Modified`
(at least a second old); otherwise (another tag, a weak one, another date)
the whole file is sent with `200`, so a client never splices a piece of a
newer file onto an older download.

```bash
mkdir www && echo "<h1>Hello</h1>" > www/index.html
cd www
rws
curl http://localhost:7878/          # serves index.html
curl http://localhost:7878/index.html # same file, explicit path
```

## How files are served fast

- **Open files are reused.** A static file's open handle is kept for one
  second (`open_file_cache_secs` / `RWS_CONFIG_OPEN_FILE_CACHE_SECS`, read
  at startup; `0` turns it off; Unix only) and read with positional
  reads, so a request for a hot file costs one `fstat` instead of an `open`,
  several `stat`s and a `close`. Size and modification time are read on every
  request, so a file edited in place is served correctly at once; a file
  replaced by a new one (a rename) or deleted is noticed within the second.
  At most 128 files are held open. Files up to 64 KiB also have their
  contents kept in memory (8 MiB in total), checked against the size and
  modification time on every request, so an unchanged small file costs no
  read; a file modified in the last two seconds is always read fresh.
- **One write per response.** A small response's head and body leave in one
  system call and one TCP segment.
- **Downloads use `sendfile`** over plain HTTP on Linux and macOS: the kernel
  copies the file to the socket, with the response head in the same segment
  (`TCP_CORK` on Linux, `sendfile`'s header vector on macOS) and the open
  file handle reused rather than reopened.
- **Static files and health checks run on the event loop**, without a hand-off
  to a thread pool.

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

If you set a CSP (`RWS_CONFIG_CSP`, or the default one via `RWS_CONFIG_SECURITY_HEADERS`) and excludes `'self'` from `style-src`/`script-src`, add an allowance for it or the listing will render unstyled the same way any other same-origin asset would under that policy.

## SPA fallback

For local development, before you have a built app to serve this way, see [Frontend Dev Server Proxy](/getting-started/frontend-dev-proxy/) for running a React/Vite dev server alongside `rws`.

**Opt-in — unset (disabled) by default.** `RWS_CONFIG_SPA_FALLBACK` serves a configured file for any `GET`/`HEAD` request that matches no real file, directory, or `path.html`, instead of `404` — the standard client-side-router ("SPA") fallback a React Router / Vue Router / etc. app needs for deep links to work:

```bash
# Build your React/Vue/etc. app, then serve its output directly:
cd dist && RWS_CONFIG_SPA_FALLBACK=index.html rws
```

```bash
curl http://localhost:7878/dashboard/settings
# no file on disk at this path — served index.html instead of 404,
# so the client-side router can take over and render /dashboard/settings
```

```rust
use rust_web_server::app::App;
use rust_web_server::test_client::TestClient;

let client = TestClient::new(App::new());
let response = client.get("/dashboard/settings").send();
assert_eq!(200, response.status());
```

A file or directory that *does* exist is always served as itself — the fallback only ever applies to what was previously a `404`, and only when the configured file actually exists on disk (a typo'd `RWS_CONFIG_SPA_FALLBACK` value is a silent no-op, not a broken response).

### Scoping — avoiding swallowed 404s

A blanket fallback would turn every typo'd URL and missing asset into a `200`, which is worse than the `404` it replaces. Two checks apply automatically:

1. **Extension heuristic** — only a request whose *last path segment* has no `.` gets the fallback. `/dashboard/settings` qualifies (a client-side route); `/assets/logo.png` (a missed static asset) still `404`s. This is the same heuristic webpack-dev-server's `historyApiFallback`, `sirv`, and `vite preview` use.
2. **`RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`** — a comma-separated list of path prefixes that never receive the fallback, even when it's configured:

```bash
export RWS_CONFIG_SPA_FALLBACK=index.html
export RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES=/api,/healthz
```

```bash
curl http://localhost:7878/api/users/999   # 404 — excluded prefix, not rewritten
curl http://localhost:7878/settings/theme  # 200 — served index.html
```

Both env vars are read fresh on every request, so they can be changed via `rws.config.toml` + `SIGHUP` without a restart — see [Hot Config Reload](/features/hot-reload/).
