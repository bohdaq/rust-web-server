#!/usr/bin/env bash
# systemd integration against the binary, without a systemd instance:
#   - Type=notify: a Python datagram socket stands in for $NOTIFY_SOCKET and
#     must see READY=1, RELOADING=1 + READY=1 on SIGHUP, WATCHDOG=1 pings and
#     STOPPING=1 on SIGTERM;
#   - socket activation: systemd-socket-activate passes a listening socket
#     the way systemd does; rws serves on it (per-core runtimes included).
#
#   ci/e2e/systemd.sh <path-to-rws>
# Needs python3, curl and systemd-socket-activate (the systemd package).
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
cd "$WORK"
fail() { echo "FAIL: $*"; tail -20 rws*.log 2>/dev/null || true; echo "notifications:"; cat notify.out 2>/dev/null || true; exit 1; }
mkdir www && echo ok > www/index.html

python3 - "$WORK/notify.sock" > notify.out <<'PY' &
import socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
s.bind(sys.argv[1])
while True:
    print(s.recv(4096).decode().replace("\n", " "), flush=True)
PY
PIDS+=($!)
for _ in $(seq 1 50); do [ -S notify.sock ] && break; sleep 0.1; done

waitfor() { # <pattern> <what>
  for _ in $(seq 1 100); do grep -q "$1" notify.out && return 0; sleep 0.1; done
  fail "$2"
}

(cd www && exec env NOTIFY_SOCKET="$WORK/notify.sock" WATCHDOG_USEC=400000 RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT=18520 "$RWS" > ../rws.log 2>&1) & RWS_PID=$!
PIDS+=($RWS_PID)
waitfor '^READY=1 STATUS=' "no READY=1 at startup"
curl -sf -o /dev/null http://127.0.0.1:18520/ || fail "READY=1 before the server answers"
echo "ok: READY=1 once it serves"
waitfor '^WATCHDOG=1' "no watchdog pings with WATCHDOG_USEC"
echo "ok: WATCHDOG=1 pings"
kill -HUP "$RWS_PID"
waitfor '^RELOADING=1 MONOTONIC_USEC=[0-9]' "no RELOADING=1 on SIGHUP"
[ "$(grep -c '^READY=1' notify.out)" -ge 2 ] || { sleep 1; [ "$(grep -c '^READY=1' notify.out)" -ge 2 ] || fail "no READY=1 after the reload"; }
echo "ok: RELOADING=1 then READY=1 around a reload"
kill -TERM "$RWS_PID"
waitfor '^STOPPING=1' "no STOPPING=1 on SIGTERM"
echo "ok: STOPPING=1 at shutdown"

(cd www && exec systemd-socket-activate -l 127.0.0.1:18521 --fdname=http env RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT=18521 "$RWS" > ../rws-activated.log 2>&1) &
PIDS+=($!)
for _ in $(seq 1 50); do curl -sf -o /dev/null http://127.0.0.1:18521/ && break; sleep 0.2; done
[ "$(curl -sf http://127.0.0.1:18521/)" = ok ] || fail "the activated socket isn't served"
grep -q "systemd passed 1 socket" rws-activated.log || fail "rws didn't say it took systemd's socket"
for _ in $(seq 1 20); do curl -sf -o /dev/null http://127.0.0.1:18521/ || fail "a request on the activated socket failed"; done
echo "ok: socket activation (the socket systemd-socket-activate passed, every runtime)"
echo "systemd: all checks passed"
