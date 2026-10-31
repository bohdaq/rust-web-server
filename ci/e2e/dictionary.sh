#!/usr/bin/env bash
# Dictionary compression (RFC 9842) for static files in headless Chromium
# against the rws binary: v1 of a bundle is offered as a dictionary, v2
# arrives as a dcz delta that Chromium decodes to the exact file. Then curl
# forces each coding: sizes against br, dcz decoded by the zstd CLI, dcb by
# the system libbrotlidec (1.1+, through ctypes) — decoders rws doesn't share.
#
#   ci/e2e/dictionary.sh <path-to-rws>
# Needs node and `npm install playwright@1.64.0` + `npx playwright install chromium`
# in ci/e2e/dictionary (CI does both).
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HERE="$(cd "$(dirname "$0")/dictionary" && pwd)"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -30 "$WORK"/rws.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir -p www/js
echo '<!doctype html><title>dictionary</title>' > www/index.html
for v in 1 2; do
  python3 -I - "$v" > "www/js/app.v$v.js" <<'PY'
import random, sys
v = sys.argv[1]
rng = random.Random(7)   # same "code" in both versions, a few changed lines
words = ["render", "state", "props", "fetch", "cache", "route", "widget", "event", "value", "node"]
for i in range(1500):
    a, b, c = rng.choice(words), rng.choice(words), rng.randrange(10**6)
    tag = f"v{v}" if i % 300 == 0 else "v"
    print(f"export function {a}{i}({b}) {{ return {b}.{rng.choice(words)}({c}, '{tag}'); }}")
PY
done
cat > rws.config.toml <<'TOML'
port = 18530
compression_dictionaries = ["/js/app.*.js"]
static_cache_control = "public, max-age=3600"
TOML
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://localhost:18530/index.html && break; sleep 0.1; done

node "$HERE/check.mjs" http://localhost:18530 "$WORK/www" || fail "Chromium check failed"

# curl: hash of v1, each coding forced, sizes compared.
curl -s -o /dev/null http://localhost:18530/js/app.v1.js
held=":$(python3 -I -c 'import hashlib,base64,sys;print(base64.b64encode(hashlib.sha256(open(sys.argv[1],"rb").read()).digest()).decode())' www/js/app.v1.js):"
size() { curl -s -o "$WORK/out" -w '%{size_download} %header{content-encoding}' -H "Accept-Encoding: $1" -H "Available-Dictionary: $held" http://localhost:18530/js/app.v2.js; }
read -r br_size br_coding <<<"$(size br)"
[ "$br_coding" = br ] || fail "expected br, got $br_coding"
for coding in dcz dcb; do
  read -r d_size d_coding <<<"$(size "br, $coding")"
  [ "$d_coding" = "$coding" ] || fail "expected $coding, got $d_coding"
  [ "$d_size" -lt $((br_size / 3)) ] || fail "$coding is $d_size bytes, br $br_size"
  pass "$coding: $d_size bytes vs br $br_size bytes ($(wc -c < www/js/app.v2.js) uncompressed)"
done
# Each body decoded independently: dcz by the zstd CLI, dcb by the system
# libbrotlidec (>= 1.1, BrotliDecoderAttachDictionary) through ctypes.
cat > decode_dcb.py <<'PY'
import ctypes, ctypes.util, hashlib, sys
lib = ctypes.CDLL(ctypes.util.find_library("brotlidec") or "libbrotlidec.so.1")
lib.BrotliDecoderCreateInstance.restype = ctypes.c_void_p
lib.BrotliDecoderAttachDictionary.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_size_t, ctypes.c_char_p]
lib.BrotliDecoderDecompressStream.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_void_p),
                                              ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_void_p), ctypes.c_void_p]
data, dictionary = open(sys.argv[1], "rb").read(), open(sys.argv[2], "rb").read()
assert data[:4] == b"\xff\x44\x43\x42" and data[4:36] == hashlib.sha256(dictionary).digest(), "dcb header"
state = lib.BrotliDecoderCreateInstance(None, None, None)
assert lib.BrotliDecoderAttachDictionary(state, 0, len(dictionary), dictionary) == 1, "attach dictionary"
src = ctypes.create_string_buffer(data[36:], len(data) - 36)
out = ctypes.create_string_buffer(1 << 22)
avail_in, avail_out = ctypes.c_size_t(len(data) - 36), ctypes.c_size_t(len(out))
next_in, next_out = ctypes.c_void_p(ctypes.addressof(src)), ctypes.c_void_p(ctypes.addressof(out))
result = lib.BrotliDecoderDecompressStream(state, ctypes.byref(avail_in), ctypes.byref(next_in), ctypes.byref(avail_out), ctypes.byref(next_out), None)
assert result == 1, f"decoder result {result}"
sys.stdout.buffer.write(out.raw[: len(out) - avail_out.value])
PY
for coding in dcz dcb; do
  curl -s -o "$coding.body" -H "Accept-Encoding: $coding" -H "Available-Dictionary: $held" http://localhost:18530/js/app.v2.js
done
tail -c +41 dcz.body | zstd -q -d --patch-from=www/js/app.v1.js --long=23 -o dcz.out -f || fail "zstd CLI couldn't decode dcz"
cmp -s dcz.out www/js/app.v2.js || fail "dcz decoded by the zstd CLI differs from v2"
pass "dcz decodes with the zstd CLI to the exact v2"
python3 -I decode_dcb.py dcb.body www/js/app.v1.js > dcb.out || fail "libbrotlidec couldn't decode dcb"
cmp -s dcb.out www/js/app.v2.js || fail "dcb decoded by libbrotlidec differs from v2"
pass "dcb decodes with libbrotlidec to the exact v2"
[ "$(curl -s -o /dev/null -w '%header{content-encoding}' -H 'Accept-Encoding: br, dcz' -H 'Available-Dictionary: :AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=:' http://localhost:18530/js/app.v2.js)" = br ] || fail "an unknown dictionary didn't fall back to br"
pass "an unknown dictionary falls back to br"
echo "all dictionary compression end-to-end checks passed"
