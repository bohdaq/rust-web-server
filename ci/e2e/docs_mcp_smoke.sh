#!/usr/bin/env bash
# Smoke test of the docs MCP server from outside: one session against
# <url> (default https://rws8.tech/mcp) calling every tool, plus the
# stateless 2026-07-28 protocol. Workflow docs-mcp-smoke.yml runs it.
#   ci/e2e/docs_mcp_smoke.sh [url]
set -euo pipefail
URL=${1:-https://rws8.tech/mcp}
H=(-H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' -H 'MCP-Protocol-Version: 2025-11-25')
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }

SID=$(curl -sS -m 20 -D - -o /dev/null "${H[@]}" -X POST "$URL" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"docs-mcp-smoke","version":"1"}}}' \
  | tr -d '\r' | awk -F': ' 'tolower($1)=="mcp-session-id"{print $2}')
[ -n "$SID" ] || fail "initialize returned no Mcp-Session-Id"
pass "initialize (session ${SID:0:8}…)"
curl -sS -m 20 "${H[@]}" -H "Mcp-Session-Id: $SID" -X POST "$URL" -d '{"jsonrpc":"2.0","method":"notifications/initialized"}' -o /dev/null

# call <tool> <arguments json> <text the answer must contain>
call() {
  local out
  out=$(curl -sS -m 30 "${H[@]}" -H "Mcp-Session-Id: $SID" -X POST "$URL" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}")
  python3 -c 'import json,sys; d=json.loads(sys.argv[1]); r=d.get("result") or {}; sys.exit(0 if sys.argv[2] in "".join(c.get("text","") for c in r.get("content",[])) else 1)' "$out" "$3" \
    || fail "$1 $2: expected \"$3\" in: ${out:0:400}"
  pass "$1"
}

TOOLS=$(curl -sS -m 20 "${H[@]}" -H "Mcp-Session-Id: $SID" -X POST "$URL" -d '{"jsonrpc":"2.0","id":3,"method":"tools/list"}' \
  | python3 -c 'import json,sys; print(" ".join(sorted(t["name"] for t in json.load(sys.stdin)["result"]["tools"])))')
[ "$TOOLS" = "api_item changes check_config get_example get_page search_docs setting versions" ] || fail "tools/list: $TOOLS"
pass "tools/list: $TOOLS"

call versions '{}' 'latest is'
call search_docs '{"query":"add a route with a path parameter and test it","limit":5}' 'building-apps/routing'
call get_page '{"path":"building-apps/routing","version":"18.7"}' 'rws 18.7'
call setting '{"name":"[acme] domains"}' 'RWS_CONFIG_ACME_DOMAINS'
call api_item '{"name":"TestClient"}' 'docs.rs/rust-web-server'
call get_example '{"name":"rest_api"}' 'cargo run --example rest_api'
call check_config '{"toml":"[[upstream]]\nname = \"app\"\nbackends = [\"10.0.0.1:80\"]\nstrategy = \"fastest\"\n"}' 'unknown strategy'
call changes '{"since":"18.8.0"}' '18.8.3'

STATELESS=$(curl -sS -m 20 -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' -X POST "$URL" \
  -d '{"jsonrpc":"2.0","id":5,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"docs-mcp-smoke","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}}}')
grep -q '"search_docs"' <<<"$STATELESS" || fail "2026-07-28 tools/list: ${STATELESS:0:300}"
pass "2026-07-28 stateless tools/list"

CODE=$(curl -sS -m 20 -o /dev/null -w '%{http_code}' "${H[@]}" -H 'Origin: https://evil.example' -X POST "$URL" -d '{"jsonrpc":"2.0","id":1,"method":"ping"}')
[ "$CODE" = 403 ] || fail "a foreign Origin got $CODE, expected 403"
pass "foreign Origin refused (403)"
echo "docs MCP smoke test passed: $URL"
