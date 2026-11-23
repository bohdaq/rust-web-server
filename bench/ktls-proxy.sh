#!/usr/bin/env bash
# Kernel TLS on upstream connections: an rws front (plain HTTP) proxies to an
# rws HTTPS backend; the front runs once with rustls upstream connections and
# once with `tls_ktls = true`. Prints req/s and each server's CPU time per
# request (the backend is the same in both runs).
#
#   RWS=target/release/rws OHA=oha bench/ktls-proxy.sh
#
# Tunables: DURATION (default 8s), CONNECTIONS (32), SIZES ("1k 64k 1m").
set -euo pipefail

RWS="$(cd "$(dirname "${RWS:?set RWS to the rws binary}")" && pwd)/$(basename "$RWS")"
OHA="${OHA:-oha}"
DURATION="${DURATION:-8s}"
CONNECTIONS="${CONNECTIONS:-32}"
SIZES="${SIZES:-1k 64k 1m}"
BACKEND_PORT="${BACKEND_PORT:-8393}"
FRONT_PORT="${FRONT_PORT:-8394}"

WORK="$(mktemp -d)"
PIDS=()
cleanup() { for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$WORK"; }
trap cleanup EXIT

mkdir -p "$WORK/site" "$WORK/front"
for s in $SIZES; do
  case "$s" in *k) n=$(( ${s%k} * 1024 ));; *m) n=$(( ${s%m} * 1048576 ));; *) n="$s";; esac
  head -c "$n" /dev/urandom > "$WORK/site/$s.bin"
done
# A CA and a leaf it signed (rustls refuses a self-signed CA as the leaf).
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 1 -subj /CN=bench-ca \
  -keyout "$WORK/ca.key" -out "$WORK/ca.pem" > /dev/null 2>&1
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -subj /CN=localhost \
  -keyout "$WORK/key.pem" -out "$WORK/leaf.csr" > /dev/null 2>&1
printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n' > "$WORK/ext.cnf"
openssl x509 -req -in "$WORK/leaf.csr" -CA "$WORK/ca.pem" -CAkey "$WORK/ca.key" -CAcreateserial -days 1 \
  -extfile "$WORK/ext.cnf" -out "$WORK/cert.pem" > /dev/null 2>&1

printf 'port = %s\nlog_format = "off"\nopen_file_cache_strict = false\ntls_cert_file = "%s"\ntls_key_file = "%s"\n' \
  "$BACKEND_PORT" "$WORK/cert.pem" "$WORK/key.pem" > "$WORK/backend.toml"
(cd "$WORK/site" && RWS_CONFIG_FILE="$WORK/backend.toml" exec "$RWS" > "$WORK/backend.log" 2>&1) &
BACKEND=$!; PIDS+=("$BACKEND")

cpu_ms() { awk -v hz="$(getconf CLK_TCK)" '{ print int(($14 + $15) * 1000 / hz) }' "/proc/$1/stat"; }
wait_ok() { for _ in $(seq 1 50); do curl -skf -o /dev/null "$1" && return 0; sleep 0.2; done; echo "no answer from $1" >&2; exit 1; }
wait_ok "https://127.0.0.1:$BACKEND_PORT/${SIZES%% *}.bin"

printf '| upstream TLS | file | req/s | front CPU ms/req | backend CPU ms/req |\n|---|---|---:|---:|---:|\n'
for ktls in false true; do
  cat > "$WORK/front.toml" <<EOF
port = $FRONT_PORT
log_format = "off"
tls_ktls = $ktls

[[upstream]]
name = "backend"
backends = ["https://127.0.0.1:$BACKEND_PORT"]
tls_ca_file = "$WORK/ca.pem"

[[route]]
  [route.match]
  path = "/*"
  [route.action]
  type = "proxy"
  upstream = "backend"
EOF
  (cd "$WORK/front" && RWS_CONFIG_FILE="$WORK/front.toml" exec "$RWS" > "$WORK/front.log" 2>&1) &
  FRONT=$!; PIDS+=("$FRONT")
  wait_ok "http://127.0.0.1:$FRONT_PORT/${SIZES%% *}.bin"
  for s in $SIZES; do
    url="http://127.0.0.1:$FRONT_PORT/$s.bin"
    "$OHA" -z 2s -c "$CONNECTIONS" --no-tui --disable-compression "$url" > /dev/null 2>&1 || true
    f0=$(cpu_ms "$FRONT"); b0=$(cpu_ms "$BACKEND")
    "$OHA" -z "$DURATION" -c "$CONNECTIONS" --no-tui --disable-compression --output-format json "$url" > "$WORK/out.json" 2>/dev/null || true
    f1=$(cpu_ms "$FRONT"); b1=$(cpu_ms "$BACKEND")
    python3 -I - "$WORK/out.json" "$ktls" "$s" "$((f1 - f0))" "$((b1 - b0))" <<'PY'
import json, sys
path, ktls, size, front, backend = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
d = json.load(open(path))
ok = sum(v for k, v in d.get("statusCodeDistribution", {}).items() if k.startswith("2"))
mode = "kTLS" if ktls == "true" else "rustls"
print(f"| {mode} | {size} | {d['summary']['requestsPerSec']:,.0f} | {front / ok if ok else float('nan'):.4f} | {backend / ok if ok else float('nan'):.4f} |")
PY
  done
  kill "$FRONT"; wait "$FRONT" 2>/dev/null || true
  sleep 6 # the old front drains before the port is free
done
