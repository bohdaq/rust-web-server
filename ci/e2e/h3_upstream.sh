#!/usr/bin/env bash
# HTTP/3 to upstreams ([[upstream]] protocol = "h3") against the rws binary:
# a proxy rws forwards to a backend rws over QUIC (the backend's access log
# shows HTTP/3), and to a TCP-only HTTPS backend (Python) by falling back
# to TCP — counted once, then remembered.
#
#   ci/e2e/h3_upstream.sh <path-to-rws>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  [ -n "${KEEP:-}" ] && { echo "kept $WORK"; for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done; return; }
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in "$WORK"/*.log; do echo "== $f"; tail -15 "$f"; done; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

# A self-signed certificate for localhost, trusted by the proxy as its CA.
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 2 -subj /CN=localhost \
  -addext "subjectAltName=DNS:localhost" -addext "basicConstraints=critical,CA:FALSE" -keyout key.pem -out cert.pem 2>/dev/null

# The HTTP/3 backend: rws with TLS (TCP and QUIC on 18541).
mkdir -p backend/www && echo "from the h3 backend" > backend/www/hello.txt
cat > backend/rws.config.toml <<TOML
port = 18541
tls_cert_file = "$WORK/cert.pem"
tls_key_file = "$WORK/key.pem"
TOML
(cd backend/www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../../backend.log 2>&1) & PIDS+=($!)

# A TCP-only HTTPS backend: nothing on its UDP port.
cat > tcp_backend.py <<'PY'
import http.server, ssl, sys
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        out = b"from the tcp backend"
        self.send_response(200); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
    def log_message(self, *a): pass
server = http.server.ThreadingHTTPServer(("127.0.0.1", 18542), H)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(sys.argv[1], sys.argv[2])
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
PY
python3 -I tcp_backend.py cert.pem key.pem & PIDS+=($!)

mkdir -p proxy/www
cat > proxy/rws.config.toml <<TOML
port = 18540

[[upstream]]
name = "quic"
protocol = "h3"
backends = ["https://localhost:18541"]
tls_ca_file = "$WORK/cert.pem"

[[upstream]]
name = "tcp-only"
protocol = "h3"
backends = ["https://localhost:18542"]
tls_ca_file = "$WORK/cert.pem"

[[route]]
[route.match]
path = "/hello.txt"
[route.action]
type = "proxy"
upstream = "quic"

[[route]]
[route.match]
path = "/tcp/*"
[route.action]
type = "proxy"
upstream = "tcp-only"
connect_timeout_ms = 1000
TOML
(cd proxy && "$RWS" --check >../check.log 2>&1) || { cat check.log; fail "rws --check rejected the proxy config"; }
(cd proxy/www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../../proxy.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do
  curl -s -o /dev/null --cacert cert.pem https://localhost:18541/hello.txt && curl -s -o /dev/null http://127.0.0.1:18540/healthz && break
  sleep 0.1
done

body=$(curl -s http://127.0.0.1:18540/hello.txt)
[ "$body" = "from the h3 backend" ] || fail "proxy over QUIC answered: $body"
# The access log is flushed every 100 ms.
logged() { for _ in $(seq 1 30); do grep '"path":"/hello.txt"' backend.log | grep -q "$1" && return 0; sleep 0.1; done; return 1; }
logged '"protocol":"HTTP/3' || fail "the backend didn't see HTTP/3: $(grep hello.txt backend.log | tail -2)"
pass "proxied over HTTP/3: the backend logged the request as HTTP/3"

for i in 1 2 3; do curl -s -o /dev/null http://127.0.0.1:18540/hello.txt; done
for _ in $(seq 1 30); do [ "$(grep '"path":"/hello.txt"' backend.log | grep -c '"protocol":"HTTP/3')" -ge 4 ] && break; sleep 0.1; done
[ "$(grep '"path":"/hello.txt"' backend.log | grep -c '"protocol":"HTTP/3')" -ge 4 ] || fail "not every request reached the backend over HTTP/3"
grep -q "HTTP/3 to .*18541 failed" proxy.log && fail "the QUIC backend fell back to TCP"
pass "repeated requests all go over HTTP/3"

start=$(date +%s%N)
body=$(curl -s http://127.0.0.1:18540/tcp/x)
[ "$body" = "from the tcp backend" ] || fail "no TCP fallback: $body"
pass "a backend QUIC can't reach is served over TCP ($(( ($(date +%s%N) - start) / 1000000 )) ms, connect timeout 1000)"
start=$(date +%s%N)
body=$(curl -s http://127.0.0.1:18540/tcp/y)
elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
[ "$body" = "from the tcp backend" ] && [ "$elapsed" -lt 500 ] || fail "second request: $body in $elapsed ms"
curl -s http://127.0.0.1:18540/metrics | grep -q 'rws_upstream_h3_fallbacks_total{upstream="tcp-only"} 1$' || fail "fallback not counted once: $(curl -s http://127.0.0.1:18540/metrics | grep h3_fallbacks)"
pass "the fallback is remembered ($elapsed ms) and counted once"
echo "all HTTP/3 upstream end-to-end checks passed"
