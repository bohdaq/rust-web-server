#!/usr/bin/env bash
# Runtime state shared across replicas (#76), against the rws binary and a
# real Redis: two replicas of a proxy with [cluster] redis; a block, maintenance
# mode, a drain and a feature flag changed on one apply on the other within
# a couple of seconds; both nodes listed; a change made while Redis is down
# reaches the other replica once it's back; a third replica started later
# adopts the state.
#
#   ci/e2e/cluster.sh <path-to-rws> <redis-server | docker>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
REDIS="$2"
WORK="$(mktemp -d)"
PIDS=()
REDIS_PID=""
CONTAINER=""
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  [ -n "$REDIS_PID" ] && kill "$REDIS_PID" 2>/dev/null || true
  [ -n "$CONTAINER" ] && docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; grep -h "cluster\]" "$WORK"/rws-*.log 2>/dev/null | tail -20 || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

start_redis() {
  if [ "$REDIS" = docker ]; then
    CONTAINER=$(docker run -d --rm -p 127.0.0.1:18539:6379 redis:7-alpine)
  else
    "$REDIS" --port 18539 --save "" --appendonly no >redis.log 2>&1 & REDIS_PID=$!
  fi
  for _ in $(seq 1 100); do (exec 3<>/dev/tcp/127.0.0.1/18539) 2>/dev/null && return 0; sleep 0.1; done
  fail "Redis didn't start"
}
stop_redis() {
  if [ "$REDIS" = docker ]; then docker rm -f "$CONTAINER" >/dev/null; CONTAINER=""; else kill "$REDIS_PID"; wait "$REDIS_PID" 2>/dev/null || true; REDIS_PID=""; fi
}
start_redis

mkdir -p www backend && echo "from-backend" > backend/index.html
(cd backend && exec python3 -I -m http.server 18538 --bind 127.0.0.1 >/dev/null 2>&1) & PIDS+=($!)
export RWS_ADMIN_TOKEN="cluster-e2e-token-$RANDOM"
replica() { # <name> <port> <admin port>
  cat > "$1.toml" <<TOML
port = $2
[admin]
listen = "127.0.0.1:$3"
[cluster]
redis = "127.0.0.1:18539"
node  = "$1"
[[upstream]]
name = "web"
backends = ["127.0.0.1:18538", "127.0.0.1:18537"]
[[route]]
[route.match]
path = "/*"
[route.action]
type = "proxy"
upstream = "web"
TOML
  "$RWS" --check="$1.toml" >"check-$1.log" 2>&1 || fail "rws --check $1: $(cat "check-$1.log")"
  (cd www && exec env RWS_CONFIG_FILE="../$1.toml" "$RWS" >"../rws-$1.log" 2>&1) & PIDS+=($!)
  for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$3/admin/api/v1/auth" && return 0; sleep 0.1; done
  fail "replica $1 didn't start"
}
replica web-1 18531 18541
replica web-2 18532 18542
ctl1() { "$RWS" ctl --admin "127.0.0.1:18541" "$@"; }
ctl2() { "$RWS" ctl --admin "127.0.0.1:18542" "$@"; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
eventually() { # <seconds> <shell condition>: re-evaluated until it holds
  local until=$((SECONDS + $1))
  until eval "$2"; do [ $SECONDS -lt $until ] || return 1; sleep 0.2; done
}

sleep 6   # one heartbeat each
eventually 6 'ctl1 status | grep -q "cluster   node web-1 · Redis up · 2 node(s): web-1, web-2"' || fail "status: $(ctl1 status)"
pass "both replicas sync through Redis and see each other"

ctl1 block 127.0.0.1 >/dev/null
eventually 3 '[ "$(code http://127.0.0.1:18532/)" = 403 ]' || fail "a block on web-1 didn't reach web-2"
ctl2 unblock 127.0.0.1 >/dev/null
eventually 3 '[ "$(code http://127.0.0.1:18531/)" = 200 ]' || fail "an unblock on web-2 didn't reach web-1"
pass "block / unblock on one replica applies on the other"

ctl2 maintenance on >/dev/null
eventually 3 '[ "$(code http://127.0.0.1:18531/)" = 503 ]' || fail "maintenance on web-2 didn't reach web-1"
ctl1 maintenance off >/dev/null
eventually 3 '[ "$(code http://127.0.0.1:18532/)" = 200 ]' || fail "maintenance off didn't reach web-2"
pass "maintenance mode follows"

ctl1 drain web 127.0.0.1:18537 >/dev/null || fail "drain on web-1: $(ctl1 drain web 127.0.0.1:18537 2>&1)"
eventually 3 'ctl2 --json upstreams | grep -q "\"url\":\"127.0.0.1:18537\",[^}]*\"drained\":true"' || fail "the drain didn't reach web-2: $(ctl2 --json upstreams)"
pass "drained backends follow"

curl -s -X PUT -H "Authorization: Bearer $RWS_ADMIN_TOKEN" -d '{"enabled":true}' http://127.0.0.1:18541/admin/api/v1/flags/new-checkout >/dev/null
eventually 3 'curl -s -H "Authorization: Bearer $RWS_ADMIN_TOKEN" http://127.0.0.1:18542/admin/api/v1/flags | grep -q "\"name\":\"new-checkout\",\"enabled\":true"' || fail "the flag didn't reach web-2"
pass "feature flags follow"

stop_redis
ctl1 block 192.0.2.50 >/dev/null
sleep 2
ctl1 status | grep -q "Redis down, " && true
ctl1 status | grep -q "change(s) waiting" || fail "web-1 should hold the change while Redis is down: $(ctl1 status)"
start_redis
eventually 8 'ctl2 blocklist | grep -qx 192.0.2.50' || fail "the change made while Redis was down never reached web-2"
ctl2 --json upstreams | grep -q '"url":"127.0.0.1:18537",[^}]*"drained":true' || fail "a Redis that lost its data undid the drain on web-2"
pass "Redis restarted empty: the state is seeded back from the replicas, the change made meanwhile arrives, nothing is undone"

replica web-3 18533 18543
eventually 4 '"$RWS" ctl --admin 127.0.0.1:18543 blocklist | grep -qx 192.0.2.50' || fail "a new replica didn't adopt the blocklist"
pass "a replica started later adopts the shared state"

grep -q '"node":"web-1"' <<<"$(ctl1 --json audit)" || fail "audit entries don't name the node"
pass "audit entries name the replica"

echo "all cluster end-to-end checks passed"
