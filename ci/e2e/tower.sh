#!/usr/bin/env bash
# Tower interop over real HTTP/2: examples/tower.rs serves tonic-health's
# generated service through rws's h2 server (cleartext h2c here), and an rws
# route behind a tower-http layer.
#
#   ci/e2e/tower.sh <path-to-tower-example> <path-to-grpcurl>
set -euo pipefail

EXAMPLE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
GRPCURL="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
PROTO_DIR="$(cd "$(dirname "$0")/tower" && pwd)"
WORK="$(mktemp -d)"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -20 "$WORK/example.log" 2>/dev/null || true; exit 1; }
cd "$WORK"

RWS_CONFIG_PORT=8500 "$EXAMPLE" > example.log 2>&1 &
PID=$!
for _ in $(seq 1 30); do
  curl -sf -o /dev/null http://127.0.0.1:8500/hello && break
  sleep 0.5
done

curl -s -D head.out -o body.out http://127.0.0.1:8500/hello
grep -qi '^x-served-by: rws + tower-http' head.out || fail "tower-http layer header missing: $(cat head.out)"
grep -q '"hello":"tower"' body.out || fail "route body: $(cat body.out)"
echo "ok: an rws route behind tower-http's SetResponseHeaderLayer"

grpc() { "$GRPCURL" -plaintext -import-path "$PROTO_DIR" -proto health.proto "$@" 127.0.0.1:8500 "${METHOD}"; }
METHOD=grpc.health.v1.Health/Check
out=$(grpc -d '{"service":""}' 2>&1) || fail "Check failed: $out"
echo "$out" | grep -q '"status": "SERVING"' || fail "Check answered: $out"
echo "ok: tonic-health Check over h2c → SERVING (body and grpc-status trailer)"

METHOD=grpc.health.v1.Health/List
out=$(grpc -d '{}' 2>&1 || true)
echo "$out" | grep -qiE 'Unimplemented|statuses' || fail "List answered: $out"
echo "ok: List → $(echo "$out" | grep -oiE 'Code: [A-Za-z]+|statuses' | head -1)"

METHOD=grpc.health.v1.Health/Watch
out=$(timeout 5 "$GRPCURL" -plaintext -max-time 2 -import-path "$PROTO_DIR" -proto health.proto -d '{"service":""}' 127.0.0.1:8500 "$METHOD" 2>&1 || true)
echo "$out" | grep -q '"status": "SERVING"' || fail "Watch (server streaming) answered: $out"
echo "ok: Watch streams its first message before the call ends"
echo "tower: all checks passed"
