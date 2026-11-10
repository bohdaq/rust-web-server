#!/usr/bin/env python3
"""Hosting-network table for rws's traffic analytics: which addresses are
cloud and server hosting (a browser User-Agent from one is a program, not a
person reading the page).

    ci/gen_hosting.py --update [YYYY-MM]   download DB-IP Lite ASN for that month
                                           (default: this month) and rewrite hosting.bin
    ci/gen_hosting.py --from <csv[.gz]>    build hosting.bin from a downloaded file
    ci/gen_hosting.py --check              decode hosting.bin and check it (CI, offline)

Source: DB-IP "IP to ASN Lite" (https://db-ip.com/db/lite.php), CC BY 4.0
— attribution in crates/rws-proxy/src/traffic/NOTICE.md.

The file has geo.bin's format (ci/gen_geo.py) with a provider code where
geo.bin has a country: the networks of PROVIDERS get their code, any other
network whose name says hosting (HOSTING_WORDS) gets "DC", the rest "--".
Consumer VPN and CDN egress (EXCLUDE) is never hosting: people browse from
it (Cloudflare WARP, iCloud Private Relay, corporate proxies, VPN services).
The codes' names are in crates/rws-proxy/src/traffic/hosting.rs.
"""
import csv, datetime, gzip, io, os, re, sys, urllib.request

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_geo  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUTPUT = os.path.join(ROOT, "crates/rws-proxy/src/traffic/hosting.bin")

# code: ASNs. Big compute providers by name; the rest of hosting is "DC".
PROVIDERS = {
    "AW": [16509, 14618, 8987, 7224, 38895, 39111],  # Amazon (AWS)
    "GC": [396982, 19527, 139070, 45566],  # Google Cloud (not 15169: Google's own crawlers are verified by name)
    "AZ": [8075, 8068, 8069],  # Microsoft Azure
    "OR": [31898],  # Oracle Cloud
    "AL": [45102, 37963, 24429, 134963],  # Alibaba Cloud
    "TC": [132203, 45090],  # Tencent Cloud
    "HW": [136907, 55990, 265443],  # Huawei Cloud
    "DO": [14061],  # DigitalOcean
    "LI": [63949],  # Linode (Akamai)
    "VU": [20473],  # Vultr
    "OV": [16276, 35540],  # OVHcloud
    "HZ": [24940, 213230, 212317],  # Hetzner
    "CB": [51167, 40021, 141995],  # Contabo
    "SC": [12876],  # Scaleway
    "LW": [60781, 16265, 28753, 30633, 59253, 133752, 7203],  # Leaseweb
    "IO": [8560],  # IONOS
    "HG": [47583, 204915],  # Hostinger
    "IB": [36351],  # IBM Cloud
    "RS": [33070, 19994, 27357],  # Rackspace
}
# Other hosting and internet-scanning networks the names don't give away.
OTHER_HOSTING = [
    22168,  # Shadowserver
    398324, 398705, 398722,  # Censys
    36352,  # ColoCrossing
    40676,  # Psychz
    8100,  # QuadraNet
    46844,  # Sharktech
    53667,  # FranTech / BuyVM
    36007, 204548,  # Kamatera
    54290,  # Hostwinds
    59642, 16125,  # Cherry Servers
    218785,  # TC Datacenter
    201814,  # MEVSPACE
    64286,  # LogicWeb
    48090,  # Techoff SRV
    203020, 207990,  # HostRoyale
    197540,  # netcup
    62240,  # Clouvider
]
HOSTING_WORDS = re.compile(r"hosting|data ?cent(er|re)|datacenter|\bvps\b|dedicated|colocation|\bservers?\b|cloud", re.I)
EXCLUDE = {
    13335, 14789, 209242,  # Cloudflare (WARP, Private Relay)
    20940, 16625, 12222, 21342,  # Akamai (Private Relay)
    54113,  # Fastly (Private Relay)
    714, 6185,  # Apple
    15169,  # Google (Google One VPN, Googlebot is verified by name)
    9009,  # M247 (consumer VPN servers)
    60068, 212238,  # Datacamp / CDN77 (consumer VPN servers)
    147049,  # PacketHub (NordVPN)
    62044, 53813,  # Zscaler (corporate proxies)
    174,  # Cogent
}
CODES = {asn: code for code, asns in PROVIDERS.items() for asn in asns}
for asn in OTHER_HOSTING:
    CODES.setdefault(asn, "DC")


def code(asn, name):
    if asn in EXCLUDE:
        return None
    if asn in CODES:
        return CODES[asn]
    return "DC" if HOSTING_WORDS.search(name) else None


def to_rows(csv_bytes):
    """DB-IP ASN rows → `start,end,code` rows of the hosting networks only."""
    out = io.StringIO()
    w = csv.writer(out)
    for row in csv.reader(io.StringIO(csv_bytes.decode())):
        if len(row) < 4 or not row[2].isdigit():
            continue
        c = code(int(row[2]), row[3])
        if c:
            w.writerow([row[0], row[1], c])
    return out.getvalue().encode()


def main():
    args = sys.argv[1:]
    if args[:1] == ["--check"]:
        gen_geo.check(OUTPUT)
        return
    if args[:1] == ["--from"] and len(args) == 2:
        raw = open(args[1], "rb").read()
        month = re.search(r"\d{4}-\d{2}", os.path.basename(args[1]))
        name = f"dbip-asn-lite-{month.group(0) if month else 'local'}"
    elif args[:1] == ["--update"]:
        month = args[1] if len(args) > 1 else datetime.date.today().strftime("%Y-%m")
        name = f"dbip-asn-lite-{month}"
        url = f"https://download.db-ip.com/free/{name}.csv.gz"
        req = urllib.request.Request(url, headers={"User-Agent": "rws gen_hosting.py"})
        raw = urllib.request.urlopen(req, timeout=120).read()
    else:
        sys.exit(__doc__)
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    blob, n4, n6, nc = gen_geo.build(to_rows(raw), name)
    with open(OUTPUT, "wb") as f:
        f.write(blob)
    print(f"{OUTPUT}: {name}, {n4} IPv4 + {n6} IPv6 ranges, {nc} codes, {len(blob)} bytes")
    gen_geo.check(OUTPUT)


if __name__ == "__main__":
    main()
