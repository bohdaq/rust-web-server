#!/usr/bin/env bash
# TLS keys on a PKCS#11 token (#79), against the rws binary (built with the
# pkcs11 feature) and SoftHSM2: an EC P-256 key and an RSA 2048 key imported
# into a token serve HTTPS (TLS 1.3 and 1.2); a wrong PIN fails to start
# TLS with a clear error; rws --check accepts the URI.
#
#   ci/e2e/pkcs11.sh <path-to-rws>     (needs softhsm2, openssl, curl)
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
fail() { echo "FAIL: $*"; tail -20 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www tokens && echo hsm-ok > www/index.html

MODULE=""
for m in /usr/lib/softhsm/libsofthsm2.so /usr/lib/*/softhsm/libsofthsm2.so /usr/local/lib/softhsm/libsofthsm2.so; do [ -f "$m" ] && { MODULE=$m; break; }; done
[ -n "$MODULE" ] || fail "libsofthsm2.so not found"
export SOFTHSM2_CONF="$WORK/softhsm2.conf"
printf 'directories.tokendir = %s/tokens\nobjectstore.backend = file\nlog.level = ERROR\n' "$WORK" > "$SOFTHSM2_CONF"
softhsm2-util --init-token --free --label rws --pin 1234 --so-pin 0000 >/dev/null
export HSM_PIN=1234

import() { # <label> <id> <openssl genpkey args…>
  local label=$1 id=$2; shift 2
  openssl genpkey "$@" -out "$label.key" 2>/dev/null
  openssl req -x509 -new -key "$label.key" -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost" -days 2 -out "$label.pem" 2>/dev/null
  softhsm2-util --import "$label.key" --token rws --label "$label" --id "$id" --pin 1234 >/dev/null
  rm "$label.key"   # only the token has it now
}
import ec-key 01 -algorithm EC -pkeyopt ec_paramgen_curve:P-256
import rsa-key 02 -algorithm RSA -pkeyopt rsa_keygen_bits:2048
pass "SoftHSM2 token with an EC P-256 and an RSA 2048 key (private keys deleted from disk)"

serve() { # <name> <label> <pin variable>
  cat > rws.config.toml <<TOML
port = 18571
tls_cert_file = "$WORK/$2.pem"
tls_key_file = "pkcs11:token=rws;object=$2?module-path=$MODULE&pin-source=env:$3"
TOML
  if [ "$1" = wrongpin ]; then
    ! "$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check accepted a wrong PIN"
    grep -q "PIN is incorrect" check.log || fail "rws --check didn't say why: $(cat check.log)"
  else
    "$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check: $(cat check.log)"
  fi
  (cd www && exec env RWS_CONFIG_FILE=../rws.config.toml ${RUNTIME:+RWS_CONFIG_RUNTIME=$RUNTIME} "$RWS" >"../rws-$1.log" 2>&1) & PIDS+=($!)
}
probe() { # <cert> <curl tls args…>
  local cert=$1; shift
  for _ in $(seq 1 50); do
    [ "$(curl -s --cacert "$cert" "$@" https://localhost:18571/ 2>/dev/null)" = hsm-ok ] && return 0
    sleep 0.2
  done
  return 1
}

serve ec ec-key HSM_PIN
probe ec-key.pem --tlsv1.3 || fail "HTTPS with the EC key on the token (TLS 1.3)"
probe ec-key.pem --tlsv1.2 --tls-max 1.2 || fail "HTTPS with the EC key on the token (TLS 1.2)"
pass "EC P-256 key on the token: TLS 1.3 and 1.2"
grep -q "runtime: shared (a TLS key is on a PKCS#11 token" rws-ec.log || fail "the binary didn't keep the shared runtime for a pkcs11: key"
pids=(); for i in $(seq 1 24); do curl -s --cacert ec-key.pem https://localhost:18571/ >"par.out.$i" & pids+=($!); done
for p in "${pids[@]}"; do wait "$p" || fail "a parallel handshake failed"; done
[ "$(cat par.out.* | grep -c hsm-ok)" = 24 ] || fail "parallel handshakes: $(cat par.out.* | grep -c hsm-ok)/24"
rm -f par.out.*
pass "shared runtime by default with a pkcs11: key; 24 parallel handshakes"
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true

RUNTIME=per-core serve percore ec-key HSM_PIN
probe ec-key.pem --tlsv1.3 || fail "HTTPS with the EC key on per-core runtimes"
grep -q "per-core with a PKCS#11 TLS key" rws-percore.log || fail "no warning for per-core with a pkcs11: key"
pass "explicit per-core still serves, with a startup warning"
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true

serve rsa rsa-key HSM_PIN
probe rsa-key.pem --tlsv1.3 || fail "HTTPS with the RSA key on the token (TLS 1.3, RSA-PSS)"
probe rsa-key.pem --tlsv1.2 --tls-max 1.2 || fail "HTTPS with the RSA key on the token (TLS 1.2)"
pass "RSA 2048 key on the token: TLS 1.3 (RSA-PSS) and 1.2"
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true

export WRONG_PIN=9999
serve wrongpin ec-key WRONG_PIN
sleep 2
! probe ec-key.pem --tlsv1.3 2>/dev/null || fail "served with a wrong PIN"
grep -qi "pkcs#11: login" rws-wrongpin.log || fail "no clear error for a wrong PIN: $(tail -5 rws-wrongpin.log)"
pass "a wrong PIN: rws --check fails saying so, and nothing is served"

export RWS_TEST_PKCS11_URI="pkcs11:token=rws;object=ec-key?module-path=$MODULE&pin-source=env:HSM_PIN"
echo "$RWS_TEST_PKCS11_URI" > "${PKCS11_URI_OUT:-/dev/null}"
echo "all PKCS#11 end-to-end checks passed"
