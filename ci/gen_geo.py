#!/usr/bin/env python3
"""Country lookup table for rws's traffic analytics.

    ci/gen_geo.py --update [YYYY-MM]   download DB-IP Lite Country for that month
                                       (default: this month) and rewrite geo.bin
    ci/gen_geo.py --from <csv[.gz]>    build geo.bin from a downloaded file
    ci/gen_geo.py --check              decode geo.bin and check its structure (CI, offline)

Source: DB-IP "IP to Country Lite" (https://db-ip.com/db/lite.php),
CC BY 4.0 — attribution in crates/rws-proxy/src/traffic/NOTICE.md and on
the Traffic page.

Format of crates/rws-proxy/src/traffic/geo.bin — zlib-deflated:
    "RWSGEO1 <source name>\\n"
    varint countries, then 2 ASCII bytes per country ("--" = no country)
    varint n4, varint n6
    n4 IPv4 range starts (u32), delta-varint, ascending, first = 0
    n6 IPv6 range starts (upper 64 bits), delta-varint, ascending, first = 0
    n4 + n6 country indexes, one byte each
A range runs from its start to the next start; neighbours never share a
country. Varints are LEB128 (7 bits a byte, low first).
"""
import csv, datetime, gzip, io, ipaddress, os, sys, urllib.request, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUTPUT = os.path.join(ROOT, "crates/rws-proxy/src/traffic/geo.bin")
MAGIC = b"RWSGEO1 "
NONE = "--"


def varint(n, out):
    while n >= 0x80:
        out.append(n & 0x7F | 0x80)
        n >>= 7
    out.append(n)


def ranges(rows, bits, shift):
    """(start, end, country) rows → [(start, country)] covering 0..2^bits, gaps as NONE."""
    out = []
    nxt = 0
    for start, end, country in sorted(rows):
        start >>= shift
        end >>= shift
        if end < nxt:
            continue  # inside an IPv6 /64 already started
        start = max(start, nxt)
        if start > nxt:
            out.append((nxt, NONE))
        out.append((start, country))
        nxt = end + 1
    if nxt < 1 << bits:
        out.append((nxt, NONE))
    merged = []
    for start, country in out:
        if merged and merged[-1][1] == country:
            continue
        merged.append((start, country))
    return merged


def build(csv_bytes, name):
    v4, v6 = [], []
    for row in csv.reader(io.StringIO(csv_bytes.decode())):
        if len(row) < 3:
            continue
        a, b = ipaddress.ip_address(row[0]), ipaddress.ip_address(row[1])
        country = row[2].strip().upper()
        if len(country) != 2 or not country.isascii() or not country.isalpha() or country == "ZZ":
            country = NONE
        (v4 if a.version == 4 else v6).append((int(a), int(b), country))
    r4, r6 = ranges(v4, 32, 0), ranges(v6, 64, 64)
    countries = sorted({c for _, c in r4 + r6})
    assert len(countries) <= 256
    index = {c: i for i, c in enumerate(countries)}
    out = bytearray(MAGIC + name.encode() + b"\n")
    varint(len(countries), out)
    for c in countries:
        out += c.encode()
    varint(len(r4), out)
    varint(len(r6), out)
    for table in (r4, r6):
        prev = 0
        for start, _ in table:
            varint(start - prev, out)
            prev = start
    out += bytes(index[c] for _, c in r4 + r6)
    # Level 9, no timestamp in zlib: the same input always gives the same file.
    return zlib.compress(bytes(out), 9), len(r4), len(r6), len(countries)


def check(path):
    data = zlib.decompress(open(path, "rb").read())
    if not data.startswith(MAGIC):
        sys.exit(f"{path}: bad magic")
    nl = data.index(b"\n")
    pos = nl + 1

    def read():
        nonlocal pos
        n = shift = 0
        while True:
            b = data[pos]
            pos += 1
            n |= (b & 0x7F) << shift
            shift += 7
            if b < 0x80:
                return n

    nc = read()
    pos += 2 * nc
    n4, n6 = read(), read()
    for n, top in ((n4, 1 << 32), (n6, 1 << 64)):
        prev = None
        value = 0
        for i in range(n):
            d = read()
            if i and d == 0:
                sys.exit(f"{path}: starts not ascending")
            value += d
            if i == 0 and value != 0:
                sys.exit(f"{path}: first start isn't 0")
            if value >= top:
                sys.exit(f"{path}: start out of range")
    idx = data[pos:]
    if len(idx) != n4 + n6 or any(i >= nc for i in idx):
        sys.exit(f"{path}: bad country indexes")
    print(f"{path}: {data[len(MAGIC):nl].decode()}, {n4} IPv4 + {n6} IPv6 ranges, {nc} countries, {os.path.getsize(path)} bytes")


def main():
    args = sys.argv[1:]
    if args[:1] == ["--check"]:
        check(OUTPUT)
        return
    if args[:1] == ["--from"] and len(args) == 2:
        raw = open(args[1], "rb").read()
        name = os.path.basename(args[1]).removesuffix(".gz").removesuffix(".csv")
    elif args[:1] == ["--update"]:
        month = args[1] if len(args) > 1 else datetime.date.today().strftime("%Y-%m")
        name = f"dbip-country-lite-{month}"
        url = f"https://download.db-ip.com/free/{name}.csv.gz"
        req = urllib.request.Request(url, headers={"User-Agent": "rws gen_geo.py"})
        raw = urllib.request.urlopen(req, timeout=120).read()
    else:
        sys.exit(__doc__)
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    blob, n4, n6, nc = build(raw, name)
    with open(OUTPUT, "wb") as f:
        f.write(blob)
    print(f"{OUTPUT}: {name}, {n4} IPv4 + {n6} IPv6 ranges, {nc} countries, {len(blob)} bytes")
    check(OUTPUT)


if __name__ == "__main__":
    main()
