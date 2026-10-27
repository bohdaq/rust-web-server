#!/usr/bin/env bash
# On-demand TLS ([acme] on_demand) against Pebble, which resolves
# *.example.test to 127.0.0.1 through BIND. An ask endpoint allows names
# starting with "shop" or "multi" and refuses the rest. rws must:
#   1. order a certificate at the first handshake for an allowed name and
#      complete that handshake with it;
#   2. refuse a name the ask endpoint refuses (the default certificate is
#      served, nothing ordered, and it isn't asked again right away);
#   3. order once for five simultaneous first handshakes to one name;
#   4. after a restart, serve the certificates from disk without ordering.
#
#   ci/e2e/acme_on_demand.sh <rws> <pebble> <pebble-source-dir> <named>
#
# rws must be built with the `acme` feature. Uses ports 8053 (BIND), 8470
# (rws HTTPS), 5002 (rws redirect listener; Pebble's HTTP-01 port), 9180
# (ask endpoint), 14000/15000 (Pebble).
set -euo pipefail
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; PEBBLE="$(abs "$2")"; PEBBLE_SRC="$(cd "$3" && pwd)"; NAMED="$4"
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
  for log in "$WORK"/rws*.log "$WORK/ask.log" "$WORK/pebble.log"; do
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
subject() { echo | openssl s_client -connect 127.0.0.1:8470 -servername "$1" 2>/dev/null | openssl x509 -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' '; }

# ── BIND: example.test and every name under it at 127.0.0.1 ─────────────────
mkdir -p "$WORK/bind"
cat > "$WORK/bind/example.test.zone" <<ZONE
\$TTL 60
@   IN SOA ns.example.test. admin.example.test. 1 60 60 600 60
@   IN NS  ns.example.test.
ns  IN A   127.0.0.1
@   IN A   127.0.0.1
*   IN A   127.0.0.1
ZONE
cat > "$WORK/bind/named.conf" <<CONF
options {
    directory "$WORK/bind";
    pid-file "$WORK/bind/named.pid";
    listen-on port 8053 { 127.0.0.1; };
    listen-on-v6 { none; };
    recursion no;
    dnssec-validation no;
};
zone "example.test" { type primary; file "$WORK/bind/example.test.zone"; };
CONF
# A copy of named: the distribution's AppArmor profile confines /usr/sbin/named.
cp "$NAMED" "$WORK/named"
"$WORK/named" -g -c "$WORK/bind/named.conf" > "$WORK/named.log" 2>&1 &
PIDS+=($!)
wait_for "$WORK/named.log" "running" 30 || fail "BIND did not start"

(cd "$PEBBLE_SRC" && PEBBLE_AUTHZREUSE=0 PEBBLE_VA_NOSLEEP=1 PEBBLE_WFE_NONCEREJECT=0 \
  exec "$PEBBLE" -config test/config/pebble-config.json -dnsserver 127.0.0.1:8053 > "$WORK/pebble.log" 2>&1) &
PIDS+=($!)
wait_for "$WORK/pebble.log" "ACME directory available" 30 || fail "pebble did not start"

# ── the ask endpoint ─────────────────────────────────────────────────────────
cat > "$WORK/ask.py" <<'PY'
import http.server, urllib.parse, sys
class Ask(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        name = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query).get("domain", [""])[0]
        allowed = name.startswith(("shop", "multi"))
        print(f"ask {name} {'allowed' if allowed else 'refused'}", flush=True)
        self.send_response(200 if allowed else 403)
        self.send_header("Content-Length", "0")
        self.end_headers()
    def log_message(self, *args):
        pass
http.server.ThreadingHTTPServer(("127.0.0.1", 9180), Ask).serve_forever()
PY
python3 -I "$WORK/ask.py" > "$WORK/ask.log" 2>&1 &
PIDS+=($!)

mkdir -p "$WORK/site" && cd "$WORK/site"
echo "hello on demand" > index.html
cat > rws.config.toml <<CFG
port = 8470
http_redirect_port = 5002

[acme]
domains        = ["saas.example.test"]
email          = "ci@example.com"
directory      = "https://localhost:14000/dir"
ca_file        = "$MINICA"
challenge_port = 5002
on_demand      = true
on_demand_ask  = "http://127.0.0.1:9180/ask"
CFG
"$RWS" --check rws.config.toml > "$WORK/check.log" || fail "rws --check: $(cat "$WORK/check.log")"
start_rws() { # <log>
  "$RWS" > "$1" 2>&1 &
  PIDS+=($!)
  wait_for "$1" "Listening for TLS" 120 || fail "rws never started its TLS listener"
  wait_for "$1" "On-demand TLS" 10 || fail "on-demand TLS didn't start"
}
start_rws "$WORK/rws.log"
curl -sf --cacert "$MINICA" https://localhost:15000/roots/0 > "$WORK/root.pem" || fail "could not fetch Pebble's root"

# ── 1. an allowed name: ordered during its first handshake ───────────────────
body="$(curl -sS --cacert "$WORK/root.pem" --resolve shop.example.test:8470:127.0.0.1 https://shop.example.test:8470/)" \
  || fail "the first handshake for shop.example.test failed"
[ "$body" = "hello on demand" ] || fail "body: $body"
[ "$(subject shop.example.test)" = "DNS:shop.example.test" ] || fail "shop.example.test got $(subject shop.example.test)"
[ -f .rws/acme/on-demand/shop.example.test/cert.pem ] || fail "no certificate file for shop.example.test"
pass "an allowed name got its own certificate during its first handshake"

# ── 2. a refused name: the default certificate, nothing ordered ──────────────
[ "$(subject evil.example.test)" = "DNS:saas.example.test" ] || fail "evil.example.test got $(subject evil.example.test)"
subject evil.example.test > /dev/null
[ "$(grep -c "ask evil.example.test" "$WORK/ask.log")" = "1" ] || fail "evil.example.test was asked about $(grep -c "ask evil.example.test" "$WORK/ask.log") times"
grep -q "getting a certificate for 'evil" "$WORK/rws.log" && fail "a refused name was ordered"
[ ! -e .rws/acme/on-demand/evil.example.test ] || fail "files for a refused name"
pass "a refused name gets the default certificate, is not ordered and not asked again at once"

# ── 3. five first handshakes at once: one order ──────────────────────────────
for _ in 1 2 3 4 5; do
  curl -sS -o /dev/null --cacert "$WORK/root.pem" --resolve multi.example.test:8470:127.0.0.1 https://multi.example.test:8470/ &
done
wait_ok=0
for job in $(jobs -p); do
  case " ${PIDS[*]} " in *" $job "*) continue ;; esac
  wait "$job" || wait_ok=1
done
[ "$wait_ok" = 0 ] || fail "a simultaneous first handshake failed"
[ "$(grep -c "getting a certificate for 'multi.example.test'" "$WORK/rws.log")" = "1" ] || fail "multi.example.test was ordered $(grep -c "getting a certificate for 'multi.example.test'" "$WORK/rws.log") times"
pass "five simultaneous first handshakes, one order"

# ── 4. restart: certificates from disk ───────────────────────────────────────
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
start_rws "$WORK/rws2.log"
grep -q "(2 loaded)" "$WORK/rws2.log" || fail "the on-demand certificates weren't loaded at startup: $(grep "On-demand TLS" "$WORK/rws2.log")"
[ "$(subject shop.example.test)" = "DNS:shop.example.test" ] || fail "after a restart shop.example.test got $(subject shop.example.test)"
grep -q "getting a certificate" "$WORK/rws2.log" && fail "ordered again after a restart"
pass "after a restart the certificates come from disk"

echo "all on-demand TLS checks passed"
