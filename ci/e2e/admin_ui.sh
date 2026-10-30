#!/usr/bin/env bash
# The admin UI (admin-ui/, embedded in src/admin/ui/) in headless Chromium
# against the rws binary: sign-in, drain from the upstreams page, maintenance
# mode, the live log, and no CSP violations.
#
#   ci/e2e/admin_ui.sh <path-to-rws>
# Needs node and `npm install playwright@1.64.0` + `npx playwright install chromium`
# in ci/e2e/admin_ui (CI does both).
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HERE="$(cd "$(dirname "$0")/admin_ui" && pwd)"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
cd "$WORK"
mkdir www
for port in 18481 18482; do
  mkdir -p "b$port" && echo "backend-$port" > "b$port/index.html"
  (cd "b$port" && exec python3 -I -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) & PIDS+=($!)
done
cat > rws.config.toml <<'TOML'
port = 18480
[admin]
listen = "127.0.0.1:18489"
token_env = "UI_E2E_ADMIN_TOKEN"

[[upstream]]
name = "web"
backends = ["http://127.0.0.1:18481", "http://127.0.0.1:18482"]
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
export UI_E2E_ADMIN_TOKEN="ui-e2e-$RANDOM"
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18489/admin/ && break; sleep 0.1; done
node "$HERE/check.mjs" http://127.0.0.1:18489 "$UI_E2E_ADMIN_TOKEN" http://127.0.0.1:18480/ || { tail -20 rws.log; exit 1; }
echo "all admin UI end-to-end checks passed"
