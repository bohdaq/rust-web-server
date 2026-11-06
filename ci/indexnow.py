#!/usr/bin/env python3
"""Announce the rws8.tech family's pages to IndexNow (Bing, Yandex, Seznam,
Naver; Bing's index also feeds ChatGPT search).

    ci/indexnow.py                     every URL in each site's live sitemap
    ci/indexnow.py --changed-since D   only sitemap URLs with lastmod >= D (YYYY-MM-DD)
    ci/indexnow.py --dry-run           print what would be sent

Each site serves the key file at /<KEY>.txt (written by its prerender
step), which is how the search engines check the request is the owner's.
Run it after a deploy; it only reads public sitemaps and posts URL lists.
"""
import json, re, sys, urllib.request

KEY = "2c32426256ab5e82b6a7fa63c90848c0"
SITES = ["rws8.tech", "lit.rws8.tech", "hitop.rws8.tech"]
ENDPOINT = "https://api.indexnow.org/indexnow"
UA = "rws indexnow.py"


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    return urllib.request.urlopen(req, timeout=30).read().decode()


def urls(host, since):
    xml = fetch(f"https://{host}/sitemap.xml")
    out = []
    for loc, mod in re.findall(r"<url><loc>([^<]+)</loc>(?:<lastmod>([^<]+)</lastmod>)?</url>", xml):
        if since and (not mod or mod[:10] < since):
            continue
        out.append(loc)
    return out


def main():
    args = sys.argv[1:]
    dry = "--dry-run" in args
    since = args[args.index("--changed-since") + 1] if "--changed-since" in args else None
    for host in SITES:
        if fetch(f"https://{host}/{KEY}.txt").strip() != KEY:
            print(f"{host}: key file missing or wrong; skipped")
            continue
        found = urls(host, since)
        print(f"{host}: {len(found)} URLs")
        if dry or not found:
            continue
        for i in range(0, len(found), 10000):
            body = json.dumps({"host": host, "key": KEY, "keyLocation": f"https://{host}/{KEY}.txt", "urlList": found[i : i + 10000]}).encode()
            req = urllib.request.Request(ENDPOINT, data=body, headers={"Content-Type": "application/json; charset=utf-8", "User-Agent": UA})
            with urllib.request.urlopen(req, timeout=30) as r:
                print(f"  sent {len(found[i : i + 10000])}: HTTP {r.status}")


if __name__ == "__main__":
    main()
