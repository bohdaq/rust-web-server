#!/usr/bin/env bash
# A2A compatibility kit: run the official TCK
# (https://github.com/a2aproject/a2a-tck, pinned below) over JSON-RPC against
# the agent in examples/a2a_tck.rs. Tests listed in the baseline are known
# failures; the run fails on any other failure and on a listed test that now
# passes (remove it from the baseline). SHOULD-level checks are xfail in the
# kit and never fail the run.
#
#   ci/e2e/a2a_tck.sh <path-to-a2a_tck-example> [baseline.txt]
#
# Needs Python 3.11+ and git. A2A_TCK_COMMIT pins the kit; reports land in
# $RESULTS_DIR (default ./a2a-tck-results).
set -euo pipefail

AGENT="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
BASELINE="${2:+$(cd "$(dirname "$2")" && pwd)/$(basename "$2")}"
COMMIT="${A2A_TCK_COMMIT:-d4d6fae7887f88ef607fb461c13e3b0e1802f19b}"
PORT="${A2A_TCK_PORT:-9999}"
RESULTS_DIR="$(mkdir -p "${RESULTS_DIR:-a2a-tck-results}" && cd "${RESULTS_DIR:-a2a-tck-results}" && pwd)"

WORK="$(mktemp -d)"
cleanup() {
  [ -n "${AGENT_PID:-}" ] && kill "$AGENT_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
cd "$WORK"

git init -q tck && git -C tck fetch -q --depth 1 https://github.com/a2aproject/a2a-tck.git "$COMMIT" && git -C tck checkout -q FETCH_HEAD
python3 -m venv venv
venv/bin/pip install -q -e ./tck

RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT="$PORT" RWS_CONFIG_LOG_FORMAT=off "$AGENT" > agent.log 2>&1 &
AGENT_PID=$!
for _ in $(seq 1 30); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/.well-known/agent-card.json" && break
  sleep 1
done
curl -sf -o /dev/null "http://127.0.0.1:$PORT/.well-known/agent-card.json" || { echo "agent did not start:"; cat agent.log; exit 1; }

cd tck
set +e
../venv/bin/python run_tck.py --sut-host "http://127.0.0.1:$PORT" --transport jsonrpc > "$RESULTS_DIR/tck.log" 2>&1
set -e
cp -r reports/. "$RESULTS_DIR/" 2>/dev/null || true
tail -40 "$RESULTS_DIR/tck.log"

python3 - "$RESULTS_DIR/junitreport.xml" "${BASELINE:-}" <<'PY'
import sys, xml.etree.ElementTree as ET
report, baseline = sys.argv[1], sys.argv[2]
cases = ET.parse(report).getroot().iter("testcase")
failed, passed = set(), set()
for case in cases:
    name = f'{case.get("classname")}::{case.get("name")}'
    if case.find("failure") is not None or case.find("error") is not None:
        failed.add(name)
    elif case.find("skipped") is None:
        passed.add(name)
expected = set()
if baseline:
    expected = {l.strip() for l in open(baseline) if l.strip() and not l.startswith("#")}
unexpected = sorted(failed - expected)
fixed = sorted(expected & passed)
print(f"A2A TCK (JSON-RPC): {len(passed)} passed, {len(failed)} failed ({len(failed & expected)} known)")
for name in unexpected:
    print(f"FAIL (not in the baseline): {name}")
for name in fixed:
    print(f"PASSES NOW (remove from the baseline): {name}")
sys.exit(1 if unexpected or fixed else 0)
PY
