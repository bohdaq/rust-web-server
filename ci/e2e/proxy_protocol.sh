#!/usr/bin/env bash
# PROXY protocol behind HAProxy: with proxy_protocol = true, rws logs the
# client's address from the header HAProxy sends (send-proxy = v1,
# send-proxy-v2 = v2), on the plain and the TLS listener, not HAProxy's own.
# The client connects from 127.0.0.2 so its address differs from HAProxy's
# (127.0.0.1).
#
#   ci/e2e/proxy_protocol.sh <path-to-rws> <path-to-haproxy>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HAPROXY="$2"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -20 "$WORK"/*.log 2>/dev/null || true; exit 1; }
cd "$WORK"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 \
  -subj /CN=localhost -addext subjectAltName=DNS:localhost -keyout key.pem -out cert.pem 2>/dev/null

wait_port() {
  for _ in $(seq 1 60); do
    (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return 0
    sleep 0.25
  done
  fail "nothing listening on $1"
}

# Two rws: plain on 8791, TLS on 8792.
mkdir -p plain tls
printf "ip = '127.0.0.1'\nport = 8791\nproxy_protocol = true\nlog_format = 'combined'\n" > plain/rws.config.toml
printf "ip = '127.0.0.1'\nport = 8792\nproxy_protocol = true\nlog_format = 'combined'\ntls_cert_file = '$WORK/cert.pem'\ntls_key_file = '$WORK/key.pem'\n" > tls/rws.config.toml
(cd plain && exec "$RWS") > plain.log 2>&1 & PIDS+=($!)
(cd tls && exec "$RWS") > tls.log 2>&1 & PIDS+=($!)
wait_port 8791
wait_port 8792

cat > haproxy.cfg <<CFG
defaults
    mode tcp
    timeout connect 5s
    timeout client 30s
    timeout server 30s
frontend plain_v1
    bind 127.0.0.1:8781
    default_backend plain_v1
backend plain_v1
    server rws 127.0.0.1:8791 send-proxy
frontend plain_v2
    bind 127.0.0.1:8782
    default_backend plain_v2
backend plain_v2
    server rws 127.0.0.1:8791 send-proxy-v2
frontend tls_v1
    bind 127.0.0.1:8783
    default_backend tls_v1
backend tls_v1
    server rws 127.0.0.1:8792 send-proxy
frontend tls_v2
    bind 127.0.0.1:8784
    default_backend tls_v2
backend tls_v2
    server rws 127.0.0.1:8792 send-proxy-v2
CFG
"$HAPROXY" -c -f haproxy.cfg >/dev/null || fail "haproxy config"
"$HAPROXY" -f haproxy.cfg > haproxy.log 2>&1 & PIDS+=($!)
for port in 8781 8782 8783 8784; do wait_port "$port"; done

check() { # <label> <url> <log>
  local before after
  before=$(grep -c '^127.0.0.2 ' "$3" || true)
  curl -sk --http1.1 --interface 127.0.0.2 -o /dev/null -w '%{http_code}' "$2" | grep -q 200 || fail "$1: no 200 through HAProxy"
  sleep 0.5 # the access log is flushed every 100 ms
  after=$(grep -c '^127.0.0.2 ' "$3" || true)
  [ "$after" -gt "$before" ] || fail "$1: rws didn't log the client's address 127.0.0.2"
  echo "ok: $1"
}
check "plain, PROXY v1" http://127.0.0.1:8781/ plain.log
check "plain, PROXY v2" http://127.0.0.1:8782/ plain.log
check "TLS, PROXY v1" https://localhost:8783/ tls.log
check "TLS, PROXY v2" https://localhost:8784/ tls.log

# Straight to rws, without a header: refused.
if curl -s -m 5 -o /dev/null http://127.0.0.1:8791/; then fail "a connection without a PROXY header was served"; fi
echo "ok: a connection without a PROXY header is closed"
