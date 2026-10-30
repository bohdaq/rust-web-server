#!/usr/bin/env bash
# A WebAssembly plugin from the config file ([route.middleware.wasm]),
# against the rws binary built with --features wasm-plugins: the example
# plugin (examples/wasm_plugin) tags requests, answers one itself, stamps
# responses with its config; a missing module fails closed; metrics.
#
#   ci/e2e/wasm_plugin.sh <path-to-rws> <path-to-plugin.wasm>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
PLUGIN="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
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
mkdir www

# A backend that answers with the request headers it got.
cat > backend.py <<'PY'
import http.server
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = "".join(f"{k.lower()}: {v}\n" for k, v in self.headers.items()).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 18491), H).serve_forever()
PY
python3 backend.py & PIDS+=($!)

cat > rws.config.toml <<EOF2
port = 18490

[[upstream]]
name = "app"
backends = ["http://127.0.0.1:18491"]

[[route]]
  name = "plugged"
  [route.match]
  path = "/app/*"
  [route.action]
  type = "proxy"
  upstream = "app"
  [route.middleware.wasm]
  module = "$PLUGIN"
  fuel = 5000000
  memory_mb = 8
  config = "from-the-config-file"

[[route]]
  name = "broken"
  [route.match]
  path = "/broken/*"
  [route.action]
  type = "proxy"
  upstream = "app"
  [route.middleware.wasm]
  module = "$WORK/missing.wasm"
EOF2

! "$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check accepted a missing module"
grep -q "missing.wasm" check.log || fail "rws --check didn't name the missing module: $(cat check.log)"
pass "rws --check reports a plugin that can't be loaded"

(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18490/app/ && break; sleep 0.1; done

out=$(curl -s -D headers.txt http://127.0.0.1:18490/app/x)
grep -q "^x-plugin: rws-example" <<<"$out" || fail "the backend didn't get the plugin's header: $out"
grep -qi "^x-plugin-config: from-the-config-file" headers.txt || fail "the response hook didn't run with the config: $(cat headers.txt)"
pass "request header added, response stamped with the plugin's config"

[ "$(curl -s -o body.txt -w '%{http_code}' -H 'X-Block: yes' http://127.0.0.1:18490/app/x)" = 403 ] || fail "the plugin didn't answer the blocked request"
grep -q "blocked by the example plugin" body.txt || fail "the plugin's body: $(cat body.txt)"
pass "the plugin answers a request itself"

[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18490/broken/x)" = 503 ] || fail "a route with a missing plugin didn't fail closed"
pass "a route whose plugin can't load answers 503"

metrics=$(curl -s http://127.0.0.1:18490/metrics)
grep -q 'rws_wasm_plugin_calls_total{plugin="rws_wasm_plugin_example",hook="request"} [3-9]' <<<"$metrics" || fail "no plugin metrics: $(grep wasm <<<"$metrics")"
pass "rws_wasm_plugin_calls_total"

echo "all WebAssembly plugin end-to-end checks passed"
