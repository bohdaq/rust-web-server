#!/usr/bin/env bash
# Local HTTPS (tls = "internal") against the rws binary: rws creates its CA,
# serves a certificate for localhost and extra names, keeps the CA across
# restarts, `rws trust --print` prints it, and `rws --check` validates the
# setting. On CI (GITHUB_ACTIONS) also `rws trust` into the system store and
# curl without --cacert, then `rws untrust`; locally the system store is
# left alone.
#
#   ci/e2e/tls_internal.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -30 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www && echo hello-local-https > www/index.html
export RWS_CONFIG_TLS_INTERNAL_DIR="$WORK/pki"

cat > rws.config.toml <<'TOML'
port = 18481
tls = "internal"
tls_internal_names = "app.localhost"
TOML
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check: $(cat check.log)"
sed 's/tls = "internal"/tls = "selfsigned"/' rws.config.toml > bad.toml
! "$RWS" --check=bad.toml >check.log 2>&1 || fail "rws --check accepted tls = selfsigned"
grep -q 'expected \\"internal\\"\|expected "internal"' check.log || fail "no reason for the bad tls value: $(cat check.log)"
pass "rws --check validates tls"

start() {
  (cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >"../rws-$1.log" 2>&1) & PIDS+=($!)
  for _ in $(seq 1 100); do curl -s -o /dev/null --cacert pki/ca.pem https://localhost:18481/ && return 0; sleep 0.1; done
  fail "rws didn't serve HTTPS ($1)"
}
start first
[ -f pki/ca.pem ] && [ -f pki/ca.key ] || fail "no CA in $RWS_CONFIG_TLS_INTERNAL_DIR"
[ "$(stat -c %a pki/ca.key)" = 600 ] || fail "ca.key is not 0600"
[ "$(curl -s --cacert pki/ca.pem https://localhost:18481/)" = hello-local-https ] || fail "wrong body over HTTPS"
[ "$(curl -s --cacert pki/ca.pem --resolve app.localhost:18481:127.0.0.1 https://app.localhost:18481/)" = hello-local-https ] || fail "app.localhost not covered"
[ "$(curl -s --cacert pki/ca.pem https://127.0.0.1:18481/)" = hello-local-https ] || fail "127.0.0.1 not covered"
! curl -s -o /dev/null https://localhost:18481/ 2>/dev/null || [ -n "${GITHUB_ACTIONS:-}" ] || fail "the CA is trusted without rws trust?"
grep -q "local HTTPS: serving a certificate from rws's own CA" rws-first.log || fail "no startup line"
pass "HTTPS for localhost, 127.0.0.1 and extra names, trusted through the CA"

fingerprint=$(openssl x509 -in pki/ca.pem -noout -fingerprint -sha256)
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
start second
[ "$(openssl x509 -in pki/ca.pem -noout -fingerprint -sha256)" = "$fingerprint" ] || fail "the CA changed across restarts"
"$RWS" trust --print | openssl x509 -noout -fingerprint -sha256 | grep -qx "$fingerprint" || fail "rws trust --print printed another CA"
pass "the CA is kept across restarts; rws trust --print"

if [ -n "${GITHUB_ACTIONS:-}" ]; then
  "$RWS" trust || fail "rws trust"
  [ "$(curl -s https://localhost:18481/)" = hello-local-https ] || fail "curl doesn't trust the CA after rws trust"
  "$RWS" untrust || fail "rws untrust"
  ! curl -s -o /dev/null https://localhost:18481/ || fail "curl still trusts the CA after rws untrust"
  pass "rws trust / untrust change the system trust store"
fi

echo "all local HTTPS end-to-end checks passed"
