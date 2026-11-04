# S6 realtime: WebSocket through the gateway and an rws proxy route to an
# echo backend (manifests/services.yaml). Not here yet (the test app):
# SSE, realtime channels, fan-out across replicas. Sourced by run.sh.

WS_URL="wss://${RWS_URL#https://}"
ws_ok() { node "$STAGE_DIR/ws_client.mjs" "$WS_URL/e2e/ws/echo" s6-hello >&2; }
check S6.1 "WebSocket upgrade through the gateway and a proxy route: echo" ws_ok
ws_idle_ok() { node "$STAGE_DIR/ws_client.mjs" "$WS_URL/e2e/ws/echo" s6-idle 45 >&2; }
check S6.2 "a WebSocket idle 45 s still echoes" ws_idle_ok
ws_many_ok() { # 9 connections: each pod relays some (the gateway spreads them)
  local i ok=0
  for i in $(seq 9); do node "$STAGE_DIR/ws_client.mjs" "$WS_URL/e2e/ws/echo" "s6-many$i" >/dev/null 2>&1 && ok=$((ok + 1)); done
  echo "    $ok of 9 round trips" >&2
  [ "$ok" = 9 ]
}
check S6.3 "9 WebSocket connections in a row, all echo" ws_many_ok
