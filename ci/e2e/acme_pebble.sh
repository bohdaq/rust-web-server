#!/usr/bin/env bash
# Automatic HTTPS end to end against Pebble, Let's Encrypt's test CA
# (https://github.com/letsencrypt/pebble). rws runs from a directory holding
# only rws.config.toml and must:
#   1. get a certificate (HTTP-01 via its temporary challenge server),
#   2. serve HTTPS over HTTP/1.1 and HTTP/2, verified against Pebble's root,
#   3. proxy /api/* to a plain-HTTP backend over HTTP/2, relaying its
#      errors as-is (a 404 with a non-standard reason phrase, not a 502),
#   4. redirect HTTP -> HTTPS on IPv4 (and IPv6 when the host has it),
#   5. renew with the challenge answered by the redirect listener (Pebble's
#      authorization reuse is off, so a real challenge happens) and hot-swap
#      to the new certificate without a restart.
#
#   ci/e2e/acme_pebble.sh <path-to-rws> <path-to-pebble> <pebble-source-dir>
#
# rws must be built with the `acme` feature. Uses ports 8445 (HTTPS),
# 5002 (redirect + HTTP-01; Pebble's validation port), 9099 (backend),
# 14000/15000 (Pebble).
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
PEBBLE="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
PEBBLE_SRC="$(cd "$3" && pwd)"
MINICA="$PEBBLE_SRC/test/certs/pebble.minica.pem"

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
  echo "--- rws log (tail)"; tail -60 "$WORK/site/rws.log" 2>/dev/null || true
  echo "--- pebble log (tail)"; tail -20 "$WORK/pebble.log" 2>/dev/null || true
  exit 1
}
pass() { echo "ok: $*"; }

wait_for() { # <file> <pattern> <seconds>
  for _ in $(seq 1 "$3"); do
    grep -qE "$2" "$1" 2>/dev/null && return 0
    sleep 1
  done
  return 1
}

serial() {
  echo | openssl s_client -connect 127.0.0.1:8445 -servername localhost 2>/dev/null \
    | openssl x509 -noout -serial
}

# ── Pebble ────────────────────────────────────────────────────────────────────
# exec: the recorded PID must be Pebble itself, not a wrapping subshell,
# or cleanup leaves Pebble running.
(cd "$PEBBLE_SRC" && PEBBLE_AUTHZREUSE=0 PEBBLE_VA_NOSLEEP=1 PEBBLE_WFE_NONCEREJECT=0 \
  exec "$PEBBLE" -config test/config/pebble-config.json > "$WORK/pebble.log" 2>&1) &
PIDS+=($!)
wait_for "$WORK/pebble.log" "ACME directory available" 30 || fail "pebble did not start"

# ── backend for the proxy route ──────────────────────────────────────────────
mkdir -p "$WORK/backend/api" && echo "from-backend" > "$WORK/backend/api/index.html"
(cd "$WORK/backend" && exec python3 -I -m http.server 9099 --bind 127.0.0.1 > "$WORK/backend.log" 2>&1) &
PIDS+=($!)

# ── rws ───────────────────────────────────────────────────────────────────────
mkdir -p "$WORK/site" && cd "$WORK/site"
echo "hello over acme" > index.html
cat > rws.config.toml <<EOF
port               = 8445
http_redirect_port = 5002

[acme]
domains        = ["localhost"]
email          = "ci@example.com"
directory      = "https://localhost:14000/dir"
ca_file        = "$MINICA"
challenge_port = 5002

[[upstream]]
name     = "app"
backends = ["127.0.0.1:9099"]

[[route]]
  [route.match]
  path = "/api/*"

  [route.action]
  type     = "proxy"
  upstream = "app"
EOF

# Renew on every 20s check so step 5 happens within the test.
RWS_CONFIG_ACME_RENEW_BEFORE_DAYS=1000 RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS=20 \
  "$RWS" > rws.log 2>&1 &
PIDS+=($!)

wait_for rws.log "Listening for TLS" 90 || fail "rws never started its TLS listener"
grep -q "Certificate written" rws.log || fail "no certificate was issued"
pass "certificate issued"

curl -sf --cacert "$MINICA" https://localhost:15000/roots/0 > "$WORK/root.pem" \
  || fail "could not fetch Pebble's root"

# ── 2. HTTPS ──────────────────────────────────────────────────────────────────
for proto in --http1.1 --http2; do
  body="$(curl -sS "$proto" --cacert "$WORK/root.pem" https://localhost:8445/)" \
    || fail "HTTPS $proto request failed"
  [ "$body" = "hello over acme" ] || fail "HTTPS $proto body: $body"
done
pass "HTTPS over HTTP/1.1 and HTTP/2, verified against Pebble's root"

# ── 3. proxy over HTTP/2 ──────────────────────────────────────────────────────
headers="$(curl -sS --http2 -D - -o "$WORK/proxied" --cacert "$WORK/root.pem" https://localhost:8445/api/)" \
  || fail "proxied HTTP/2 request failed (duplicate content-length?)"
[ "$(cat "$WORK/proxied")" = "from-backend" ] || fail "proxied body: $(cat "$WORK/proxied")"
[ "$(echo "$headers" | grep -ci '^content-type:')" = "1" ] || fail "proxied Content-Type count: $headers"
pass "proxy over HTTP/2 with one Content-Type / Content-Length"
code="$(curl -s --http2 -o /dev/null -w '%{http_code}' --cacert "$WORK/root.pem" https://localhost:8445/api/missing)"
[ "$code" = "404" ] || fail "backend 404 relayed as $code"
pass "backend 404 relayed as 404"

# ── 4. redirect ───────────────────────────────────────────────────────────────
loc="$(curl -s -4 -o /dev/null -w '%{http_code} %{redirect_url}' http://localhost:5002/x?y=1)"
[ "$loc" = "301 https://localhost:8445/x?y=1" ] || fail "IPv4 redirect: $loc"
if curl -s -6 -o /dev/null http://[::1]:5002/ 2>/dev/null; then
  code="$(curl -s -6 -o /dev/null -w '%{http_code}' http://[::1]:5002/)"
  [ "$code" = "301" ] || fail "IPv6 redirect: $code"
  pass "redirect on IPv4 and IPv6"
else
  pass "redirect on IPv4 (no IPv6 loopback on this host)"
fi

# ── 5. renewal through the redirect listener ──────────────────────────────────
before="$(serial)"
wait_for rws.log "Answering HTTP-01 challenge via the redirect listener" 60 \
  || fail "renewal challenge was not answered by the redirect listener"
for _ in $(seq 1 60); do
  [ "$(grep -c 'Certificate written' rws.log)" -ge 2 ] && break
  sleep 1
done
[ "$(grep -c 'Certificate written' rws.log)" -ge 2 ] || fail "renewed certificate never written"
sleep 2
after="$(serial)"
[ -n "$after" ] && [ "$before" != "$after" ] || fail "served certificate did not change ($before -> $after)"
pass "renewed via the redirect listener and hot-swapped ($before -> $after)"

echo "all ACME end-to-end checks passed"
