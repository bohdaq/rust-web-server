#!/usr/bin/env python3
"""The crate's module graph, for the workspace split (spec/WORKSPACE_SPLIT.md).

    ci/module_graph.py                  strongly connected components (cycles)
    ci/module_graph.py --up a,b,c       references from modules a,b,c to any
                                        module outside the set (what keeps
                                        them from being their own crate)

A module is a directory or file directly under src/. Edges are `crate::<m>`
references in non-test code (`use crate::{a, b::C}` included); doc links
count too, since a moved module's doc links must still resolve.
"""
import collections
import os
import re
import sys

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src")


def modules():
    found = set()
    for name in os.listdir(SRC):
        path = os.path.join(SRC, name)
        if os.path.isdir(path):
            found.add(name)
        elif name.endswith(".rs") and name not in ("lib.rs", "main.rs"):
            found.add(name[:-3])
    return found


def files(module):
    path = os.path.join(SRC, module)
    if os.path.isdir(path):
        for root, _, names in os.walk(path):
            for name in names:
                if name.endswith(".rs") and "test" not in name and name != "reference_parser.rs":
                    yield os.path.join(root, name)
    else:
        yield path + ".rs"


def non_test(text):
    cut = text.find("#[cfg(test)]\nmod tests")
    return text if cut < 0 else text[:cut]


def references(text):
    """(module, line number) for each `crate::<module>` in text."""
    for m in re.finditer(r"crate::([a-z_0-9]+)", text):
        yield m.group(1), text.count("\n", 0, m.start()) + 1
    for m in re.finditer(r"crate::\{([^}]*)\}", text, re.S):
        line = text.count("\n", 0, m.start()) + 1
        for name in re.findall(r"(?:^|[,{\s])([a-z_0-9]+)(?=::|\s*[,}]|\s*$)", m.group(1)):
            yield name, line


def graph(mods):
    edges = collections.defaultdict(lambda: collections.defaultdict(list))
    for module in mods:
        for path in files(module):
            with open(path, encoding="utf-8") as f:
                text = non_test(f.read())
            for target, line in references(text):
                if target != module and target in mods:
                    edges[module][target].append(f"{os.path.relpath(path, os.path.join(SRC, '..'))}:{line}")
    return edges


def sccs(mods, edges):
    index, low, stack, on, out, counter = {}, {}, [], set(), [], [0]

    def visit(v):
        # Iterative Tarjan: no recursion limit on deep graphs.
        work = [(v, iter(sorted(edges[v])))]
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on.add(v)
        while work:
            node, it = work[-1]
            advanced = False
            for w in it:
                if w not in index:
                    index[w] = low[w] = counter[0]
                    counter[0] += 1
                    stack.append(w)
                    on.add(w)
                    work.append((w, iter(sorted(edges[w]))))
                    advanced = True
                    break
                if w in on:
                    low[node] = min(low[node], index[w])
            if advanced:
                continue
            work.pop()
            if work:
                low[work[-1][0]] = min(low[work[-1][0]], low[node])
            if low[node] == index[node]:
                comp = []
                while True:
                    w = stack.pop()
                    on.discard(w)
                    comp.append(w)
                    if w == node:
                        break
                out.append(sorted(comp))

    for m in sorted(mods):
        if m not in index:
            visit(m)
    return out


def main(argv):
    mods = modules()
    edges = graph(mods)
    if len(argv) > 2 and argv[1] == "--up":
        group = set(argv[2].split(","))
        unknown = group - mods
        if unknown:
            sys.exit(f"not modules: {', '.join(sorted(unknown))}")
        leaks = 0
        for module in sorted(group):
            for target, places in sorted(edges[module].items()):
                if target not in group:
                    leaks += 1
                    print(f"{module} -> {target}: {', '.join(places[:4])}{' …' if len(places) > 4 else ''}")
        print(f"{leaks} upward edge(s)")
        return 1 if leaks else 0
    for comp in sorted(sccs(mods, edges), key=len, reverse=True):
        if len(comp) > 1:
            print(f"cycle of {len(comp)}: {' '.join(comp)}")
    print(f"{len(mods)} modules")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
