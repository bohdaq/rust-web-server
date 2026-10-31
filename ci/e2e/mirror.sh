#!/usr/bin/env bash
# Request mirroring from the config file ([route.middleware.mirror]),
# against the rws binary: copies reach the mirror, the share is honoured,
# and the client is unaffected when the mirror is slow or down.
#
#   ci/e2e/mirror.sh <path-to-rws>
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

# A backend that logs each request path to <port>.log; "slow" waits 3 s.
cat > backend.py <<'PY'
import http.server, sys, time
port, mode = int(sys.argv[1]), sys.argv[2]
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def handle_one(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        with open(f"{port}.log", "a") as f: f.write(f"{self.command} {self.path} {body.decode()}\n")
        if mode == "slow": time.sleep(3)
        out = f"from {port}".encode()
        self.send_response(200); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
    do_GET = do_POST = handle_one
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
PY
python3 backend.py 18501 fast & PIDS+=($!)
python3 backend.py 18502 fast & PIDS+=($!)
python3 backend.py 18503 slow & PIDS+=($!)

cat > rws.config.toml <<'TOML'
port = 18500

[[upstream]]
name = "app"
backends = ["http://127.0.0.1:18501"]

[[upstream]]
name = "shadow"
backends = ["http://127.0.0.1:18502"]

[[route]]
name = "all"
[route.match]
path = "/all/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.mirror]
upstream = "shadow"

[[route]]
name = "quarter"
[route.match]
path = "/quarter/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.mirror]
backends = ["http://127.0.0.1:18502"]
percent = 25

[[route]]
name = "slow"
[route.match]
path = "/slow/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.mirror]
backends = ["http://127.0.0.1:18503"]

[[route]]
name = "down"
[route.match]
path = "/down/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.mirror]
backends = ["http://127.0.0.1:18509"]
TOML

"$RWS" --check=rws.config.toml >check.log 2>&1 || { cat check.log; fail "rws --check rejected the mirror config"; }
pass "rws --check accepts the mirror config"

(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18500/all/ && break; sleep 0.1; done
: > 18501.log; : > 18502.log

out=$(curl -s -i -X POST -d payload http://127.0.0.1:18500/all/order); grep -q "from 18501" <<<"$out" || fail "the client didn't get the primary's answer: $out"
sleep 0.5
grep -q "POST /all/order payload" 18502.log || fail "the mirror didn't get the copy with its body: $(cat 18502.log)"
pass "a copy with its body reaches the mirror; the client gets the primary's answer"

for i in $(seq 1 8); do curl -s -o /dev/null "http://127.0.0.1:18500/quarter/$i"; done
sleep 0.5
n=$(grep -c "/quarter/" 18502.log || true)
[ "$n" = 2 ] || fail "25% of 8 should mirror 2, mirrored $n"
pass "percent = 25 mirrors 2 of 8"

start=$(date +%s%N)
for i in 1 2 3; do [ "$(curl -s --max-time 2 http://127.0.0.1:18500/slow/$i)" = "from 18501" ] || fail "a slow mirror delayed or broke the response"; done
ms=$(( ($(date +%s%N) - start) / 1000000 ))
[ "$ms" -lt 1500 ] || fail "three requests took ${ms} ms with a slow mirror"
pass "a slow mirror (3 s) doesn't delay responses (${ms} ms for 3)"

[ "$(curl -s --max-time 2 http://127.0.0.1:18500/down/x)" = "from 18501" ] || fail "a down mirror broke the response"
sleep 1
metrics=$(curl -s http://127.0.0.1:18500/metrics)
grep -q 'rws_mirror_requests_total{mirror="down-mirror",outcome="failed"} 1' <<<"$metrics" || fail "no failed count for the down mirror: $(grep mirror <<<"$metrics")"
grep -qE 'rws_mirror_requests_total\{mirror="shadow",outcome="sent"\} [1-9]' <<<"$metrics" || fail "no sent count: $(grep mirror <<<"$metrics")"
pass "a down mirror doesn't affect responses; rws_mirror_requests_total counts sent and failed"

echo "all mirroring end-to-end checks passed"
