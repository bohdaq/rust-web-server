#!/usr/bin/env bash
# The admin API on its own listener, against the rws binary: off without
# [admin] listen, refused to start without a token, bearer auth, the status
# endpoint, browser origin checks, and `rws --check`. Grows with each 18.10
# admin tranche.
#
#   ci/e2e/admin.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -30 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www && echo hello > www/index.html

wait_for() { # <url>
  for _ in $(seq 1 100); do curl -s -o /dev/null "$1" && return 0; sleep 0.1; done
  return 1
}

cat > rws.config.toml <<'TOML'
port = 18451
[admin]
listen = "127.0.0.1:18452"
token_env = "E2E_ADMIN_TOKEN"
allowed_origins = ["https://ops.example.com"]
TOML

unset E2E_ADMIN_TOKEN
! "$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check passed without the token"
grep -q "E2E_ADMIN_TOKEN, which is unset" check.log || fail "rws --check didn't name the token variable: $(cat check.log)"
pass "rws --check: an admin listener needs its token"

(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws-notoken.log 2>&1) & PIDS+=($!)
wait_for http://127.0.0.1:18451/ || fail "the server didn't start without the admin token"
grep -q "admin API not started" rws-notoken.log || fail "no reason logged for the missing admin listener"
! curl -s -o /dev/null http://127.0.0.1:18452/ || fail "the admin port answered without a token"
pass "without a token the admin listener stays off; the server still serves"
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true

export E2E_ADMIN_TOKEN="e2e-$RANDOM-token"
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check rejected the admin config: $(cat check.log)"
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
wait_for http://127.0.0.1:18452/ || fail "the admin listener didn't start"
grep -q "admin API at http://127.0.0.1:18452/admin/api/v1/" rws.log || fail "no admin startup line"

code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
[ "$(code http://127.0.0.1:18452/admin/api/v1/status)" = 401 ] || fail "no token was let in"
[ "$(code -H 'Authorization: Bearer wrong' http://127.0.0.1:18452/admin/api/v1/status)" = 401 ] || fail "a wrong token was let in"
pass "bearer token required"

status=$(curl -s -H "Authorization: Bearer $E2E_ADMIN_TOKEN" http://127.0.0.1:18452/admin/api/v1/status)
python3 - "$status" <<'PY' || fail "bad status: $status"
import json, sys
s = json.loads(sys.argv[1])
assert s["ready"] is True, s
assert s["listeners"]["main"]["address"].endswith(":18451"), s
assert s["listeners"]["admin"] == "127.0.0.1:18452", s
assert s["version"] and s["features"], s
PY
pass "status: version, readiness, listeners"

[ "$(code -H "Authorization: Bearer $E2E_ADMIN_TOKEN" -H 'Origin: https://evil.example' http://127.0.0.1:18452/admin/api/v1/status)" = 403 ] || fail "another origin was let in"
[ "$(code -H "Authorization: Bearer $E2E_ADMIN_TOKEN" -H 'Origin: https://ops.example.com' http://127.0.0.1:18452/admin/api/v1/status)" = 200 ] || fail "an allowed origin was refused"
pass "browser origins checked"

[ "$(code http://127.0.0.1:18451/admin/api/v1/status)" = 404 ] || fail "the admin API leaked onto the public port"
pass "the public port doesn't serve the admin API"

api() { # <method> <path> [json body] → body
  curl -s --max-time 15 -X "$1" -H "Authorization: Bearer $E2E_ADMIN_TOKEN" -H 'Content-Type: application/json' \
    ${3:+-d "$3"} "http://127.0.0.1:18452/admin/api/v1$2"
}

cfg=$(api GET /config)
grep -q '"RWS_CONFIG_ADMIN_TOKEN_ENV":"(hidden)"' <<<"$cfg" && grep -q '"RWS_CONFIG_PORT":"18451"' <<<"$cfg" || fail "config: $cfg"
! grep -q "$E2E_ADMIN_TOKEN" <<<"$cfg" || fail "the token appears in /config"
pass "effective config, secrets hidden"

api PUT /maintenance '{"active":true}' >/dev/null
[ "$(code http://127.0.0.1:18451/)" = 503 ] || fail "maintenance mode didn't reach the public port"
api PUT /maintenance '{"active":false}' >/dev/null
[ "$(code http://127.0.0.1:18451/)" = 200 ] || fail "maintenance mode didn't switch off"
pass "maintenance mode on and off"

api POST /blocklist '{"ip":"127.0.0.1"}' >/dev/null
[ "$(code http://127.0.0.1:18451/)" = 403 ] || fail "the blocklist didn't apply"
[ "$(code -H "Authorization: Bearer $E2E_ADMIN_TOKEN" http://127.0.0.1:18452/admin/api/v1/status)" = 200 ] || fail "the blocklist locked out the admin API"
api DELETE /blocklist/127.0.0.1 >/dev/null
[ "$(code http://127.0.0.1:18451/)" = 200 ] || fail "unblocking didn't apply"
pass "blocklist (the admin listener isn't behind it)"

out=$(api POST /reload)
grep -q '"reloaded":true,"applied":true' <<<"$out" || fail "reload: $out"
sed -i 's/^port = 18451$/port = 18451\n[cors]\nallow_all = false\nallow_origins = "https:\/\/reloaded.example"/' rws.config.toml
out=$(api POST /reload)
grep -q '"applied":true' <<<"$out" || fail "second reload: $out"
grep -q 'reloaded.example' <<<"$(api GET /cors)" || fail "the reload didn't apply the new CORS origin: $(api GET /cors)"
kill -0 "${PIDS[-1]}" || fail "the server died on reload"
pass "reload through the SIGHUP path, applied and reported"

# ── upstreams: drain a backend of a config-file proxy ───────────────────────
kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
for port in 18461 18462; do
  mkdir -p "b$port" && echo "backend-$port" > "b$port/index.html"
  (cd "b$port" && exec python3 -I -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) & PIDS+=($!)
done
cat > proxy.toml <<'TOML'
port = 18451
[admin]
listen = "127.0.0.1:18452"
token_env = "E2E_ADMIN_TOKEN"

[[upstream]]
name = "web"
backends = ["http://127.0.0.1:18461", "http://127.0.0.1:18462"]
[upstream.health_check]
path = "/"
interval_secs = 1

[[route]]
[route.match]
path = "/"
[route.action]
type = "proxy"
upstream = "web"
TOML
(cd www && exec env RWS_CONFIG_FILE=../proxy.toml "$RWS" >../rws-proxy.log 2>&1) & PIDS+=($!)
wait_for http://127.0.0.1:18452/ || fail "the proxy-mode admin listener didn't start"
for _ in $(seq 1 50); do curl -s http://127.0.0.1:18451/ | grep -q backend && break; sleep 0.1; done

ups=$(api GET /upstreams)
python3 - "$ups" <<'PY' || fail "upstreams: $ups"
import json, sys
[web] = json.loads(sys.argv[1])
assert web["name"] == "web", web
assert [b["url"] for b in web["backends"]] == ["http://127.0.0.1:18461", "http://127.0.0.1:18462"], web
assert all(b["healthy"] and not b["drained"] and b["weight"] == 1 for b in web["backends"]), web
PY
pass "upstreams: backends, health, weights"

api PUT /upstreams/web/drain '{"backend":"http://127.0.0.1:18461","drained":true}' >/dev/null
for _ in $(seq 1 10); do
  [ "$(curl -s http://127.0.0.1:18451/)" = "backend-18462" ] || fail "a drained backend got a request"
done
grep -q '"url":"http://127.0.0.1:18461","weight":1,"healthy":true,"breaker":null,"drained":true' <<<"$(api GET /upstreams)" || fail "drain not shown"
api PUT /upstreams/web/drain '{"backend":"http://127.0.0.1:18462","drained":true}' >/dev/null
[ "$(code http://127.0.0.1:18451/)" = 503 ] || fail "everything drained should be 503"
api PUT /upstreams/web/drain '{"backend":"http://127.0.0.1:18461","drained":false}' >/dev/null
[ "$(curl -s http://127.0.0.1:18451/)" = "backend-18461" ] || fail "undrain didn't put the backend back"
[ "$(code -X PUT -H "Authorization: Bearer $E2E_ADMIN_TOKEN" -d '{"backend":"http://nope:1","drained":true}' http://127.0.0.1:18452/admin/api/v1/upstreams/web/drain)" = 404 ] || fail "an unknown backend was accepted"
pass "drain / undrain a backend; everything drained is 503"

# ── logs: recent lines and a live tail ───────────────────────────────────────
curl -s -N --max-time 4 -H "Authorization: Bearer $E2E_ADMIN_TOKEN" http://127.0.0.1:18452/admin/api/v1/logs/stream > stream.txt &
TAIL=$!
sleep 0.5
curl -s -o /dev/null "http://127.0.0.1:18451/?tail-marker-$$"
wait "$TAIL" 2>/dev/null || true
grep -q "^data: .*tail-marker-$$" stream.txt || fail "the live tail didn't carry the request: $(head -c 400 stream.txt)"
api GET '/logs?n=50' | grep -q "tail-marker-$$" || fail "recent lines don't have the request"
pass "access log: recent lines and a live SSE tail"

echo "all admin end-to-end checks passed"
