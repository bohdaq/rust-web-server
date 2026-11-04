#!/usr/bin/env python3
"""S12 docs coverage: does the rws binary accept what the docs show?

Every ```toml block on the docs site that looks like an rws config file is
written to its own file and given to `rws --check`. A block passes when the
binary reports no unknown key (a documented key rws doesn't read is a gap)
and no error other than one about the environment (files, env vars and
hosts a docs example names but this machine doesn't have), or one that
says the block is a fragment of a bigger config (an upstream declared in an
earlier block). A second pass looks for each `RWS_CONFIG_*` named on
configuration/env-vars.md in the sources: a documented setting nothing
reads is a gap (ci/check_docs.py checks the other direction).

    ci/stage/docs_coverage.py <rws command…> --out <dir>
    ci/stage/docs_coverage.py docker run --rm -v {dir}:/c bohdaq/rws:18.27.2 --out report/x

`{dir}` in the command is replaced by the work directory (mounted, for a
container); config paths given to the command are relative to it. Prints
one line per block and a summary; writes coverage.tsv to --out. Exit 1 when
a block has an unknown key.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "src", "content", "docs")

# Errors that come from the docs example's environment, not from rws: a file
# it names isn't here, an env var it names isn't set, a host isn't resolvable.
ENVIRONMENT = re.compile(
    r"No such file|not a directory|is not set|isn't set|unset|not found|"
    r"failed to read|can't read|cannot read|resolve|lookup|ENOENT|"
    r"plugin|htpasswd|certificate|key file|cert file|private key|"
    r"does not exist|doesn't exist|empty",
    re.I,
)
FRAGMENT = re.compile(r"is not a declared \[\[upstream\]\]|not declared \[\[upstream\]\]|no `type`|needs dns_api|must hold at least", re.I)
UNKNOWN = re.compile(r"unknown (key|setting|section|table)|not a known|unrecognized", re.I)
# A config-file block: top-level rws tables or settings, not Cargo.toml,
# Helm values or a WASM plugin manifest.
RWS_TABLES = re.compile(
    r"^\s*\[\[?(route|upstream|tcp_proxy|udp_proxy|ws_proxy|virtual_host|acme|cors|rate_limit|admin|traffic|"
    r"cluster|fleet|gateway_api|circuit_breaker|authz|middleware|waf|session|log)\b", re.M)
NOT_RWS = re.compile(r"^\s*\[(package|dependencies|features|workspace|lib|bin|profile)", re.M)


def blocks():
    for d, _, files in os.walk(DOCS):
        for name in sorted(files):
            if not name.endswith((".md", ".mdx")):
                continue
            path = os.path.join(d, name)
            text = open(path, encoding="utf-8").read()
            for n, m in enumerate(re.finditer(r"```toml\n(.*?)```", text, re.S)):
                body = m.group(1)
                if NOT_RWS.search(body) or not RWS_TABLES.search(body):
                    continue
                line = text[: m.start()].count("\n") + 1
                yield os.path.relpath(path, DOCS), line, body


def main():
    args = sys.argv[1:]
    out = args[args.index("--out") + 1]
    cmd = args[: args.index("--out")]
    work = os.path.join(os.path.abspath(out), "docs-blocks")
    os.makedirs(work, exist_ok=True)
    rows, unknown = [], 0
    for i, (page, line, body) in enumerate(blocks()):
        name = f"b{i:03d}.toml"
        with open(os.path.join(work, name), "w") as f:
            f.write(body)
        argv = [a.replace("{dir}", work) for a in cmd]
        target = f"/c/{name}" if any("{dir}" in a for a in cmd) else os.path.join(work, name)
        r = subprocess.run(argv + [f"--check={target}"], capture_output=True, text=True, timeout=60)
        report = [l for l in (r.stdout + r.stderr).splitlines() if re.match(r"\s*(error|warning)", l)]
        unknowns = [l for l in report if UNKNOWN.search(l)]
        others = [l for l in report if l.startswith("error") and not UNKNOWN.search(l) and not ENVIRONMENT.search(l) and not FRAGMENT.search(l)]
        status = "unknown-key" if unknowns else ("error" if others else "ok")
        unknown += bool(unknowns)
        detail = " | ".join(unknowns or others)[:400]
        rows.append((f"{page}:{line}", status, detail))
        print(f"{status:12} {page}:{line} {detail}")
    sources = ""
    for d, dirs, files in os.walk(ROOT):
        dirs[:] = [x for x in dirs if x not in {"target", "node_modules", ".git", "docs", "site", "fuzz"}]
        sources += "".join(open(os.path.join(d, n), encoding="utf-8", errors="replace").read() for n in files if n.endswith(".rs"))
    documented = sorted(set(re.findall(r"RWS_CONFIG_[A-Z0-9_]+[A-Z0-9]", open(os.path.join(DOCS, "configuration", "env-vars.md")).read())))
    # VIRTUAL_HOST_<n>_* names are built with format!.
    unread = [v for v in documented if not re.match(r"RWS_CONFIG_VIRTUAL_HOST_[0-9]", v) and f'"{v}' not in sources and not re.search(r"\b" + v + r"\b", sources)]
    for v in unread:
        rows.append((f"env-vars.md {v}", "unread", "documented, no source reads it"))
        print(f"{'unread':12} env-vars.md {v}")
    print(f"env-vars.md: {len(documented)} RWS_CONFIG_* settings, {len(unread)} that no source names")
    with open(os.path.join(out, "coverage.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    blocks_ = [r for r in rows if r[1] != "unread"]
    ok = sum(r[1] == "ok" for r in blocks_)
    print(f"docs toml blocks: {len(blocks_)} checked, {ok} ok, {unknown} with unknown keys, "
          f"{sum(r[1] == 'error' for r in rows)} with other errors")
    return 1 if unknown or unread else 0


if __name__ == "__main__":
    sys.exit(main())
