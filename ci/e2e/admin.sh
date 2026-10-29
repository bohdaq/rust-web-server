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

echo "all admin end-to-end checks passed"
