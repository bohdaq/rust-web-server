#!/usr/bin/env bash
# Slow / idle clients must not starve the server ("Slowloris"). Opens 1000
# idle connections against plain HTTP and against HTTPS, then requires a
# normal request to succeed within 2 seconds on each; checks that a client
# that never finishes the TLS handshake is dropped at the handshake deadline.
#
#   ci/e2e/slow_clients.sh <path-to-rws>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -30 "$WORK"/*/rws.log 2>/dev/null || true; exit 1; }

ulimit -n 4096 2>/dev/null || true

start_rws() { # <dir> <config>
  mkdir -p "$1" && cd "$1"
  echo "ok" > index.html
  printf '%s\n' "$2" > rws.config.toml
  "$RWS" > rws.log 2>&1 &
  PIDS+=($!)
  for _ in $(seq 1 30); do
    grep -qE "serving plain HTTP|Listening for TLS" rws.log && return 0
    sleep 1
  done
  fail "rws did not start in $1"
}

hold_idle() { # <port> <count> <seconds>
  python3 -I -c "
import socket, sys, time
socks = [socket.create_connection(('127.0.0.1', $1)) for _ in range($2)]
print('holding', len(socks), 'idle connections', flush=True)
time.sleep($3)
" &
  PIDS+=($!)
  sleep 3
}

# ── plain HTTP ────────────────────────────────────────────────────────────────
start_rws "$WORK/plain" "port = 8601"
hold_idle 8601 1000 20
code="$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:8601/ || true)"
[ "$code" = "200" ] || fail "plain HTTP with 1000 idle connections: got '$code'"
echo "ok: plain HTTP serves with 1000 idle connections open"

# ── HTTPS ─────────────────────────────────────────────────────────────────────
mkdir -p "$WORK/tls"
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$WORK/tls/key.pem" -out "$WORK/tls/cert.pem" \
  -days 1 -subj "/CN=localhost" >/dev/null 2>&1
start_rws "$WORK/tls" $'port = 8602\ntls_cert_file = "cert.pem"\ntls_key_file = "key.pem"'
hold_idle 8602 1000 20
for proto in --http1.1 --http2; do
  code="$(curl -sk "$proto" -m 2 -o /dev/null -w '%{http_code}' https://localhost:8602/ || true)"
  [ "$code" = "200" ] || fail "HTTPS $proto with 1000 idle connections: got '$code'"
done
echo "ok: HTTPS (HTTP/1.1 and HTTP/2) serves with 1000 idle connections open"

# A client that connects and never starts the handshake is dropped at the deadline.
elapsed="$(python3 -I -c "
import socket, time
s = socket.create_connection(('127.0.0.1', 8602)); s.settimeout(30); t = time.time()
try: s.recv(1)
except Exception: pass
print(int(time.time() - t))
")"
[ "$elapsed" -le 12 ] || fail "silent TLS client held for ${elapsed}s (handshake deadline is 10s)"
echo "ok: silent TLS client dropped after ${elapsed}s"
