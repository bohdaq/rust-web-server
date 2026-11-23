#!/usr/bin/env bash
# Kernel TLS A/B: the same rws binary serving the same files over plain
# HTTP, over HTTPS with rustls, and over HTTPS with kTLS (`tls_ktls = true`).
# Prints req/s, throughput and the server's CPU time per request.
#
#   RWS=target/release/rws OHA=oha bench/ktls.sh
#
# Tunables: DURATION (default 10s), CONNECTIONS (default 32), SIZES
# (default "1k 64k 1m 16m"), MODES (default "http rustls ktls"; a build
# without the kTLS setting only runs "http rustls").
set -euo pipefail

RWS="${RWS:?set RWS to the rws binary}"
OHA="${OHA:-oha}"
DURATION="${DURATION:-10s}"
CONNECTIONS="${CONNECTIONS:-32}"
SIZES="${SIZES:-1k 64k 1m 16m}"
MODES="${MODES:-http rustls ktls}"
PORT="${PORT:-8391}"

WORK="$(mktemp -d)"
PID=""
cleanup() { [ -n "$PID" ] && kill "$PID" 2>/dev/null; wait 2>/dev/null || true; rm -rf "$WORK"; }
trap cleanup EXIT

mkdir -p "$WORK/static"
for s in $SIZES; do
  case "$s" in
    *k) n=$(( ${s%k} * 1024 ));;
    *m) n=$(( ${s%m} * 1024 * 1024 ));;
    *)  n="$s";;
  esac
  head -c "$n" /dev/urandom > "$WORK/static/$s.bin"
done
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 1 \
  -subj /CN=localhost -keyout "$WORK/key.pem" -out "$WORK/cert.pem" > /dev/null 2>&1

cpu_ms() {
  awk -v hz="$(getconf CLK_TCK)" '{ print int(($14 + $15) * 1000 / hz) }' "/proc/$1/stat"
}

start() { # <mode>
  local conf="$WORK/rws.toml"
  printf 'port = %s\nlog_format = "off"\nopen_file_cache_strict = false\n' "$PORT" > "$conf"
  if [ "$1" != http ]; then
    printf 'tls_cert_file = "%s"\ntls_key_file = "%s"\n' "$WORK/cert.pem" "$WORK/key.pem" >> "$conf"
  fi
  [ "$1" = ktls ] && printf 'tls_ktls = true\n' >> "$conf"
  (cd "$WORK/static" && RWS_CONFIG_FILE="$conf" exec "$RWS" > "$WORK/rws-$1.log" 2>&1) &
  PID=$!
  local scheme=http; [ "$1" != http ] && scheme=https
  for _ in $(seq 1 50); do
    curl -skf -o /dev/null "$scheme://127.0.0.1:$PORT/${SIZES%% *}.bin" && return 0
    sleep 0.2
  done
  echo "rws ($1) did not start; log:" >&2; cat "$WORK/rws-$1.log" >&2; exit 1
}

printf '| mode | file | req/s | MiB/s | CPU ms/req |\n|---|---|---:|---:|---:|\n'
for mode in $MODES; do
  start "$mode"
  scheme=http; [ "$mode" != http ] && scheme=https
  for s in $SIZES; do
    url="$scheme://127.0.0.1:$PORT/$s.bin"
    "$OHA" -z 2s -c "$CONNECTIONS" --no-tui --disable-compression --insecure "$url" > /dev/null 2>&1 || true
    before=$(cpu_ms "$PID")
    "$OHA" -z "$DURATION" -c "$CONNECTIONS" --no-tui --disable-compression --insecure \
      --http-version 1.1 --output-format json "$url" > "$WORK/out.json" 2>/dev/null || true
    after=$(cpu_ms "$PID")
    python3 -I - "$WORK/out.json" "$mode" "$s" "$((after - before))" <<'EOF'
import json, sys
path, mode, size, cpu = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
d = json.load(open(path))
s = d["summary"]
rps = s["requestsPerSec"]
ok = sum(v for k, v in d.get("statusCodeDistribution", {}).items() if k.startswith("2"))
mib = s.get("sizePerSec", 0) / 1048576
per = cpu / ok if ok else float("nan")
print(f"| {mode} | {size} | {rps:,.0f} | {mib:,.0f} | {per:.4f} |")
EOF
  done
  kill "$PID"; wait "$PID" 2>/dev/null || true; PID=""
done
