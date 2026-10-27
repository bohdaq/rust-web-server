#!/usr/bin/env bash
# Automatic HTTPS end to end against Pebble, Let's Encrypt's test CA
# (https://github.com/letsencrypt/pebble). rws runs from a directory holding
# only rws.config.toml and must:
#   1. get a certificate (HTTP-01 via its temporary challenge server),
#   2. serve HTTPS over HTTP/1.1 and HTTP/2, verified against Pebble's root,
#   3. proxy /api/* to a plain-HTTP backend over HTTP/2, relaying its
#      errors as-is (a 404 with a non-standard reason phrase, not a 502),
#   4. redirect HTTP -> HTTPS on IPv4 (and IPv6 when the host has it),
#   5. order the `shortlived` profile (a 6-day certificate) and not renew it
#      while it is current,
#   6. renew when Pebble's ACME Renewal Information (RFC 9773) says so,
#      naming the replaced certificate, with the challenge answered by the
#      redirect listener (Pebble's authorization reuse is off, so a real
#      challenge happens), and hot-swap to the new certificate without a
#      restart.
#   7. with challenge = "tls-alpn-01" (RFC 8737), on Pebble's TLS-ALPN port:
#      get the first certificate through a temporary acceptor on the HTTPS
#      socket, and renew through the live TLS listener — no port 80.
#
#   ci/e2e/acme_pebble.sh <path-to-rws> <path-to-pebble> <pebble-source-dir>
#
# rws must be built with the `acme` feature. Uses ports 8445 (HTTPS),
# 5002 (redirect + HTTP-01; Pebble's validation port), 5001 (HTTPS with
# TLS-ALPN-01; Pebble's), 9099 (backend), 14000/15000 (Pebble).
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
profile        = "shortlived"

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

# Check every 20s so step 6 happens within the test.
RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS=20 "$RWS" > rws.log 2>&1 &
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

# ── 5. profile, and no renewal while current ──────────────────────────────────
lifetime() {
  local dates nb na
  dates="$(echo | openssl s_client -connect 127.0.0.1:8445 -servername localhost 2>/dev/null | openssl x509 -noout -startdate -enddate)"
  nb="$(date -u -d "$(echo "$dates" | sed -n 's/^notBefore=//p')" +%s)"
  na="$(date -u -d "$(echo "$dates" | sed -n 's/^notAfter=//p')" +%s)"
  echo $((na - nb))
}
life="$(lifetime)"
# Pebble's shortlived profile: 518400 s (6 days), give or take a second.
[ "$life" -ge 518399 ] && [ "$life" -le 518401 ] || fail "certificate lifetime $life s, not the shortlived profile"
pass "ordered the shortlived profile (lifetime ${life}s)"
wait_for rws.log "is current; renewal due" 60 || fail "the renewal check never ran"
[ "$(grep -c 'Certificate written' rws.log)" = "1" ] || fail "renewed a current certificate"
pass "a current certificate is not renewed"

# ── 6. renewal when the CA's renewal information says so ─────────────────────
before="$(serial)"
# Pebble's management API overrides its ARI answer for a certificate: a
# window that has already opened means "renew now".
python3 -I - "$WORK/site/.rws/acme/cert.pem" > "$WORK/ari.json" <<'PY'
import json, sys, datetime
pem = open(sys.argv[1]).read()
first = pem[:pem.index("-----END CERTIFICATE-----") + len("-----END CERTIFICATE-----")] + "\n"
now = datetime.datetime.now(datetime.timezone.utc)
fmt = lambda t: t.strftime("%Y-%m-%dT%H:%M:%SZ")
window = {"suggestedWindow": {"start": fmt(now - datetime.timedelta(hours=1)), "end": fmt(now - datetime.timedelta(minutes=30))}}
print(json.dumps({"Certificate": first, "ARIResponse": json.dumps(window)}))
PY
curl -sf --cacert "$MINICA" -X POST --data @"$WORK/ari.json" https://localhost:15000/set-renewal-info/ \
  || fail "could not set Pebble's renewal information"
wait_for rws.log "replacing [A-Za-z0-9_-]+\.[A-Za-z0-9_-]+" 60 || fail "no order replacing the certificate (ARI ignored?)"
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
! grep -q "ordering without 'replaces'" rws.log || fail "the CA refused the order's 'replaces'"
pass "renewed on the CA's renewal information via the redirect listener and hot-swapped ($before -> $after)"

# ── 7. TLS-ALPN-01 ───────────────────────────────────────────────────────────
set_ari_now() { # <cert.pem>: Pebble's renewal information says "renew now"
  python3 -I - "$1" > "$WORK/ari.json" <<'PY'
import json, sys, datetime
pem = open(sys.argv[1]).read()
first = pem[:pem.index("-----END CERTIFICATE-----") + len("-----END CERTIFICATE-----")] + "\n"
now = datetime.datetime.now(datetime.timezone.utc)
fmt = lambda t: t.strftime("%Y-%m-%dT%H:%M:%SZ")
window = {"suggestedWindow": {"start": fmt(now - datetime.timedelta(hours=1)), "end": fmt(now - datetime.timedelta(minutes=30))}}
print(json.dumps({"Certificate": first, "ARIResponse": json.dumps(window)}))
PY
  curl -sf --cacert "$MINICA" -X POST --data @"$WORK/ari.json" https://localhost:15000/set-renewal-info/ \
    || fail "could not set Pebble's renewal information"
}
mkdir -p "$WORK/alpn" && cd "$WORK/alpn"
echo "hello over tls-alpn" > index.html
cat > rws.config.toml <<CFG
port = 5001

[acme]
domains   = ["localhost"]
email     = "ci@example.com"
directory = "https://localhost:14000/dir"
ca_file   = "$MINICA"
challenge = "tls-alpn-01"
profile   = "shortlived"
CFG
RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS=20 "$RWS" > rws.log 2>&1 &
PIDS+=($!)
alpn_fail() { cp rws.log "$WORK/site/rws.log"; fail "tls-alpn-01: $*"; }
wait_for rws.log "Listening for TLS" 90 || alpn_fail "rws never started its TLS listener"
grep -q "Answering TLS-ALPN-01 challenge for localhost on" rws.log || alpn_fail "no temporary acceptor at startup"
grep -q "Certificate written" rws.log || alpn_fail "no certificate was issued"
body="$(curl -sS --cacert "$WORK/root.pem" https://localhost:5001/)" || alpn_fail "HTTPS request failed"
[ "$body" = "hello over tls-alpn" ] || alpn_fail "body $body"
pass "TLS-ALPN-01: certificate issued on the HTTPS socket, served and verified"

alpn_serial() { echo | openssl s_client -connect 127.0.0.1:5001 -servername localhost 2>/dev/null | openssl x509 -noout -serial; }
before="$(alpn_serial)"
set_ari_now "$WORK/alpn/.rws/acme/cert.pem"
wait_for rws.log "Answering TLS-ALPN-01 challenge for localhost via the TLS listener" 60 \
  || alpn_fail "renewal not answered by the TLS listener"
for _ in $(seq 1 60); do
  [ "$(grep -c 'Certificate written' rws.log)" -ge 2 ] && break
  sleep 1
done
[ "$(grep -c 'Certificate written' rws.log)" -ge 2 ] || alpn_fail "renewed certificate never written"
sleep 2
after="$(alpn_serial)"
[ -n "$after" ] && [ "$before" != "$after" ] || alpn_fail "served certificate did not change"
pass "TLS-ALPN-01: renewed through the live TLS listener and hot-swapped ($before -> $after)"

echo "all ACME end-to-end checks passed"
