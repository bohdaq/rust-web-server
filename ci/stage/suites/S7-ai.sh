# S7 AI: rws's MCP gateway, A2A gateway and LLM proxy routes (release image)
# in front of the test app's MCP server, A2A echo agent and mock LLM.
# Sourced by run.sh with lib.sh.

MCP_H=(-H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream')
mcp() { # <session id or -> <json> — the body (SSE data unwrapped)
  local sid=() ; [ "$1" != - ] && sid=(-H "Mcp-Session-Id: $1")
  curl -sS --max-time 20 -D "$LAST_HEADERS" "${MCP_H[@]}" -H "Authorization: Bearer $E2E_MCP_CALLER" "${sid[@]}" -d "$2" "$RWS_URL/e2e/mcp" | sed -n 's/^data: //p; /^{/p'
}
mcp_init() {
  mcp - '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"stage-e2e","version":"1"}}}' >/dev/null
  MCP_SID=$(header_of mcp-session-id)
  [ -n "$MCP_SID" ] && mcp "$MCP_SID" '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null
}
mcp_tools_ok() {
  eventually 90 mcp_init || return 1
  local out; out=$(mcp "$MCP_SID" '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')
  echo "    tools: $(grep -o '"name":"[^"]*"' <<<"$out" | tr '\n' ' ')" >&2
  grep -q '"name":"app__add"' <<<"$out"
}
app_check S7.1 "MCP gateway: the app's tools listed as app__*" mcp_tools_ok
mcp_call_ok() {
  mcp "$MCP_SID" '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"app__add","arguments":{"a":2,"b":40}}}' | grep -q '"text":"42"'
}
app_check S7.2 "MCP gateway: app__add(2, 40) = 42 through the gateway" mcp_call_ok
mcp_auth_ok() {
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "${MCP_H[@]}" -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$RWS_URL/e2e/mcp")" = 401 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "${MCP_H[@]}" -H 'Authorization: Bearer wrong' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$RWS_URL/e2e/mcp")" = 401 ]
}
app_check S7.3 "MCP gateway: no or a wrong caller token 401" mcp_auth_ok
mcp_limit_ok() { # 5 calls an hour per caller (S7.2 was one), on every replica
  local i out limited=0
  for i in $(seq 8); do
    out=$(mcp "$MCP_SID" "{\"jsonrpc\":\"2.0\",\"id\":$((10 + i)),\"method\":\"tools/call\",\"params\":{\"name\":\"app__add\",\"arguments\":{\"a\":1,\"b\":1}}}")
    grep -q '"text":"2"' <<<"$out" || limited=$((limited + 1))
  done
  echo "    8 more calls: $limited refused" >&2
  [ "$limited" -ge 4 ]
}
app_check S7.4 "MCP gateway: the per-caller tool limit (5 an hour) refuses the rest" mcp_limit_ok

a2a_card_ok() {
  local card; card=$(curl -sS --max-time 20 "$RWS_URL/e2e/a2a/.well-known/agent-card.json")
  python3 -I -c '
import json, sys
c = json.loads(sys.argv[1]); urls = [i["url"] for i in c["supportedInterfaces"]]
assert urls == [sys.argv[2] + "/e2e/a2a"] or all(u.startswith(sys.argv[2]) for u in urls), urls
' "$card" "$RWS_URL" || { echo "    $card" >&2; return 1; }
}
app_check S7.5 "A2A gateway: the agent's card, its interface pointing at the route" a2a_card_ok
a2a_send_ok() {
  curl -sS --max-time 20 -H 'A2A-Version: 1.0' -H 'Content-Type: application/json' "$RWS_URL/e2e/a2a/" \
    -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"stage-ping"}]}}}' | grep -q '"text":"stage-ping"'
}
app_check S7.6 "A2A gateway: SendMessage reaches the agent and its answer comes back" a2a_send_ok

llm() { curl -sS --max-time 30 -H "Authorization: Bearer $1" -H 'Content-Type: application/json' "$RWS_URL/v1/chat/completions" -d "$2" -w '\n%{http_code}'; }
llm_ok() {
  local out; out=$(llm "$E2E_LLM_KEY" '{"model":"mock-1","messages":[{"role":"user","content":"hi"}]}')
  [ "$(tail -1 <<<"$out")" = 200 ] && head -1 <<<"$out" | grep -q '"content":"echo: hi"'
}
app_check S7.7 "LLM proxy: a completion through the virtual key" llm_ok
llm_stream_ok() {
  local out; out=$(llm "$E2E_LLM_KEY" '{"model":"mock-1","stream":true,"messages":[{"role":"user","content":"hi"}]}')
  grep -q '^data: \[DONE\]' <<<"$out" && [ "$(tail -1 <<<"$out")" = 200 ]
}
app_check S7.8 "LLM proxy: a streamed completion ends with [DONE]" llm_stream_ok
llm_key_ok() { [ "$(llm wrong-key '{"messages":[{"role":"user","content":"hi"}]}' | tail -1)" = 401 ]; }
app_check S7.9 "LLM proxy: an unknown virtual key 401" llm_key_ok
llm_budget_ok() { # 40 tokens an hour shared through valkey; 30 used so far
  local i codes=""
  for i in 1 2 3; do codes+="$(llm "$E2E_LLM_KEY" '{"messages":[{"role":"user","content":"hi"}]}' | tail -1) "; done
  echo "    3 more requests: $codes" >&2
  grep -q 429 <<<"$codes"
}
app_check S7.10 "LLM proxy: over the token budget (shared by 3 replicas) → 429" llm_budget_ok
llm_metrics_ok() { [ "$(metric_sum 'rws_llm_tokens_total{')" -gt 0 ]; }
app_check S7.11 "rws_llm_tokens_total counts the tokens" llm_metrics_ok
