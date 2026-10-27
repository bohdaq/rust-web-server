#!/usr/bin/env bash
# Two replicas sharing certificates through Redis ([acme] redis), against
# Pebble. Replica A owns Pebble's HTTP-01 port (5002, its redirect
# listener); replica B's challenge port is one Pebble never connects to, so
# when B orders, its challenge can only be answered by A — from Redis.
#   1. A orders the certificate; B, started after, installs A's from Redis
#      without ordering, and both serve the same certificate;
#   2. when the CA's renewal information says "renew", exactly one replica
#      orders, and both end up serving the same new certificate;
#   3. two orders in all — never two for one certificate.
#
#   ci/e2e/acme_shared.sh <rws> <pebble> <pebble-source-dir> <redis-server>
#
# rws must be built with the `acme` feature. Uses ports 8461, 8462 (rws
# HTTPS), 5002 (A's redirect listener; Pebble's HTTP-01 port), 6390 (Redis),
# 14000/15000 (Pebble).
set -euo pipefail
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; PEBBLE="$(abs "$2")"; PEBBLE_SRC="$(cd "$3" && pwd)"; REDIS="$4"
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
  for log in "$WORK"/a/rws.log "$WORK"/b/rws.log "$WORK/pebble.log"; do
    [ -f "$log" ] && { echo "--- $log (tail)"; tail -40 "$log"; }
  done
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
serial() { echo | openssl s_client -connect "127.0.0.1:$1" -servername localhost 2>/dev/null | openssl x509 -noout -serial; }
orders() { cat "$WORK"/a/rws.log "$WORK"/b/rws.log 2>/dev/null | grep -c "\[ACME\] Order:" || true; }

"$REDIS" --port 6390 --save "" --appendonly no > "$WORK/redis.log" 2>&1 &
PIDS+=($!)
wait_for "$WORK/redis.log" "Ready to accept connections" 30 || fail "Redis did not start"

(cd "$PEBBLE_SRC" && PEBBLE_AUTHZREUSE=0 PEBBLE_VA_NOSLEEP=1 PEBBLE_WFE_NONCEREJECT=0 \
  exec "$PEBBLE" -config test/config/pebble-config.json > "$WORK/pebble.log" 2>&1) &
PIDS+=($!)
wait_for "$WORK/pebble.log" "ACME directory available" 30 || fail "pebble did not start"

start_replica() { # <name> <https port> <extra top-level TOML> <challenge port> <check interval>
  mkdir -p "$WORK/$1" && cd "$WORK/$1"
  echo "replica $1" > index.html
  cat > rws.config.toml <<CFG
port = $2
$3

[acme]
domains        = ["localhost"]
email          = "ci@example.com"
directory      = "https://localhost:14000/dir"
ca_file        = "$MINICA"
challenge_port = $4
profile        = "shortlived"
redis          = "127.0.0.1:6390"
CFG
  RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS=$5 "$RWS" > rws.log 2>&1 &
  PIDS+=($!)
}

# ── 1. A orders; B installs it from Redis ────────────────────────────────────
start_replica a 8461 "http_redirect_port = 5002" 5002 20
wait_for "$WORK/a/rws.log" "Listening for TLS" 90 || fail "replica A never served TLS"
[ "$(orders)" = "1" ] || fail "replica A: $(orders) orders"
start_replica b 8462 "" 5003 10
wait_for "$WORK/b/rws.log" "Listening for TLS" 90 || fail "replica B never served TLS"
grep -q "Installed the certificate another replica stored" "$WORK/b/rws.log" || fail "replica B didn't install A's certificate"
[ "$(orders)" = "1" ] || fail "replica B ordered too ($(orders) orders)"
first="$(serial 8461)"
[ -n "$first" ] && [ "$first" = "$(serial 8462)" ] || fail "the replicas serve different certificates"
pass "one order; the second replica installed the first one's certificate ($first)"

# ── 2. renewal: one replica orders, both swap ────────────────────────────────
python3 -I - "$WORK/a/.rws/acme/cert.pem" > "$WORK/ari.json" <<'PY'
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
for _ in $(seq 1 120); do
  a="$(serial 8461)"; b="$(serial 8462)"
  [ -n "$a" ] && [ "$a" != "$first" ] && [ "$a" = "$b" ] && break
  sleep 1
done
[ "$a" != "$first" ] && [ "$a" = "$b" ] || fail "the replicas didn't both swap to one renewed certificate ($first -> A $a, B $b)"
[ "$(orders)" = "2" ] || fail "$(orders) orders for two certificates"
renewer="A"; grep -q "replacing" "$WORK/b/rws.log" && renewer="B"
pass "renewed once (by $renewer) and both replicas swapped ($first -> $a)"

echo "all shared-storage checks passed"
