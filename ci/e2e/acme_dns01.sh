#!/usr/bin/env bash
# DNS-01 end to end: Pebble (Let's Encrypt's test CA) validates against BIND,
# which rws updates. rws must:
#   1. with dns_provider = "rfc2136" and a TSIG key, get a certificate for
#      example.test and *.example.test (the wildcard needs DNS-01), serve it,
#      and remove the challenge record afterwards;
#   2. with a wrong TSIG secret, say so (BADSIG) instead of failing silently;
#   3. with dns_provider = "exec", get one through a hook script (nsupdate).
#
#   ci/e2e/acme_dns01.sh <rws> <pebble> <pebble-source-dir> <named> <nsupdate>
#
# rws must be built with the `acme` feature. Uses ports 8053 (BIND), 8446,
# 8447, 8448 (rws HTTPS), 14000/15000 (Pebble).
set -euo pipefail
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; PEBBLE="$(abs "$2")"; PEBBLE_SRC="$(cd "$3" && pwd)"; NAMED="$4"; NSUPDATE="$5"
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
  for log in "$WORK"/*/rws.log "$WORK/named.log" "$WORK/pebble.log"; do
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
txt() { dig +short -p 8053 @127.0.0.1 TXT "$1"; }

# ── BIND: zone example.test, updatable with the TSIG key ─────────────────────
SECRET="$(openssl rand -base64 32)"
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
key "rws-acme" { algorithm hmac-sha256; secret "$SECRET"; };
zone "example.test" {
    type primary;
    file "$WORK/bind/example.test.zone";
    update-policy { grant rws-acme zonesub TXT; };
};
CONF
# A copy of named: the distribution's AppArmor profile confines /usr/sbin/named
# to /etc/bind and /var/cache/bind.
cp "$NAMED" "$WORK/named"
"$WORK/named" -g -c "$WORK/bind/named.conf" > "$WORK/named.log" 2>&1 &
PIDS+=($!)
wait_for "$WORK/named.log" "running" 30 || fail "BIND did not start"
# "running" is logged a moment before queries are answered on a slow runner.
answer=""
for _ in $(seq 1 20); do
  answer=$(dig +short +tries=1 +time=1 -p 8053 @127.0.0.1 A www.example.test 2>&1 || true)
  [ "$answer" = "127.0.0.1" ] && break
  sleep 0.5
done
[ "$answer" = "127.0.0.1" ] || fail "BIND doesn't answer for example.test: $answer"

# ── Pebble, resolving through BIND ───────────────────────────────────────────
(cd "$PEBBLE_SRC" && PEBBLE_AUTHZREUSE=0 PEBBLE_VA_NOSLEEP=1 PEBBLE_WFE_NONCEREJECT=0 \
  exec "$PEBBLE" -config test/config/pebble-config.json -dnsserver 127.0.0.1:8053 > "$WORK/pebble.log" 2>&1) &
PIDS+=($!)
wait_for "$WORK/pebble.log" "ACME directory available" 30 || fail "pebble did not start"

start_rws() { # <dir> <port> <domains TOML array> <dns lines>
  mkdir -p "$WORK/$1" && cd "$WORK/$1"
  echo "hello from $1" > index.html
  cat > rws.config.toml <<CFG
port = $2

[acme]
domains   = $3
email     = "ci@example.com"
directory = "https://localhost:14000/dir"
ca_file   = "$MINICA"
challenge = "dns-01"
$4
CFG
  "$RWS" > rws.log 2>&1 &
  PIDS+=($!)
}

# ── 1. RFC 2136 + TSIG, with a wildcard ──────────────────────────────────────
start_rws rfc2136 8446 '["example.test", "*.example.test"]' "dns_provider    = \"rfc2136\"
dns_server      = \"127.0.0.1:8053\"
dns_tsig_key    = \"rws-acme\"
dns_tsig_secret = \"$SECRET\""
"$RWS" --check rws.config.toml > check.log || fail "rws --check: $(cat check.log)"
wait_for rws.log "Listening for TLS" 120 || fail "rfc2136: rws never started its TLS listener"
grep -q "Certificate written" rws.log || fail "rfc2136: no certificate was issued"
! grep -q "$SECRET" rws.log || fail "the TSIG secret was printed"
curl -sf --cacert "$MINICA" https://localhost:15000/roots/0 > "$WORK/root.pem" || fail "could not fetch Pebble's root"
sans="$(openssl x509 -in .rws/acme/cert.pem -noout -ext subjectAltName)"
echo "$sans" | grep -q "DNS:\*\.example\.test" || fail "no wildcard in the certificate: $sans"
echo "$sans" | grep -q "DNS:example\.test" || fail "no example.test in the certificate: $sans"
body="$(curl -sS --cacert "$WORK/root.pem" --resolve any.example.test:8446:127.0.0.1 https://any.example.test:8446/)" \
  || fail "rfc2136: HTTPS for a name under the wildcard failed"
[ "$body" = "hello from rfc2136" ] || fail "rfc2136: body $body"
[ -z "$(txt _acme-challenge.example.test)" ] || fail "the challenge record was left behind: $(txt _acme-challenge.example.test)"
pass "DNS-01 over RFC 2136 + TSIG: example.test and *.example.test issued, served, record removed"

# ── 2. a wrong TSIG secret is reported ───────────────────────────────────────
start_rws badkey 8447 '["bad.example.test"]' "dns_provider    = \"rfc2136\"
dns_server      = \"127.0.0.1:8053\"
dns_tsig_key    = \"rws-acme\"
dns_tsig_secret = \"$(openssl rand -base64 32)\""
wait_for rws.log "BADSIG" 60 || fail "a wrong TSIG secret wasn't reported as BADSIG"
pass "a wrong TSIG secret is reported (BADSIG)"
kill "${PIDS[-1]}" 2>/dev/null || true

# ── 3. exec provider ─────────────────────────────────────────────────────────
cat > "$WORK/hook.sh" <<HOOK
#!/bin/sh
# <present|cleanup> <fqdn> <value>
case "\$1" in
  present) change="update add \$2 60 TXT \"\$3\"" ;;
  cleanup) change="update delete \$2 TXT \"\$3\"" ;;
  *) exit 2 ;;
esac
printf 'server 127.0.0.1 8053\nzone example.test\n%s\nsend\n' "\$change" | "$NSUPDATE" -y "hmac-sha256:rws-acme:$SECRET"
HOOK
chmod +x "$WORK/hook.sh"
start_rws exec 8448 '["exec.example.test"]' "dns_provider = \"exec\"
dns_exec     = \"$WORK/hook.sh\"
dns_server   = \"127.0.0.1:8053\""
wait_for rws.log "Listening for TLS" 120 || fail "exec: rws never started its TLS listener"
grep -q "Certificate written" rws.log || fail "exec: no certificate was issued"
grep -q "_acme-challenge.exec.example.test is visible" rws.log || fail "exec: the record wasn't seen at dns_server"
[ -z "$(txt _acme-challenge.exec.example.test)" ] || fail "exec: the cleanup hook left the record"
pass "DNS-01 through an exec hook (nsupdate): issued, record removed"

echo "all DNS-01 end-to-end checks passed"
