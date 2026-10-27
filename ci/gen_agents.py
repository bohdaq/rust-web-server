#!/usr/bin/env python3
"""Generate the Cursor rule and Copilot instructions from the agent skill
(agents/skills/rws/SKILL.md), so the three never drift.

    python3 ci/gen_agents.py           # write them
    python3 ci/gen_agents.py --check   # exit 1 if either is out of date
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKILL = os.path.join(ROOT, "agents", "skills", "rws", "SKILL.md")
NOTICE = "<!-- Generated from agents/skills/rws/SKILL.md by ci/gen_agents.py: edit that file. -->\n"


def split(text):
    """(frontmatter dict, body) of a SKILL.md."""
    if not text.startswith("---\n"):
        raise SystemExit("SKILL.md must start with YAML frontmatter")
    head, body = text[4:].split("\n---\n", 1)
    meta = {}
    for line in head.splitlines():
        key, _, value = line.partition(":")
        meta[key.strip()] = value.strip()
    return meta, body.lstrip("\n")


def build():
    meta, body = split(open(SKILL, encoding="utf-8").read())
    description = meta["description"].replace('"', "'")
    cursor = (
        "---\n"
        f'description: "{description}"\n'
        'globs: ["**/*.rs", "**/Cargo.toml", "**/rws.config.toml"]\n'
        "alwaysApply: false\n"
        "---\n" + NOTICE + "\n" + body
    )
    copilot = NOTICE + "\n" + body
    return {
        os.path.join(ROOT, "agents", "cursor", "rws.mdc"): cursor,
        os.path.join(ROOT, "agents", "copilot-instructions.md"): copilot,
    }


def main():
    check = "--check" in sys.argv[1:]
    stale = []
    for path, content in build().items():
        current = open(path, encoding="utf-8").read() if os.path.exists(path) else None
        if current == content:
            continue
        if check:
            stale.append(os.path.relpath(path, ROOT))
        else:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(content)
    if stale:
        print("out of date (run python3 ci/gen_agents.py): " + ", ".join(stale))
        sys.exit(1)


if __name__ == "__main__":
    main()
