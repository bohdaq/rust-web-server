# S7 AI: rws's MCP gateway, A2A gateway and LLM proxy routes (release image)
# in front of the test app's MCP server, A2A echo agent and mock LLM.
# Sourced by run.sh with lib.sh.

# A rerun on a kept project starts with a fresh token budget (valkey keeps
# the counts under the LLM route's prefix).
[ "${APP_READY:-0}" = 1 ] && k exec deploy/valkey -- sh -c "valkey-cli --scan --pattern 'rws:llm:*' | xargs -r valkey-cli del" >/dev/null 2>&1

MCP_H=(-H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream')
# MCP 2026-07-28 (stateless): no session, so any replica answers. Sessions
# (earlier versions) live in one replica's memory: S7.0 shows what that
# does behind the gateway.
MCP_META='"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{},"io.modelcontextprotocol/clientInfo":{"name":"stage-e2e","version":"1"}}'
mcp() { # <method> <params without _meta, or ""> [Mcp-Name] — the body
  local params=$MCP_META name=()
  [ -n "$2" ] && params="$2,$MCP_META"
  [ -n "${3:-}" ] && name=(-H "Mcp-Name: $3")
  curl -sS --max-time 20 -D "$LAST_HEADERS" "${MCP_H[@]}" -H "Authorization: Bearer $E2E_MCP_CALLER" -H 'MCP-Protocol-Version: 2026-07-28' \
    -H "Mcp-Method: $1" "${name[@]}" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":{$params}}" "$RWS_URL/e2e/mcp" | sed -n 's/^data: //p; /^{/p'
}
add() { mcp tools/call "\"name\":\"app__add\",\"arguments\":{\"a\":$1,\"b\":$2}" app__add; }

mcp_session_ok() { # initialize, then 6 more requests, each on its own connection
  local sid i ok=0
  curl -sS --max-time 20 -D "$LAST_HEADERS" -o /dev/null "${MCP_H[@]}" -H "Authorization: Bearer $E2E_MCP_CALLER" "$RWS_URL/e2e/mcp" \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"stage-e2e","version":"1"}}}'
  sid=$(header_of mcp-session-id)
  for i in $(seq 6); do
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "${MCP_H[@]}" -H "Authorization: Bearer $E2E_MCP_CALLER" -H "Mcp-Session-Id: $sid" \
      -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' "$RWS_URL/e2e/mcp")" = 200 ] && ok=$((ok + 1))
  done
  echo "    session kept on $ok of 6 connections" >&2
  [ "$ok" = 6 ]
}
app_check S7.0 "MCP session (2025-06-18) holds across rws's 3 replicas" mcp_session_ok
mcp_tools_ok() { # each replica lists the upstream's tools in the background (every
  # 10 s until it can, e.g. while the app restarts): 6 answers in a row, a
  # connection each, so every replica has.
  local out i run=0
  for i in $(seq 60); do
    out=$(mcp tools/list "")
    if grep -q '"name":"app__add"' <<<"$out"; then run=$((run + 1)); [ "$run" -ge 6 ] && break; else run=0; sleep 3; fi
  done
  echo "    tools: $(grep -o '"name":"[^"]*"' <<<"$out" | tr '\n' ' ')" >&2
  grep -q '"name":"app__add"' <<<"$out"
}
app_check S7.1 "MCP gateway (stateless): the app's tools listed as app__*" mcp_tools_ok
mcp_call_ok() { add 2 40 | grep -q '"text":"42"'; }
app_check S7.2 "MCP gateway: app__add(2, 40) = 42 through the gateway" mcp_call_ok
mcp_auth_ok() {
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "${MCP_H[@]}" -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$RWS_URL/e2e/mcp")" = 401 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "${MCP_H[@]}" -H 'Authorization: Bearer wrong' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$RWS_URL/e2e/mcp")" = 401 ]
}
app_check S7.3 "MCP gateway: no or a wrong caller token 401" mcp_auth_ok
mcp_limit_ok() { # 5 calls an hour per caller (S7.2 was one); the count is per replica
  local i limited=0
  for i in $(seq 14); do add 1 1 | grep -q '"text":"2"' || limited=$((limited + 1)); done
  echo "    14 more calls: $limited refused" >&2
  [ "$limited" -ge 1 ]
}
app_check S7.4 "MCP gateway: the per-caller tool limit refuses calls past it" mcp_limit_ok

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
