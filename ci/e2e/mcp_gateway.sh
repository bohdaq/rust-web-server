#!/usr/bin/env bash
# The MCP gateway from the config file, against the rws binary: two upstream
# MCP servers (Python stand-ins — one speaking 2025-11-25 sessions, one only
# 2026-07-28), prefixed tools, per-caller ACLs, a per-tool rate limit, the
# audit log, and upstream credentials read from the environment.
#
#   ci/e2e/mcp_gateway.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -40 "$WORK/rws.log" 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

# A minimal MCP server: tools/list and tools/call for its tools; "legacy"
# mode needs initialize + a session, "modern" mode refuses initialize and
# answers server/discover and stateless requests. It records the
# Authorization header of each tools/call in calls.log.
cat > upstream.py <<'PY'
import http.server, json, sys
port, mode, tools = int(sys.argv[1]), sys.argv[2], sys.argv[3].split(",")
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def reply(self, status, obj, headers=()):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        for k, v in headers: self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        msg = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        method, rid = msg.get("method"), msg.get("id")
        if method == "initialize":
            if mode == "modern":
                return self.reply(400, {"jsonrpc": "2.0", "id": rid, "error": {"code": -32022, "message": "Unsupported protocol version"}})
            return self.reply(200, {"jsonrpc": "2.0", "id": rid, "result": {"protocolVersion": "2025-11-25", "capabilities": {"tools": {}}, "serverInfo": {"name": "u", "version": "1"}}}, [("Mcp-Session-Id", "s1")])
        if rid is None:
            self.send_response(202); self.send_header("Content-Length", "0"); self.end_headers(); return
        if mode == "legacy" and self.headers.get("Mcp-Session-Id") != "s1":
            return self.reply(404, {"jsonrpc": "2.0", "id": rid, "error": {"code": -32001, "message": "Session not found"}})
        if mode == "modern" and self.headers.get("MCP-Protocol-Version") != "2026-07-28":
            return self.reply(400, {"jsonrpc": "2.0", "id": rid, "error": {"code": -32020, "message": "Header mismatch"}})
        if method == "server/discover":
            return self.reply(200, {"jsonrpc": "2.0", "id": rid, "result": {"supportedVersions": ["2026-07-28"], "capabilities": {"tools": {}}}})
        if method == "tools/list":
            return self.reply(200, {"jsonrpc": "2.0", "id": rid, "result": {"tools": [{"name": t, "inputSchema": {"type": "object"}} for t in tools]}})
        if method == "tools/call":
            with open(f"calls-{port}.log", "a") as f: f.write((self.headers.get("Authorization") or "-") + "\n")
            return self.reply(200, {"jsonrpc": "2.0", "id": rid, "result": {"content": [{"type": "text", "text": f"{msg['params']['name']}@{port}"}]}})
        self.reply(200, {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "Method not found"}})
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
PY
python3 upstream.py 9431 legacy search,delete & PIDS+=($!)
python3 upstream.py 9432 modern forecast & PIDS+=($!)

export GITHUB_UPSTREAM_AUTH="Bearer upstream-secret"
export ALICE_TOKEN="alice-token" BOB_TOKEN="bob-token"
cat > rws.config.toml <<'EOF'
port = 18431

[[route]]
  name = "gateway"
  [route.match]
  path = "/mcp"
  [route.action]
  type = "mcp"
  [route.action.mcp]
  refresh_secs = 0
  audit_log = true

  [[route.action.mcp.upstream]]
  prefix = "docs"
  url = "http://127.0.0.1:9431/mcp"
  headers_env = { Authorization = "GITHUB_UPSTREAM_AUTH" }

  [[route.action.mcp.upstream]]
  prefix = "weather"
  url = "http://127.0.0.1:9432/mcp"

  [[route.action.mcp.caller]]
  identity = "alice"
  token_env = "ALICE_TOKEN"
  [[route.action.mcp.caller]]
  identity = "bob"
  token_env = "BOB_TOKEN"

  [[route.action.mcp.allow]]
  identity = "alice"
  tools = ["*"]
  [[route.action.mcp.allow]]
  identity = "bob"
  tools = ["docs__search", "weather__*"]

  [[route.action.mcp.limit]]
  tools = "docs__search"
  max_calls = 2
  window_secs = 60
EOF

"$RWS" --check=rws.config.toml >check.log 2>&1 || { cat check.log; fail "rws --check rejected the gateway config"; }
pass "rws --check accepts the gateway config"

RWS_CONFIG_FILE=rws.config.toml "$RWS" >rws.log 2>&1 & PIDS+=($!)

mcp() { # <token> <json body> → response body
  curl -s --max-time 10 -H "Authorization: Bearer $1" -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' -H 'MCP-Protocol-Version: 2025-11-25' \
    ${SESSION:+-H "Mcp-Session-Id: $SESSION"} -X POST http://127.0.0.1:18431/mcp -d "$2"
}
session() { # <token> → Mcp-Session-Id
  curl -s --max-time 10 -D - -o /dev/null -H "Authorization: Bearer $1" -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' -X POST http://127.0.0.1:18431/mcp \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"e2e","version":"1"}}}' \
    | tr -d '\r' | awk -F': ' 'tolower($1)=="mcp-session-id"{print $2}'
}
call() { echo "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":{}}}"; }
LIST='{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}'

# The gateway lists its upstreams in the background: wait for it.
for _ in $(seq 1 100); do
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer alice-token" -H 'Content-Type: application/json' http://127.0.0.1:18431/mcp -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' || true)
  [ "$code" != "503" ] && [ "$code" != "000" ] && break
  sleep 0.2
done
grep -q "mcp gateway at /mcp: 2 upstream(s) federated" rws.log || fail "the gateway never came up"
pass "gateway built from the config file"

SESSION=$(session alice-token)
[ -n "$SESSION" ] || fail "alice got no session"
alice=$(mcp alice-token "$LIST")
for t in docs__search docs__delete weather__forecast; do grep -q "\"$t\"" <<<"$alice" || fail "alice doesn't see $t: $alice"; done
pass "two upstreams federated with prefixes (one 2025, one 2026-only)"

out=$(mcp alice-token "$(call weather__forecast)"); grep -q "forecast@9432" <<<"$out" || fail "a call through to the 2026-only upstream failed: $out"
pass "a 2025 client calls a tool on the 2026-only upstream"

SESSION=$(session bob-token)
bob=$(mcp bob-token "$LIST")
grep -q '"docs__search"' <<<"$bob" && ! grep -q '"docs__delete"' <<<"$bob" || fail "bob's tools are wrong: $bob"
grep -q "Unknown tool" <<<"$(mcp bob-token "$(call docs__delete)")" || fail "bob called docs__delete"
pass "per-caller ACLs"

mcp bob-token "$(call docs__search)" >/dev/null
mcp bob-token "$(call docs__search)" >/dev/null
grep -q "Rate limited" <<<"$(mcp bob-token "$(call docs__search)")" || fail "bob's third search was not limited"
pass "per-tool rate limit"

[ "$(sort -u calls-9431.log)" = "Bearer upstream-secret" ] || fail "the docs upstream got $(sort -u calls-9431.log | tr '\n' ' ')"
pass "upstream credential from the environment; callers' tokens never forwarded"

[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Authorization: Bearer eve' -H 'Content-Type: application/json' http://127.0.0.1:18431/mcp -d "$LIST")" = 401 ] || fail "an unknown token was let in"
pass "unknown callers get 401"

sleep 0.3 # the access log flushes every 100 ms
grep '"event":"mcp_tool_call"' rws.log | grep -q '"sub":"bob","tool":"docs__search","upstream":"docs","outcome":"refused"' || fail "no audit line for bob's refused search"
grep '"event":"mcp_tool_call"' rws.log | grep -q '"sub":"bob","tool":"docs__delete","upstream":"docs","outcome":"denied"' || fail "no audit line for bob's denied call"
pass "audit log"

echo "all MCP gateway end-to-end checks passed"
