#!/usr/bin/env python3
"""Documentation checks, run in CI:

1. Every root-relative link in a docs-site page (`/section/page/`, with an
   optional `#anchor`) points to an existing page and heading.
2. Every relative link in the repository's Markdown files points to an
   existing file.
3. Every `RWS_CONFIG_*` setting the binary accepts is documented on
   configuration/env-vars.md.

    python3 ci/check_docs.py
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = os.path.join(ROOT, "docs", "src", "content", "docs")
# `upstream`: files copied into other repositories, whose links resolve there.
SKIP_DIRS = {"target", "node_modules", ".git", "archive", "dist", ".astro", "upstream"}

errors = []


def md_files(base):
    for d, dirs, files in os.walk(base):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
        for name in files:
            if name.endswith((".md", ".mdx")):
                yield os.path.join(d, name)


def strip_code(text):
    return re.sub(r"`[^`\n]*`", "", re.sub(r"```.*?```", "", text, flags=re.S))


def slug(heading):
    # github-slugger, as used by Astro: lower-case, drop punctuation, spaces -> '-'.
    h = re.sub(r"[`*]|\[([^\]]*)\]\([^)]*\)", r"\1", heading.strip()).lower()
    h = re.sub(r"[^\w\- ]", "", h)
    return h.replace(" ", "-")


def page_for(link):
    rel = link.strip("/") or "index"
    for ext in (".md", ".mdx"):
        if os.path.exists(os.path.join(DOCS, rel + ext)):
            return os.path.join(DOCS, rel + ext)
    return None


def anchors(path):
    with open(path) as f:
        text = re.sub(r"```.*?```", "", f.read(), flags=re.S)
    return {slug(m.group(1)) for m in re.finditer(r"^#{1,6}\s+(.+?)\s*$", text, re.M)}


LINK = re.compile(r"\]\(([^)\s]+)\)")


def check_site_links():
    for path in md_files(DOCS):
        with open(path) as f:
            text = strip_code(f.read())
        for m in LINK.finditer(text):
            target = m.group(1)
            if not target.startswith("/") or target.startswith("//"):
                continue
            link, _, anchor = target.partition("#")
            page = page_for(link)
            where = os.path.relpath(path, ROOT)
            if page is None:
                errors.append(f"{where}: link to missing page {target}")
            elif anchor and anchor not in anchors(page):
                errors.append(f"{where}: link to missing heading {target}")


def check_repo_links():
    for path in md_files(ROOT):
        if path.startswith(DOCS):
            continue
        with open(path) as f:
            text = strip_code(f.read())
        for m in LINK.finditer(text):
            target = m.group(1).split("#")[0]
            if not target or re.match(r"[a-z]+:", target) or target.startswith("/"):
                continue
            if not os.path.exists(os.path.normpath(os.path.join(os.path.dirname(path), target))):
                errors.append(f"{os.path.relpath(path, ROOT)}: link to missing file {target}")


def check_settings_documented():
    src = open(os.path.join(ROOT, "src", "entry_point", "config_file", "mod.rs")).read()
    block = re.search(r"SETTINGS_WITHOUT_FLAG[^=]*=\s*&\[(.*?)\];", src, re.S).group(1)
    settings = set(re.findall(r'"(RWS_CONFIG_[A-Z0-9_]+)"', block))
    entry = open(os.path.join(ROOT, "src", "entry_point", "mod.rs")).read()
    settings |= {
        s for s in re.findall(r'"(RWS_CONFIG_[A-Z0-9_]+)"', entry) if not s.endswith("_DEFAULT_VALUE")
    }
    with open(os.path.join(DOCS, "configuration", "env-vars.md")) as f:
        documented = set(re.findall(r"RWS_CONFIG_[A-Z0-9_]+", f.read()))
    for name in sorted(settings - documented):
        errors.append(f"configuration/env-vars.md: {name} is not documented")


def check_systemd_unit():
    """deployment/systemd.md shows the unit the deb/rpm packages install."""
    unit = open(os.path.join(ROOT, "packaging", "linux", "rws.service")).read()
    page = open(os.path.join(DOCS, "deployment", "systemd.md")).read()
    block = re.search(r"```ini\n(.*?)```", page, re.S)
    if not block or block.group(1) != unit:
        errors.append("deployment/systemd.md: the ini block must equal packaging/linux/rws.service")


check_site_links()
check_repo_links()
check_settings_documented()
check_systemd_unit()
for e in errors:
    print(e)
sys.exit(1 if errors else 0)
