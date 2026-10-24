#!/usr/bin/env bash
# HTTP/2 conformance: run h2spec (https://github.com/summerwind/h2spec)
# against a real rws over TLS. Fails on any failed or skipped test.
#
#   ci/e2e/h2spec.sh <path-to-rws> <path-to-h2spec>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
H2SPEC="$2"
PORT="${H2SPEC_PORT:-8543}"

WORK="$(mktemp -d)"
cleanup() {
  [ -n "${RWS_PID:-}" ] && kill "$RWS_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

cd "$WORK"
openssl req -x509 -newkey rsa:2048 -nodes -keyout key.pem -out cert.pem -days 1 \
  -subj "/CN=localhost" >/dev/null 2>&1
echo "h2spec" > index.html
cat > rws.config.toml <<EOF
port          = $PORT
tls_cert_file = "cert.pem"
tls_key_file  = "key.pem"
EOF

"$RWS" > rws.log 2>&1 &
RWS_PID=$!
for _ in $(seq 1 30); do
  grep -q "Listening for TLS" rws.log && break
  sleep 1
done
grep -q "Listening for TLS" rws.log || { echo "rws did not start:"; cat rws.log; exit 1; }

if ! "$H2SPEC" -h 127.0.0.1 -p "$PORT" -t -k -o 5 | tee h2spec.out | tail -3; then
  echo "--- failures"; grep -E "×" h2spec.out || true
  echo "--- rws log (tail)"; tail -40 rws.log
  exit 1
fi
if ! grep -qE "tests, ([0-9]+) passed, 0 skipped, 0 failed" h2spec.out; then
  echo "h2spec reported skipped or failed tests"; exit 1
fi

# SETTINGS_MAX_HEADER_LIST_SIZE (64 KiB): an oversized header block is
# refused, and the server keeps serving afterwards.
big="$(head -c 100000 /dev/zero | tr '\0' a)"
code="$(curl -sk --http2 -o /dev/null -w '%{http_code}' -H "X-Big: $big" "https://localhost:$PORT/" || true)"
[ "$code" != "200" ] || { echo "100 KB header block was accepted"; exit 1; }
code="$(curl -sk --http2 -o /dev/null -w '%{http_code}' "https://localhost:$PORT/")"
[ "$code" = "200" ] || { echo "server unhealthy after oversized headers: $code"; exit 1; }
echo "oversized header block refused; server still serving"
