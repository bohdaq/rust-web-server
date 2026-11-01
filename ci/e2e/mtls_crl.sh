#!/usr/bin/env bash
# Certificate revocation lists for inbound mTLS, against the rws binary with
# curl and an openssl CA: a good client certificate is served, a revoked
# one fails the handshake, a CRL revoking more applies on SIGHUP and with
# `watch = true`, DER CRLs work, and `rws --check` rejects a junk CRL.
#
#   ci/e2e/mtls_crl.sh <path-to-rws>
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
mkdir www && echo hello > www/index.html

# ── a CA, a server certificate, two client certificates ─────────────────────
mkdir -p ca/newcerts && touch ca/index.txt && echo 1000 > ca/serial && echo 1000 > ca/crlnumber
cat > ca.cnf <<'CNF'
[ca]
default_ca = local
[local]
dir = ./ca
database = $dir/index.txt
new_certs_dir = $dir/newcerts
serial = $dir/serial
crlnumber = $dir/crlnumber
certificate = ./ca.pem
private_key = ./ca.key
default_md = sha256
default_days = 30
default_crl_days = 7
policy = any
[any]
commonName = supplied
[client]
basicConstraints = CA:FALSE
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
[server]
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost, IP:127.0.0.1
CNF
quiet() { "$@" >/dev/null 2>&1 || fail "$*"; }
quiet openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.pem -days 30 -subj "/CN=rws e2e CA" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign"
issue() { # <name> <extensions>
  quiet openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout "$1.key" -out "$1.csr" -subj "/CN=$1"
  quiet openssl ca -batch -config ca.cnf -extensions "$2" -in "$1.csr" -out "$1.pem" -notext
}
issue server server
issue alice client
issue mallory client
crl() { quiet openssl ca -batch -config ca.cnf -gencrl -out clients.crl.pem; }
crl
pass "CA, server and client certificates, empty CRL"

cat > rws.config.toml <<TOML
port = 18471
tls_cert_file = "$WORK/server.pem"
tls_key_file = "$WORK/server.key"
tls_client_ca_file = "$WORK/ca.pem"
tls_client_crl_file = "$WORK/clients.crl.pem"
TOML
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check rejected the CRL config: $(cat check.log)"

get() { # <client name> → HTTP status, or 000 when the handshake fails
  curl -s -o /dev/null -w '%{http_code}' --max-time 5 --cacert ca.pem --cert "$1.pem" --key "$1.key" https://localhost:18471/ || true
}
wait_up() {
  for _ in $(seq 1 100); do [ "$(get alice)" = 200 ] && return 0; sleep 0.1; done
  return 1
}

(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
wait_up || fail "rws didn't serve alice"
[ "$(get mallory)" = 200 ] || fail "mallory was refused before being revoked"
[ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 --cacert ca.pem https://localhost:18471/ || true)" = 000 ] || fail "no client certificate was let in"
pass "both clients served; no certificate refused"

quiet openssl ca -batch -config ca.cnf -revoke mallory.pem -crl_reason keyCompromise
crl
kill -HUP "${PIDS[-1]}"
for _ in $(seq 1 50); do [ "$(get mallory)" = 000 ] && break; sleep 0.1; done
[ "$(get mallory)" = 000 ] || fail "revoked mallory still served after SIGHUP"
[ "$(get alice)" = 200 ] || fail "alice refused after mallory's revocation"
pass "a revoked certificate fails the handshake after SIGHUP; others still served"

kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
openssl crl -in clients.crl.pem -outform DER -out clients.crl.der 2>/dev/null || fail "DER conversion"
sed -i "s|clients.crl.pem|clients.crl.der|" rws.config.toml
sed -i '1i watch = true' rws.config.toml
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws-der.log 2>&1) & PIDS+=($!)
wait_up || fail "rws didn't start with a DER CRL"
[ "$(get mallory)" = 000 ] || fail "the DER CRL didn't revoke mallory"
pass "DER CRL"

quiet openssl ca -batch -config ca.cnf -revoke alice.pem
crl
openssl crl -in clients.crl.pem -outform DER -out clients.crl.der.new 2>/dev/null && mv clients.crl.der.new clients.crl.der
for _ in $(seq 1 60); do [ "$(get alice)" = 000 ] && break; sleep 0.2; done
[ "$(get alice)" = 000 ] || fail "watch = true didn't pick up the new CRL"
pass "watch = true reloads a changed CRL without a signal"

echo "not a crl" > junk.crl
sed "s|$WORK/clients.crl.der|$WORK/junk.crl|" rws.config.toml > junk.toml
! "$RWS" --check=junk.toml >check.log 2>&1 || fail "rws --check accepted a junk CRL"
pass "rws --check rejects a junk CRL"

echo "all mTLS CRL end-to-end checks passed"
