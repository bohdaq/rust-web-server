#!/usr/bin/env bash
# `rws --check` exit codes, and proxy routes hot-reloaded on SIGHUP: a valid
# edit goes live without a restart, a broken one leaves the running routes in
# place.
#
#   ci/e2e/config_reload.sh <path-to-rws>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -30 "$WORK/rws.log" 2>/dev/null || true; exit 1; }
cd "$WORK"

write_config() { # <body> <action-type>
  cat > rws.config.toml <<TOML
port = 8701

[[route]]
  [route.match]
  path = "/v"
  [route.action]
  type = "$2"
  body = "$1"
TOML
}

# ── rws --check ──────────────────────────────────────────────────────────────
write_config v1 respond
"$RWS" --check > check.out || fail "--check rejected a valid file: $(cat check.out)"
grep -q "configuration OK" check.out || fail "--check output: $(cat check.out)"
write_config v1 respnd
if "$RWS" --check > check.out; then fail "--check accepted an unknown action type"; fi
grep -q "unknown action type 'respnd'" check.out || fail "--check output: $(cat check.out)"
echo "ok: rws --check exits 0 on a valid file, 1 on an invalid one"

# ── hot reload ───────────────────────────────────────────────────────────────
write_config v1 respond
"$RWS" > rws.log 2>&1 &
PID=$!
for _ in $(seq 1 30); do
  curl -sf -o /dev/null http://127.0.0.1:8701/v && break
  sleep 0.5
done
get() { curl -s -m 2 http://127.0.0.1:8701/v || true; }
[ "$(get)" = "v1" ] || fail "initial route: got '$(get)'"

wait_for() { # <expected body>
  for _ in $(seq 1 20); do
    [ "$(get)" = "$1" ] && return 0
    sleep 0.25
  done
  fail "expected '$1' after SIGHUP, got '$(get)'"
}

write_config v2 respond
kill -HUP "$PID"
wait_for v2
echo "ok: SIGHUP reloads proxy routes"

write_config v3 respnd
kill -HUP "$PID"
sleep 1
[ "$(get)" = "v2" ] || fail "a broken config replaced the running routes: got '$(get)'"
grep -q "keeping the running routes" rws.log || fail "no reload error logged"
kill -0 "$PID" 2>/dev/null || fail "rws exited after a broken reload"
echo "ok: a broken config is rejected and the running routes keep serving"

# ── watch = true: an edit applies itself, no signal ──────────────────────────
kill "$PID"; wait "$PID" 2>/dev/null || true
write_watch_config() { # <body> <action-type>
  write_config "$1" "$2"
  sed -i '1i watch = true' rws.config.toml
}
write_watch_config w1 respond
"$RWS" > rws.log 2>&1 &
PID=$!
for _ in $(seq 1 30); do
  curl -sf -o /dev/null http://127.0.0.1:8701/v && break
  sleep 0.5
done
[ "$(get)" = "w1" ] || fail "initial watched route: got '$(get)'"
sleep 1.1   # a new modification time, even on a 1 s-resolution filesystem
write_watch_config w2 respond
for _ in $(seq 1 24); do [ "$(get)" = "w2" ] && break; sleep 0.25; done
[ "$(get)" = "w2" ] || fail "watch: an edit was not applied: got '$(get)'"
echo "ok: watch = true applies an edit without SIGHUP"
sleep 1.1
write_watch_config w3 respnd
sleep 4
[ "$(get)" = "w2" ] || fail "watch: a broken edit replaced the running routes: got '$(get)'"
kill -0 "$PID" 2>/dev/null || fail "rws exited after a broken watched edit"
echo "ok: watch = true rejects a broken edit and keeps serving"
