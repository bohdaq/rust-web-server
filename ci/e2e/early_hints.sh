#!/usr/bin/env bash
# 103 Early Hints against the rws binary with curl: hints from a config
# route's `early_hints` and from a backend's own 103, over h2c (prior
# knowledge) and over HTTP/1.1 only when early_hints_http1 is on.
# (HTTP/3 is covered by the unit tests: this curl has no QUIC.)
#
#   ci/e2e/early_hints.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -30 "$WORK"/rws.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www

# A backend that sends a 103 with one Link, then the answer.
cat > backend.py <<'PY'
import socketserver, sys
class H(socketserver.StreamRequestHandler):
    def handle(self):
        while True:
            line = self.rfile.readline()
            if not line:
                return
            while self.rfile.readline() not in (b"\r\n", b""):
                pass
            self.wfile.write(b"HTTP/1.1 103 Early Hints\r\nLink: </backend.css>; rel=preload; as=style\r\n\r\n")
            self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nbackend")
class S(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True
S(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
python3 backend.py 18521 & PIDS+=($!)

start() {
  cat > rws.config.toml <<TOML
port = 18520
early_hints_http1 = $1

[[upstream]]
name = "app"
backends = ["http://127.0.0.1:18521"]

[[route]]
name = "page"
[route.match]
path = "/page"
[route.action]
type = "respond"
body = "page"
[route.middleware]
early_hints = ["</page.css>; rel=preload; as=style", "<https://cdn.example.com>; rel=preconnect"]

[[route]]
name = "api"
[route.match]
path = "/api/*"
[route.action]
type = "proxy"
upstream = "app"
TOML
  "$RWS" --check >check.log 2>&1 || { cat check.log; fail "rws --check rejected the config"; }
  (cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
  for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18520/page && return; sleep 0.1; done
  fail "rws didn't start"
}
stop() { kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true; unset 'PIDS[-1]'; }

start false
out=$(curl -sv --http2-prior-knowledge http://127.0.0.1:18520/page 2>&1)
echo "$out" | grep -q "< HTTP/2 103" || fail "no 103 over h2c from the config route: $out"
echo "$out" | grep -qi "< link: </page.css>; rel=preload; as=style" || fail "config Link missing: $out"
echo "$out" | grep -qi "< link: <https://cdn.example.com>; rel=preconnect" || fail "second config Link missing: $out"
echo "$out" | grep -q "< HTTP/2 200" || fail "no final 200: $out"
pass "config route: 103 then 200 over h2c"

out=$(curl -sv --http2-prior-knowledge http://127.0.0.1:18520/api/x 2>&1)
echo "$out" | grep -qi "< link: </backend.css>; rel=preload; as=style" || fail "backend's 103 not passed on: $out"
echo "$out" | tail -1 | grep -q "backend" || fail "proxied body missing: $out"
pass "proxy route: the backend's 103 is passed on over h2c"

out=$(curl -sv --http1.1 http://127.0.0.1:18520/page 2>&1)
echo "$out" | grep -q "103" && fail "a 103 over HTTP/1.1 with early_hints_http1 off: $out"
pass "HTTP/1.1 gets no 103 by default"
curl -s http://127.0.0.1:18520/metrics | grep -q 'rws_early_hints_total{source="upstream"} [1-9]' || fail "upstream hints not counted"
pass "rws_early_hints_total counts them"
stop

start true
out=$(curl -sv --http1.1 http://127.0.0.1:18520/page 2>&1)
echo "$out" | grep -q "< HTTP/1.1 103 Early Hints" || fail "no 103 over HTTP/1.1 with early_hints_http1 on: $out"
echo "$out" | grep -q "< HTTP/1.1 200" || fail "no final 200 over HTTP/1.1: $out"
out=$(curl -sv --http1.1 http://127.0.0.1:18520/api/y 2>&1)
echo "$out" | grep -qi "< link: </backend.css>" || fail "backend's 103 not passed on over HTTP/1.1: $out"
pass "HTTP/1.1 with early_hints_http1 = true: config and backend hints"
echo "all early hints end-to-end checks passed"
