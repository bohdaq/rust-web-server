#!/usr/bin/env bash
# A FIPS build (--features fips) against curl: `--version` says FIPS, the
# admin status lists the feature, HTTPS works with AES-GCM, and a TLS 1.3
# client offering only ChaCha20-Poly1305 (not FIPS-approved) is refused.
#
#   ci/e2e/fips.sh <path-to-rws built with fips>
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
fail() { echo "FAIL: $*"; tail -20 "$WORK"/rws.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www && echo fips-ok > www/index.html

"$RWS" --version | grep -q "(FIPS)$" || fail "--version doesn't say FIPS: $("$RWS" --version)"
pass "--version says FIPS"

export RWS_CONFIG_TLS_INTERNAL_DIR="$WORK/pki" RWS_ADMIN_TOKEN="fips-e2e-token-0123456789"
printf 'port = 18561\ntls = "internal"\n[admin]\nlisten = "127.0.0.1:18562"\n' > rws.config.toml
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null --cacert pki/ca.pem https://localhost:18561/ && break; sleep 0.1; done
[ "$(curl -s --cacert pki/ca.pem --tlsv1.3 https://localhost:18561/)" = fips-ok ] || fail "HTTPS (TLS 1.3, AES-GCM) failed"
[ "$(curl -s --cacert pki/ca.pem --tlsv1.2 --tls-max 1.2 --ciphers ECDHE-ECDSA-AES128-GCM-SHA256 https://localhost:18561/)" = fips-ok ] || fail "HTTPS (TLS 1.2, AES-GCM) failed"
pass "HTTPS with FIPS-approved suites (TLS 1.3 and 1.2, AES-GCM)"

if curl -s -o /dev/null --cacert pki/ca.pem --tlsv1.3 --tls13-ciphers TLS_CHACHA20_POLY1305_SHA256 https://localhost:18561/ 2>/dev/null; then
  fail "a ChaCha20-only TLS 1.3 client was served"
fi
pass "a ChaCha20-only client is refused"

curl -s -H "Authorization: Bearer $RWS_ADMIN_TOKEN" http://127.0.0.1:18562/admin/api/v1/status | grep -q '"fips"' || fail "status doesn't list the fips feature"
pass "admin status lists fips"

echo "all FIPS end-to-end checks passed"
