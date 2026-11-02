#!/usr/bin/env python3
"""Merge cargo public-api listings of the facade and its workspace crates
into the listing users see, in a normal form (ci/public_api.sh,
spec/WORKSPACE_SPLIT.md).

    public_api_merge.py <dir with facade.txt and <crate>.txt> <crate_ident>=<facade path>…

A crate's paths are rewritten to the facade's, and a module the facade
re-exports (one `pub use` line there) is replaced by the crate's listing.

Normal form: rustdoc inlines a `pub use` of an item from the same crate (the
prelude's `Request`: its declaration, fields and some impls repeated under
the new path) but not from another crate (one `pub use` line). So every
re-export the facade's source declares (`pub use crate::…;`) is written as
one `pub use <path>` line, the lines rustdoc inlined under it are dropped,
and repeated lines count once. The baseline is kept in this form, so moving
an item between crates leaves the listing unchanged.
"""
import collections
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def facade(path):
    return "rust_web_server" + ("::" + path if path else "")


def source_reexports(maps):
    """{re-export path: canonical path} from `pub use crate::…;` in src/ and
    in each mapped crate's crates/<crate>/src/, as facade paths."""
    found = {}
    roots = [(os.path.join(ROOT, "src"), "rust_web_server")]
    roots += [(os.path.join(ROOT, "crates", krate.replace("_", "-"), "src"), facade(path)) for krate, path in maps]
    for src, prefix in roots:
        found.update(reexports_in(src, prefix))
    return found


def reexports_in(src, prefix):
    found = {}
    for root, _, names in os.walk(src):
        for name in names:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(root, name)
            rel = os.path.relpath(path, src)[:-3].split(os.sep)
            if rel[-1] in ("mod", "lib"):
                rel = rel[:-1]
            module = "::".join([prefix] + rel)
            text = open(path, encoding="utf-8").read()
            for m in re.finditer(r"^\s*pub use crate::([^;]+);", text, re.M):
                spec = m.group(1).replace("\n", " ")
                if "{" in spec:
                    head, body = spec.split("{", 1)
                    items = [head + x.strip() for x in body.rstrip("} ").split(",") if x.strip()]
                else:
                    items = [spec.strip()]
                for item in items:
                    target, _, alias = item.partition(" as ")
                    target = target.strip()
                    name = (alias or target.rsplit("::", 1)[-1]).strip()
                    found[f"{module}::{name}"] = prefix + "::" + target
    return found


def main(parts, maps):
    maps = [m.split("=", 1) for m in maps]
    lines = set()
    for name in sorted(os.listdir(parts)):
        for line in open(os.path.join(parts, name)).read().splitlines():
            for krate, path in maps:
                if line == f"pub mod {krate}":
                    line = f"pub mod {facade(path)}"
                line = line.replace(f"{krate}::", facade(path) + "::")
            lines.add(line)
    # A re-exported module is a `pub use` in the facade's listing; the crate's
    # own (rewritten) `pub mod` line stands for it.
    mods = {l[len("pub mod "):] for l in lines if l.startswith("pub mod ")}
    lines = {l for l in lines if not (l.startswith("pub use ") and l[len("pub use "):] in mods)}
    # Item re-exports as one `pub use` each.
    reexports = {path for path, target in source_reexports(maps).items() if path not in mods and target not in mods}
    out = set()
    for line in lines:
        words = line.split(" ")
        named = next((w for w in words[1:3] if w.startswith("rust_web_server::")), None) if words[0] == "pub" else None
        hit = None
        if named:
            for r in reexports:
                if named == r or named.startswith((r + "::", r + "<", r + "(", r + ":")):
                    hit = r
                    break
        out.add(f"pub use {hit}" if hit else line)
    for line in sorted(out):
        print(line)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
