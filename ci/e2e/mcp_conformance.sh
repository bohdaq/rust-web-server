#!/usr/bin/env bash
# MCP conformance: run the official suite
# (https://github.com/modelcontextprotocol/conformance) against the fixture
# server in examples/mcp_conformance.rs. Scenarios listed in the baseline
# are known failures; the run fails on any other failure and on a listed
# scenario that now passes (remove it from the baseline).
#
#   ci/e2e/mcp_conformance.sh <path-to-mcp_conformance-example> [baseline.yml]
#
# Needs Node (npx). CONFORMANCE_VERSION pins the suite (default below);
# per-scenario checks land in $RESULTS_DIR (default ./mcp-conformance-results).
set -euo pipefail

SERVER="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
BASELINE="${2:+$(cd "$(dirname "$2")" && pwd)/$(basename "$2")}"
VERSION="${CONFORMANCE_VERSION:-0.1.16}"
PORT="${MCP_CONFORMANCE_PORT:-3001}"
RESULTS_DIR="$(mkdir -p "${RESULTS_DIR:-mcp-conformance-results}" && cd "${RESULTS_DIR:-mcp-conformance-results}" && pwd)"

WORK="$(mktemp -d)"
cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

cd "$WORK"
RWS_CONFIG_IP=127.0.0.1 RWS_CONFIG_PORT="$PORT" RWS_CONFIG_LOG_FORMAT=off "$SERVER" > server.log 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 30); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/healthz" && break
  sleep 1
done
curl -s -o /dev/null "http://127.0.0.1:$PORT/healthz" || { echo "fixture server did not start:"; cat server.log; exit 1; }

args=(server --url "http://127.0.0.1:$PORT/mcp")
[ -n "$BASELINE" ] && args+=(--expected-failures "$BASELINE")
status=0
npx -y "@modelcontextprotocol/conformance@$VERSION" "${args[@]}" 2>&1 | tee conformance.out || status=$?
cp -r results/. "$RESULTS_DIR/" 2>/dev/null || true
cp conformance.out server.log "$RESULTS_DIR/"
exit "$status"
