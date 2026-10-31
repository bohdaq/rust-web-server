#!/usr/bin/env bash
# rws as a wasi:http component (rws-wasm-shim) under `wasmtime serve`: a
# static page from a granted directory, the built-in /healthz, a 404, and a
# file large enough to stream, byte for byte.
#
#   ci/e2e/wasm_shim.sh <rws_wasm_shim.wasm> <wasmtime>
# Build the component with
#   cargo build --release --target wasm32-wasip2 -p rws-wasm-shim
set -euo pipefail

WASM="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WASMTIME="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
WORK="$(mktemp -d)"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
cd "$WORK"
mkdir www
echo '<!doctype html><title>wasm</title><p>served from a wasi:http guest</p>' > www/index.html
head -c 3000000 /dev/urandom > www/big.bin

"$WASMTIME" serve -Scli --addr 127.0.0.1:18490 --dir www::. "$WASM" > wasmtime.log 2>&1 & PID=$!
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18490/healthz && break; sleep 0.1; done

fail() { echo "FAIL: $*"; tail -20 wasmtime.log; exit 1; }
body=$(curl -sS http://127.0.0.1:18490/) || fail "no answer"
[[ "$body" == *"served from a wasi:http guest"* ]] || fail "index.html not served: $body"
echo "ok: a static page from the granted directory"
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18490/healthz)" = 200 ] || fail "/healthz"
echo "ok: /healthz"
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18490/no-such-page)" = 404 ] || fail "missing page isn't 404"
echo "ok: 404 for a missing page"
curl -sS -o got.bin http://127.0.0.1:18490/big.bin || fail "big file"
cmp -s www/big.bin got.bin || fail "big file differs ($(stat -c %s got.bin) bytes)"
echo "ok: a 3 MB file streamed byte for byte"
echo "all wasm shim checks passed"
