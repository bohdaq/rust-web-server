#!/usr/bin/env bash
# The access log in a file: lines land there (not on stdout), the file is
# reopened on SIGUSR1 after logrotate moves it, and it rotates by size.
#
#   ci/e2e/access_log.sh <path-to-rws>
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
echo "hello" > index.html

start() { # <config body>
  printf '%s\n' "$1" > rws.config.toml
  "$RWS" > rws.log 2>&1 &
  PID=$!
  for _ in $(seq 1 30); do curl -s -o /dev/null "http://127.0.0.1:8702/healthz" && return; sleep 0.5; done
  fail "rws did not start"
}
stop() { kill "$PID"; wait "$PID" 2>/dev/null || true; PID=""; }
hit() { for _ in $(seq 1 "$1"); do curl -s -o /dev/null "http://127.0.0.1:8702/index.html?$2"; done; sleep 0.5; }
lines() { grep -c "$2" "$1" 2>/dev/null || true; }

# ── lines go to the file, and SIGUSR1 reopens it after logrotate ─────────────
start 'ip = "127.0.0.1"
port = 8702
log_format = "combined"
access_log = "access.log"'
hit 3 first
[ "$(lines access.log first)" = 3 ] || fail "expected 3 lines in access.log: $(cat access.log 2>/dev/null)"
! grep -q "index.html?first" rws.log || fail "access log lines went to stdout too"
mv access.log access.log.1
hit 2 moved
[ "$(lines access.log.1 moved)" = 2 ] || fail "before SIGUSR1 lines go on to the moved file"
kill -USR1 "$PID"
hit 2 reopened
kill -0 "$PID" || fail "SIGUSR1 killed rws"
[ "$(lines access.log reopened)" = 2 ] || fail "after SIGUSR1 lines go to a new access.log"
echo "ok: access log in a file, reopened on SIGUSR1"
stop

# ── rotation by size ─────────────────────────────────────────────────────────
rm -f access.log*
start 'ip = "127.0.0.1"
port = 8702
log_format = "combined"
access_log = "access.log"
access_log_max_size = "1K"
access_log_keep = 2'
for i in $(seq 1 6); do hit 10 "batch$i"; done
[ -f access.log.1 ] || fail "no rotation past 1 KiB: $(ls -la)"
[ ! -f access.log.3 ] || fail "more than access_log_keep files kept: $(ls)"
for f in access.log access.log.1 access.log.2; do
  [ ! -f "$f" ] || [ "$(wc -c < "$f")" -le 2048 ] || fail "$f grew past the limit: $(wc -c < "$f")"
done
echo "ok: access log rotated by size, keeping 2"
stop

# ── without access_log, SIGUSR1 keeps its default action ────────────────────
start 'ip = "127.0.0.1"
port = 8702'
kill -USR1 "$PID"
sleep 1
if kill -0 "$PID" 2>/dev/null; then fail "SIGUSR1 was caught without an access-log file"; fi
PID=""
echo "ok: SIGUSR1 untouched when logging to stdout"
