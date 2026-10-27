#!/usr/bin/env bash
# WebSocket routes and streaming SSE end to end (examples/realtime.rs), with
# Node's WebSocket and fetch as clients (ci/e2e/realtime_client.mjs): plain
# and TLS, shared and per-core runtimes, plus SSE over HTTP/2 with curl.
#
#   ci/e2e/realtime.sh <path-to-realtime-example>
#
# Uses ports 8481 and 8482. Needs node (22+), curl and openssl.
set -euo pipefail
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
APP="$(abs "$1")"
CLIENT="$(abs "$(dirname "$0")/realtime_client.mjs")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() {
  echo "FAIL: $*"
  for log in "$WORK"/*.log; do [ -f "$log" ] && { echo "--- $log (tail)"; tail -30 "$log"; }; done
  exit 1
}
wait_for() { # <file> <pattern> <seconds>
  for _ in $(seq 1 $(( $3 * 4 ))); do
    grep -qE "$2" "$1" 2>/dev/null && return 0
    sleep 0.25
  done
  return 1
}

cd "$WORK"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 -subj /CN=realtime-ca \
  -keyout ca.key -out ca.pem 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj /CN=localhost -keyout server.key -out server.csr 2>/dev/null
printf 'subjectAltName=DNS:localhost\n' > san.ext
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 1 -extfile san.ext -out server.pem 2>/dev/null

start() { # <name> <port> <runtime> [tls]
  local tls=()
  [ "${4:-}" = tls ] && tls=(RWS_CONFIG_TLS_CERT_FILE="$WORK/server.pem" RWS_CONFIG_TLS_KEY_FILE="$WORK/server.key")
  env RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT="$2" RWS_CONFIG_RUNTIME="$3" REALTIME_EVENT_MS=400 "${tls[@]}" "$APP" > "$1.log" 2>&1 &
  PIDS+=($!)
  local ready="serving plain HTTP|Listening for TLS"
  [ "$3" = per-core ] && ready="Started [0-9]+ per-core runtimes"
  wait_for "$1.log" "$ready" 30 || fail "$1 didn't start"
}
stop() { kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true; unset 'PIDS[-1]'; }

for runtime in shared per-core; do
  start "plain-$runtime" 8481 "$runtime"
  echo "── plain, $runtime runtime"
  node "$CLIENT" http://localhost:8481 || fail "plain, $runtime runtime"
  stop

  start "tls-$runtime" 8482 "$runtime" tls
  echo "── TLS, $runtime runtime"
  NODE_EXTRA_CA_CERTS="$WORK/ca.pem" node "$CLIENT" https://localhost:8482 || fail "TLS, $runtime runtime"
  # SSE over HTTP/2 (WebSocket over HTTP/2 isn't offered: RFC 8441 is not implemented).
  body="$(curl -sS -N --http2 --cacert ca.pem -w '\n%{http_version}' https://localhost:8482/events)" || fail "SSE over HTTP/2"
  [ "$(echo "$body" | tail -1)" = 2 ] || fail "SSE wasn't served over HTTP/2: $(echo "$body" | tail -1)"
  [ "$(echo "$body" | grep -c '^data: event ')" = 3 ] || fail "SSE over HTTP/2: $body"
  echo "ok: SSE over HTTP/2"
  stop
done

echo "all WebSocket and SSE checks passed"
