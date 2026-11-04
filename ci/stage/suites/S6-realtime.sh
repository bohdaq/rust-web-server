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

# SSE from the test app through the gateway: events arrive as they're sent,
# not buffered until the end.
sse_ok() {
  local out first last
  out=$(curl -sS -N --max-time 30 "$APP_URL/app/sse?count=5&every_ms=1000" | while IFS= read -r line; do printf '%s %s\n' "$(date +%s%3N)" "$line"; done)
  [ "$(grep -c ' data: {"n":' <<<"$out")" = 5 ] || { echo "    $(grep -c data: <<<"$out") events" >&2; return 1; }
  first=$(grep ' data: {"n":1}' <<<"$out" | cut -d' ' -f1); last=$(grep ' data: {"n":5}' <<<"$out" | cut -d' ' -f1)
  echo "    first → last event: $((last - first)) ms" >&2
  [ $((last - first)) -ge 3000 ]
}
app_check S6.4 "SSE through the gateway: 5 events, 1 s apart, not buffered" sse_ok
# The kube-dc gateway ends a stream that has sent nothing for ~15 s (found
# 2026-11-04: 16 s through it, the full 40 s straight to the app): an event
# every 10 s, for 2 minutes.
sse_long_ok() {
  local n; n=$(curl -sS -N --max-time 150 "$APP_URL/app/sse?count=12&every_ms=10000" | grep -c '^data: ')
  echo "    $n of 12 events over 2 min" >&2
  [ "$n" = 12 ]
}
app_check S6.5 "an SSE stream with an event every 10 s lasts 2 min" sse_long_ok
