#!/usr/bin/env bash
# gRPC through rws against tonic (ci/e2e/grpc): unary, server streaming (and a
# status after messages), client streaming, bidirectional (the first echo
# arrives before the second message is sent) and a trailers-only error, via
# the config file's grpc action — once to a cleartext upstream (grpc://), once
# to a TLS upstream that requires a client certificate (grpcs:// with
# tls_ca_file and tls_client_cert_file/tls_client_key_file). The client talks
# HTTP/2 over TLS to rws.
#
#   ci/e2e/grpc.sh <path-to-rws> <path-to-e2e-server> <path-to-e2e-client>
set -euo pipefail

abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; SERVER="$(abs "$2")"; CLIENT="$(abs "$3")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -30 "$WORK"/*.log 2>/dev/null || true; exit 1; }
cd "$WORK"

# ── A private CA: localhost for rws and the TLS upstream, a client cert ──────
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 \
  -subj /CN=rws-e2e-ca -keyout ca.key -out ca.pem 2>/dev/null
leaf() { # <name> <cn> <extendedKeyUsage>
  openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj "/CN=$2" \
    -keyout "$1.key" -out "$1.csr" 2>/dev/null
  printf 'subjectAltName=DNS:localhost\nextendedKeyUsage=%s\n' "$3" > "$1.ext"
  openssl x509 -req -in "$1.csr" -CA ca.pem -CAkey ca.key -CAcreateserial -days 1 \
    -extfile "$1.ext" -out "$1.pem" 2>/dev/null
}
leaf server localhost serverAuth
leaf client rws-client clientAuth

wait_port() { # <port>
  for _ in $(seq 1 60); do
    (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return 0
    sleep 0.25
  done
  fail "nothing listening on $1"
}

"$SERVER" 127.0.0.1:50061 > upstream-plain.log 2>&1 & PIDS+=($!)
"$SERVER" 127.0.0.1:50062 server.pem server.key ca.pem > upstream-tls.log 2>&1 & PIDS+=($!)
wait_port 50061
wait_port 50062

run() { # <label> <upstream TOML lines>
  cat > rws.config.toml <<TOML
ip = '127.0.0.1'
port = 8761
tls_cert_file = '$WORK/server.pem'
tls_key_file = '$WORK/server.key'

[[upstream]]
name = "echo"
$2

[[route]]
  [route.match]
  path_prefix = "/echo.Echo/"
  [route.action]
  type = "grpc"
  upstream = "echo"
TOML
  "$RWS" --check rws.config.toml > check.log || fail "$1: rws --check: $(cat check.log)"
  RWS_CONFIG_FILE="$WORK/rws.config.toml" "$RWS" > "rws-$1.log" 2>&1 &
  local pid=$!
  PIDS+=($pid)
  wait_port 8761
  echo "── $1"
  "$CLIENT" https://localhost:8761 ca.pem || fail "$1"
  kill "$pid"
  wait "$pid" 2>/dev/null || true
}

run cleartext-upstream 'backends = ["grpc://127.0.0.1:50061"]'
run mtls-upstream "backends = [\"grpcs://localhost:50062\"]
tls_ca_file = '$WORK/ca.pem'
tls_client_cert_file = '$WORK/client.pem'
tls_client_key_file = '$WORK/client.key'"
echo "ok: gRPC through rws, cleartext and mutual-TLS upstreams"
