#!/usr/bin/env python3
"""Bot, AI-agent and crawler-IP tables for rws's traffic insights.

    ci/gen_bot_lists.py            regenerate crates/rws-proxy/src/traffic/data.rs from crates/rws-proxy/src/traffic/bots.json
    ci/gen_bot_lists.py --check    fail if data.rs isn't what bots.json gives (CI, offline)
    ci/gen_bot_lists.py --update   re-fetch the sources into bots.json, then regenerate

Sources (bots.json keeps their commits / fetch dates):
  - ai.robots.txt robots.json (MIT) — AI agents by User-Agent token, with
    operator and function
  - crawler-user-agents (MIT) — bot User-Agent patterns with tags
  - the crawler IP ranges Google, Bing, Apple, DuckDuckGo, OpenAI,
    Perplexity and Common Crawl publish
"""
import json, re, subprocess, sys, urllib.request, datetime, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INPUT = os.path.join(ROOT, "crates/rws-proxy/src/traffic/bots.json")
OUTPUT = os.path.join(ROOT, "crates/rws-proxy/src/traffic/data.rs")

AI_ROBOTS = ("ai-robots-txt/ai.robots.txt", "main", "robots.json")
CRAWLERS = ("monperrus/crawler-user-agents", "master", "crawler-user-agents.json")
# name, URL, group, kind, the UA token the range verifies
RANGES = [
    ("Googlebot", "https://developers.google.com/static/search/apis/ipranges/googlebot.json", "bot", "search-engine", "googlebot"),
    ("Google special crawlers", "https://developers.google.com/static/search/apis/ipranges/special-crawlers.json", "bot", "search-engine", ""),
    ("Google user-triggered fetchers", "https://developers.google.com/static/search/apis/ipranges/user-triggered-fetchers.json", "bot", "fetcher", ""),
    ("Bingbot", "https://www.bing.com/toolbox/bingbot.json", "bot", "search-engine", "bingbot"),
    ("Applebot", "https://search.developer.apple.com/applebot.json", "bot", "search-engine", "applebot"),
    ("DuckDuckBot", "https://duckduckgo.com/duckduckbot.json", "bot", "search-engine", "duckduckbot"),
    ("GPTBot", "https://openai.com/gptbot.json", "ai", "ai-training", "gptbot"),
    ("OAI-SearchBot", "https://openai.com/searchbot.json", "ai", "ai-search", "oai-searchbot"),
    ("ChatGPT-User", "https://openai.com/chatgpt-user.json", "ai", "ai-assistant", "chatgpt-user"),
    ("PerplexityBot", "https://www.perplexity.ai/perplexitybot.json", "ai", "ai-search", "perplexitybot"),
    ("Perplexity-User", "https://www.perplexity.ai/perplexity-user.json", "ai", "ai-assistant", "perplexity-user"),
    ("CCBot", "https://index.commoncrawl.org/ccbot.json", "ai", "ai-training", "ccbot"),
]

def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "rws gen_bot_lists.py"})
    return urllib.request.urlopen(req, timeout=30).read()

def head(repo, branch):
    out = subprocess.run(["gh", "api", f"repos/{repo}/commits/{branch}", "--jq", ".sha"], capture_output=True, text=True, check=True)
    return out.stdout.strip()

def ai_kind(function):
    f = function.lower()
    if "assistant" in f or "user" in f and "agent" in f:
        return "ai-assistant"
    if "search" in f:
        return "ai-search"
    if "training" in f or "data" in f or "scrap" in f or "crawler" in f:
        return "ai-training"
    return "ai-agent"

LITERAL = re.compile(r"^\^?[A-Za-z0-9 _\-./:;+!@&=~,']+$")

def literals(pattern):
    """A crawler-user-agents regex as `[(prefix_only, lowercase literal)]`:
    plain (escaped) literals, and one `(a|b)` group of literal alternatives
    expanded; [] for anything else."""
    p = re.sub(r"\\([/.\-_ :+()])", r"\1", pattern)
    group = re.fullmatch(r"(\^?)([^()|]*)\(([^()]*)\)([^()|]*)", p)
    variants = [f"{group.group(1)}{group.group(2)}{alt}{group.group(4)}" for alt in group.group(3).split("|")] if group and "|" in group.group(3) else [p]
    out = []
    for v in variants:
        anchored = v.startswith("^")
        if not LITERAL.match(v):
            return []
        v = v[1:] if anchored else v
        if len(v) >= 3:
            out.append((anchored, v.lower()))
    return out

# Common bots neither list names as such (lowercase token, name, kind).
EXTRA_CRAWLERS = [("semrushbot", "SemrushBot", "seo"), ("ahrefsbot", "AhrefsBot", "seo"), ("dataforseobot", "DataForSeoBot", "seo")]
# AI-list entries that are better known as something else.
NOT_AI = {"facebookexternalhit": "social-preview"}

def update():
    data = {"generated": datetime.date.today().isoformat()}
    sha = head(AI_ROBOTS[0], AI_ROBOTS[1])
    robots = json.loads(fetch(f"https://raw.githubusercontent.com/{AI_ROBOTS[0]}/{sha}/{AI_ROBOTS[2]}"))
    data["ai_robots"] = {"source": f"https://github.com/{AI_ROBOTS[0]} @ {sha}", "license": "MIT",
                         "agents": sorted([[name, v.get("operator", ""), v.get("function", "")] for name, v in robots.items()])}
    sha = head(CRAWLERS[0], CRAWLERS[1])
    crawlers = json.loads(fetch(f"https://raw.githubusercontent.com/{CRAWLERS[0]}/{sha}/{CRAWLERS[2]}"))
    data["crawlers"] = {"source": f"https://github.com/{CRAWLERS[0]} @ {sha}", "license": "MIT",
                        "patterns": [[c["pattern"], ",".join(c.get("tags", []))] for c in crawlers]}
    data["ranges"] = []
    for name, url, group, kind, token in RANGES:
        prefixes = [p.get("ipv4Prefix") or p.get("ipv6Prefix") for p in json.loads(fetch(url)).get("prefixes", [])]
        data["ranges"].append({"name": name, "source": url, "group": group, "kind": kind, "verifies": token, "prefixes": sorted(p for p in prefixes if p)})
    with open(INPUT, "w") as f:
        json.dump(data, f, indent=0, sort_keys=True)
        f.write("\n")

def rust_str(s):
    return json.dumps(s, ensure_ascii=False)

CRAWLER_KIND = [("search", "search-engine"), ("seo", "seo"), ("monitor", "monitoring"), ("social", "social-preview"),
                ("feed", "feed"), ("archive", "archiver"), ("security", "scanner"), ("ai", "ai-agent")]

def generate():
    data = json.load(open(INPUT))
    lines = ["// Generated by ci/gen_bot_lists.py from crates/rws-proxy/src/traffic/bots.json — don't edit.",
             f"// {data['ai_robots']['source']} (MIT); {data['crawlers']['source']} (MIT);",
             f"// crawler IP ranges as published, fetched {data['generated']}. Licenses: crates/rws-proxy/src/traffic/NOTICE.md.", "",
             "/// `(lowercase User-Agent token, name, operator, kind)` of AI agents.",
             "pub(super) const AI_AGENTS: &[(&str, &str, &str, &str)] = &["]
    for name, operator, function in data["ai_robots"]["agents"]:
        if name.lower() in NOT_AI:
            continue
        operator = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", operator)
        operator = "" if operator.lower().startswith("unclear") else operator
        lines.append(f"    ({rust_str(name.lower())}, {rust_str(name)}, {rust_str(operator)}, {rust_str(ai_kind(function))}),")
    lines.append("];\n")
    lines.append("/// `(anchored at the start, lowercase literal, name, kind)` of other bots.")
    lines.append("pub(super) const CRAWLERS: &[(bool, &str, &str, &str)] = &[")
    dropped = 0
    seen = set()
    ai_tokens = {a[0].lower() for a in data["ai_robots"]["agents"]} - set(NOT_AI)
    entries = [((False, token), name, kind) for token, name, kind in EXTRA_CRAWLERS]
    entries += [((False, token), token, kind) for token, kind in NOT_AI.items()]
    for pattern, tags in data["crawlers"]["patterns"]:
        found = literals(pattern)
        if not found:
            dropped += 1
            continue
        kind = next((k for key, k in CRAWLER_KIND if key in tags), "crawler")
        for lit in found:
            entries.append((lit, re.sub(r"[/ ].*$", "", lit[1]) or lit[1], kind))
    for lit, name, kind in entries:
        if lit in seen or lit[1] in ai_tokens:
            continue
        seen.add(lit)
        lines.append(f"    ({str(lit[0]).lower()}, {rust_str(lit[1])}, {rust_str(name)}, {rust_str(kind)}),")
    lines.append(f"];\n// {dropped} crawler-user-agents patterns are real regexes and aren't used.\n")
    lines.append("/// Published crawler networks: `(name, group, kind, UA token it verifies, prefixes)`.")
    lines.append("pub(super) const RANGES: &[(&str, &str, &str, &str, &[&str])] = &[")
    for r in data["ranges"]:
        prefixes = ", ".join(rust_str(p) for p in r["prefixes"])
        lines.append(f"    ({rust_str(r['name'])}, {rust_str(r['group'])}, {rust_str(r['kind'])}, {rust_str(r['verifies'])}, &[{prefixes}]),")
    lines.append("];")
    return "\n".join(lines) + "\n"

if __name__ == "__main__":
    if "--update" in sys.argv:
        update()
    out = generate()
    if "--check" in sys.argv:
        if open(OUTPUT).read() != out:
            sys.exit("crates/rws-proxy/src/traffic/data.rs is stale: run ci/gen_bot_lists.py")
        print("crates/rws-proxy/src/traffic/data.rs is up to date")
    else:
        open(OUTPUT, "w").write(out)
        print(f"wrote {OUTPUT}")
