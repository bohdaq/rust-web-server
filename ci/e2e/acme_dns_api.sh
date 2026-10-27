#!/usr/bin/env bash
# DNS-01 through provider APIs, and DNS-PERSIST-01, against Pebble resolving
# through BIND. ci/e2e/dns_api_mock.py plays Cloudflare and Route 53 and
# writes the records into BIND with nsupdate. rws must:
#   1. get example.test-wildcard certificates through the Cloudflare API
#      (Bearer token) and remove the records afterwards;
#   2. get one through Route 53 with requests the mock's own SigV4 check
#      accepts, and report SignatureDoesNotMatch with a wrong secret;
#   3. with dns-persist-01, print the record to publish, then — once it's
#      published — get a wildcard certificate, leaving the record in place.
#
#   ci/e2e/acme_dns_api.sh <rws> <pebble> <pebble-source-dir> <named> <nsupdate>
#
# rws must be built with the `acme` feature. Uses ports 8053 (BIND), 8450,
# 8451, 8452, 8453 (rws HTTPS), 9190 (Cloudflare mock), 9191 (Route 53
# mock), 14000/15000 (Pebble).
set -euo pipefail
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; PEBBLE="$(abs "$2")"; PEBBLE_SRC="$(cd "$3" && pwd)"; NAMED="$4"; NSUPDATE="$5"
MOCK="$(abs "$(dirname "$0")/dns_api_mock.py")"
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
  for log in "$WORK"/*/rws.log "$WORK/mock.log" "$WORK/named.log" "$WORK/pebble.log"; do
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
sans() { openssl x509 -in "$1" -noout -ext subjectAltName | tail -1 | tr -d ' '; }

# ── BIND: example.test, updatable with a TSIG key (by the mock and by us) ───
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
key "mock" { algorithm hmac-sha256; secret "$SECRET"; };
zone "example.test" {
    type primary;
    file "$WORK/bind/example.test.zone";
    update-policy { grant mock zonesub TXT; };
};
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

CF_TOKEN="cf-$(openssl rand -hex 8)"
AWS_KEY="AKIAE2ETEST"; AWS_SECRET_KEY="$(openssl rand -base64 30)"
python3 -I "$MOCK" "$NSUPDATE" 8053 mock "$SECRET" 9190 "$CF_TOKEN" 9191 "$AWS_KEY" "$AWS_SECRET_KEY" > "$WORK/mock.log" 2>&1 &
PIDS+=($!)
wait_for "$WORK/mock.log" "mock APIs ready" 15 || fail "the mock APIs did not start"

start_rws() { # <dir> <port> <domains TOML array> <[acme] lines> [env assignments...]
  local dir="$1" port="$2" domains="$3" lines="$4"
  shift 4
  mkdir -p "$WORK/$dir" && cd "$WORK/$dir"
  echo "hello from $dir" > index.html
  cat > rws.config.toml <<CFG
port = $port

[acme]
domains    = $domains
email      = "ci@example.com"
directory  = "https://localhost:14000/dir"
ca_file    = "$MINICA"
dns_server = "127.0.0.1:8053"
$lines
CFG
  "$RWS" --check rws.config.toml > check.log || fail "$dir: rws --check: $(cat check.log)"
  env "$@" "$RWS" > rws.log 2>&1 &
  PIDS+=($!)
}

# ── 1. Cloudflare ────────────────────────────────────────────────────────────
start_rws cloudflare 8450 '["cf.example.test", "*.cf.example.test"]' "challenge = \"dns-01\"
dns_provider = \"cloudflare\"
dns_api_url  = \"http://127.0.0.1:9190/client/v4\"" RWS_CONFIG_ACME_DNS_API_TOKEN="$CF_TOKEN"
wait_for rws.log "Listening for TLS" 120 || fail "cloudflare: no certificate"
echo "$(sans .rws/acme/cert.pem)" | grep -q 'DNS:\*\.cf\.example\.test' || fail "cloudflare: no wildcard: $(sans .rws/acme/cert.pem)"
grep -q "cloudflare GET /client/v4/zones?name=example.test" "$WORK/mock.log" || fail "cloudflare: the zone wasn't looked up"
grep -q "cloudflare DELETE" "$WORK/mock.log" || fail "cloudflare: the records weren't removed through the API"
[ -z "$(txt _acme-challenge.cf.example.test)" ] || fail "cloudflare: records left: $(txt _acme-challenge.cf.example.test)"
! grep -q "$CF_TOKEN" rws.log || fail "the Cloudflare token was printed"
pass "DNS-01 through the Cloudflare API: cf.example.test + wildcard issued, records removed"

# ── 2. Route 53 ──────────────────────────────────────────────────────────────
start_rws route53 8451 '["r53.example.test"]' "challenge = \"dns-01\"
dns_provider = \"route53\"
dns_api_url  = \"http://127.0.0.1:9191\"" AWS_ACCESS_KEY_ID="$AWS_KEY" AWS_SECRET_ACCESS_KEY="$AWS_SECRET_KEY"
wait_for rws.log "Listening for TLS" 120 || fail "route53: no certificate"
grep -q "route53 POST /2013-04-01/hostedzone/ZEXAMPLE/rrset" "$WORK/mock.log" || fail "route53: no change was sent"
grep -q "route53 SignatureDoesNotMatch" "$WORK/mock.log" && fail "route53: a request was signed wrongly"
[ -z "$(txt _acme-challenge.r53.example.test)" ] || fail "route53: record left: $(txt _acme-challenge.r53.example.test)"
pass "DNS-01 through Route 53, every request's SigV4 signature checked"

start_rws route53-bad 8452 '["bad.example.test"]' "challenge = \"dns-01\"
dns_provider = \"route53\"
dns_api_url  = \"http://127.0.0.1:9191\"" AWS_ACCESS_KEY_ID="$AWS_KEY" AWS_SECRET_ACCESS_KEY="wrong-$AWS_SECRET_KEY"
wait_for rws.log "SignatureDoesNotMatch|signature we calculated does not match" 60 || fail "route53: a wrong secret wasn't reported"
kill "${PIDS[-1]}" 2>/dev/null || true
pass "a wrong AWS secret is reported (SignatureDoesNotMatch)"

# ── 3. DNS-PERSIST-01 ────────────────────────────────────────────────────────
start_rws persist 8453 '["persist.example.test", "*.persist.example.test"]' "challenge = \"dns-persist-01\""
wait_for rws.log "publish this TXT record once" 90 || fail "persist: rws didn't say what to publish"
record="$(grep -o '_validation-persist\.[^ ]* TXT ".*"' rws.log | head -1)"
name="${record%% TXT *}"; value="${record#* TXT }"
echo "$value" | grep -q 'pebble.letsencrypt.org; accounturi=https://localhost:14000/.*; policy=wildcard' || fail "persist: unexpected record: $record"
printf 'server 127.0.0.1 8053\nzone example.test\nupdate add %s 60 TXT %s\nsend\n' "$name" "$value" | "$NSUPDATE" -y "hmac-sha256:mock:$SECRET"
wait_for rws.log "Listening for TLS" 120 || fail "persist: no certificate after the record was published"
grep -q "Found the DNS-PERSIST-01 record" rws.log || fail "persist: the record wasn't checked before asking the CA"
echo "$(sans .rws/acme/cert.pem)" | grep -q 'DNS:\*\.persist\.example\.test' || fail "persist: no wildcard: $(sans .rws/acme/cert.pem)"
[ -n "$(txt _validation-persist.persist.example.test)" ] || fail "persist: the record was removed"
pass "DNS-PERSIST-01: told what to publish, then issued the wildcard with the record left in place"

echo "all DNS API and DNS-PERSIST-01 checks passed"
