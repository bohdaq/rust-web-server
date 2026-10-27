#!/usr/bin/env bash
# gRPC-web from a browser: headless Chromium (Playwright) loads a page from
# rws that calls the tonic echo service (ci/e2e/grpc) with fetch — unary,
# server streaming, a status after a message, grpc-web-text and a
# trailers-only error — through the config file's grpc action, which bridges
# gRPC-web. Over plain HTTP (HTTP/1.1, chunked) and HTTPS (HTTP/2).
#
#   ci/e2e/grpc_web.sh <path-to-rws> <path-to-e2e-server>
# Needs node and `npm install playwright@1.64.0` + `npx playwright install chromium`
# in ci/e2e/grpc_web (CI does both).
set -euo pipefail

abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$1")"; SERVER="$(abs "$2")"
HERE="$(cd "$(dirname "$0")/grpc_web" && pwd)"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -20 "$WORK"/*.log 2>/dev/null || true; exit 1; }
cd "$WORK"

wait_port() {
  for _ in $(seq 1 60); do
    (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return 0
    sleep 0.25
  done
  fail "nothing listening on $1"
}

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 \
  -subj /CN=localhost -addext subjectAltName=DNS:localhost -keyout key.pem -out cert.pem 2>/dev/null
"$SERVER" 127.0.0.1:50071 > upstream.log 2>&1 & PIDS+=($!)
wait_port 50071

for scheme in http https; do
  mkdir -p "$scheme" && cp "$HERE/index.html" "$scheme/"
  tls=""
  port=8771
  if [ "$scheme" = https ]; then tls="tls_cert_file = '$WORK/cert.pem'
tls_key_file = '$WORK/key.pem'"; port=8772; fi
  cat > "$scheme/rws.config.toml" <<TOML
ip = '127.0.0.1'
port = $port
$tls

[[upstream]]
name = "echo"
backends = ["grpc://127.0.0.1:50071"]

[[route]]
  [route.match]
  path = "/echo.Echo/*"
  [route.action]
  type = "grpc"
  upstream = "echo"
TOML
  (cd "$scheme" && exec "$RWS") > "rws-$scheme.log" 2>&1 & PIDS+=($!)
  wait_port "$port"
  (cd "$HERE" && node check.mjs "$scheme://localhost:$port/index.html") || fail "$scheme"
done
echo "ok: gRPC-web from Chromium over HTTP/1.1 and HTTP/2"
