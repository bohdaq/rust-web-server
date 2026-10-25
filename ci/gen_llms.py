#!/usr/bin/env python3
"""Generate llms.txt (an index of the docs site) and llms-full.txt (every
page in one file) from docs/src/content/docs, in sidebar order.

    python3 ci/gen_llms.py           # write both files
    python3 ci/gen_llms.py --check   # exit 1 if either is out of date

Edit the docs pages, not the generated files.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = os.path.join(ROOT, "docs", "src", "content", "docs")
SITE = "https://rws8.tech"

HEADER = """# rws — Rust Web Server

> A single-binary web server and reverse proxy — automatic HTTPS (ACME / Let's Encrypt),
> HTTP/3, load balancing, auth, rate limiting, configured in one `rws.config.toml` — and
> the same engine as a Rust library (routing, middleware, MCP server, async ORM).

Source: https://github.com/bohdaq/rust-web-server · Website: https://rws8.tech/ ·
API: https://docs.rs/rust-web-server · License: MIT · MSRV: 1.88 · Version: {version}

HTTP/1.1 parsing, routing, middleware, WebSocket, SSE, CORS and JSON are implemented in
the crate — no hyper, Tower or Axum; HTTP/2 and HTTP/3 use `h2` and `quinn`/`h3`, TLS uses
`rustls`. Every handler implements one trait,
`Application::execute(&Request, &ConnectionInfo) -> Result<Response, String>`, and every
cross-cutting concern (auth, rate limits, caching, tracing, the reverse proxy) is a
`Middleware` added with `app.wrap(layer)`.

Use it as:
- **a static file server** — `brew install bohdaq/tap/rws && rws`, `cargo binstall rust-web-server`, or `docker run -p 7878:7878 bohdaq/rws`
- **a config-driven proxy** — `[[route]]` / `[[upstream]]` in `rws.config.toml`, no code
- **automatic HTTPS** — an `[acme]` section with `domains = [...]`
- **a library** — `cargo add rust-web-server`
"""

FULL_NOTE = """
This file is every page of the documentation site in one document, generated from
`docs/src/content/docs/` by `ci/gen_llms.py`. The index with one line per page is
`llms.txt`.
"""


def version():
    with open(os.path.join(ROOT, "Cargo.toml")) as f:
        return re.search(r'^version\s*=\s*"([^"]+)"', f.read(), re.M).group(1)


def sidebar():
    """[(group label, [(link, label)])] in astro.config.mjs order."""
    with open(os.path.join(ROOT, "docs", "astro.config.mjs")) as f:
        text = f.read()
    groups, current = [], None
    for m in re.finditer(r"label:\s*'([^']+)'(?:\s*,\s*link:\s*'([^']+)')?", text):
        label, link = m.group(1), m.group(2)
        if link is None:
            current = (label, [])
            groups.append(current)
        elif current is not None:
            current[1].append((link, label))
    return groups


def page_file(link):
    rel = link.strip("/") or "index"
    for ext in (".md", ".mdx"):
        path = os.path.join(DOCS, rel + ext)
        if os.path.exists(path):
            return path
    return None


def parse(path):
    with open(path) as f:
        text = f.read()
    meta, body = {}, text
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if m:
        body = text[m.end():]
        for line in m.group(1).splitlines():
            k, _, v = line.partition(":")
            if v and not line.startswith(" "):
                meta[k.strip()] = v.strip().strip("'\"")
    # MDX: drop imports and component tags, keep their inner text.
    body = re.sub(r"^import .*?;\s*$\n?", "", body, flags=re.M)
    body = re.sub(r"</?[A-Z][A-Za-z]*[^>]*>\n?", "", body)
    return meta, body.strip() + "\n"


def build():
    groups = sidebar()
    seen = set()
    index = [HEADER.format(version=version())]
    full = [HEADER.format(version=version()), FULL_NOTE]
    for label, links in groups:
        entries = []
        for link, title in links:
            path = page_file(link)
            if path is None or path in seen:
                continue
            seen.add(path)
            meta, body = parse(path)
            title = meta.get("title", title)
            desc = meta.get("description", "")
            url = SITE + link
            entries.append(f"- [{title}]({url})" + (f": {desc}" if desc else ""))
            full.append(f"\n---\n\n# {title}\n\n<{url}>\n\n{body}")
        if entries:
            index.append(f"\n## {label}\n\n" + "\n".join(entries) + "\n")
    orphans = []
    for d, _, files in os.walk(DOCS):
        for name in sorted(files):
            path = os.path.join(d, name)
            if name.endswith((".md", ".mdx")) and path not in seen and name != "index.mdx":
                orphans.append(os.path.relpath(path, DOCS))
    index.append(
        "\n## Optional\n\n"
        "- [llms-full.txt](https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms-full.txt): every page above in one file\n"
        "- [CHANGELOG.md](https://github.com/bohdaq/rust-web-server/blob/main/CHANGELOG.md): changes per release\n"
        "- [CLAUDE.md](https://github.com/bohdaq/rust-web-server/blob/main/CLAUDE.md): architecture and conventions for working on the crate itself\n"
    )
    return "".join(index), "".join(full), orphans


def main():
    llms, full, orphans = build()
    targets = {os.path.join(ROOT, "llms.txt"): llms, os.path.join(ROOT, "llms-full.txt"): full}
    if orphans:
        print("pages missing from the sidebar (docs/astro.config.mjs):", *orphans, sep="\n  ")
    if "--check" in sys.argv:
        stale = []
        for path, content in targets.items():
            try:
                with open(path) as f:
                    if f.read() != content:
                        stale.append(path)
            except FileNotFoundError:
                stale.append(path)
        if stale or orphans:
            for path in stale:
                print(f"{os.path.relpath(path, ROOT)} is out of date — run python3 ci/gen_llms.py")
            sys.exit(1)
        return
    for path, content in targets.items():
        with open(path, "w") as f:
            f.write(content)


if __name__ == "__main__":
    main()
