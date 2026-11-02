#!/usr/bin/env python3
"""Workspace crates to publish, dependencies first: one `name version` line
each (release.yml `crates` job). Skips `publish = false` members; the facade
`rust-web-server` comes last because it depends on the rest."""
import json
import subprocess
import sys

meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--no-deps"] + sys.argv[1:]))
members = {p["name"]: p for p in meta["packages"] if p["id"] in meta["workspace_members"] and p.get("publish") != []}
done, order = set(), []


def visit(name, path=()):
    if name in done:
        return
    if name in path:
        sys.exit(f"dependency cycle: {' -> '.join(path + (name,))}")
    for dep in members[name]["dependencies"]:
        if dep["name"] in members and dep["kind"] in (None, "build"):
            visit(dep["name"], path + (name,))
    done.add(name)
    order.append(name)


for name in sorted(members):
    visit(name)
for name in order:
    print(name, members[name]["version"])
