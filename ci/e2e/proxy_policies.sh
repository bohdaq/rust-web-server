#!/usr/bin/env bash
# Config-file upstream policies against real backends: weights, cookie
# stickiness, retries, the passive circuit breaker and the per-upstream
# metrics on /metrics.
#
#   ci/e2e/proxy_policies.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -30 "$WORK/rws.log" 2>/dev/null || true; exit 1; }
cd "$WORK"

# A backend that answers its name, or always 503 with mode "unavailable".
cat > backend.py <<'PY'
import http.server, sys
port, name, mode = int(sys.argv[1]), sys.argv[2], sys.argv[3]
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def answer(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n: self.rfile.read(n)
        status = 503 if mode == "unavailable" else 200
        body = name.encode()
        self.send_response(status)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    do_GET = do_POST = answer
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
PY
backend() { # <port> <name> [mode]
  python3 backend.py "$1" "$2" "${3:-ok}" &
  PIDS+=($!)
}
backend 9411 w1
backend 9412 w2
backend 9413 s1
backend 9414 s2
backend 9416 u1 unavailable
# 9415 (b2) starts later; 9417 never does.

route() { # <path> <upstream>
  cat <<TOML

[[route]]
  [route.match]
  path = "$1/*"
  [route.action]
  type = "proxy"
  [route.action.proxy]
  upstream = "$2"
TOML
}

{
  cat <<'TOML'
port = 8490

[[upstream]]
name = "weighted"
backends = ["http://127.0.0.1:9411", "http://127.0.0.1:9412"]
weights = [3, 1]

[[upstream]]
name = "sticky"
backends = ["http://127.0.0.1:9413", "http://127.0.0.1:9414"]
[upstream.sticky]
cookie = "lb"
max_age_secs = 600

[[upstream]]
name = "retry"
backends = ["http://127.0.0.1:9413", "http://127.0.0.1:9416"]
[upstream.retry]
attempts = 1

[[upstream]]
name = "breaker"
backends = ["http://127.0.0.1:9413", "http://127.0.0.1:9415"]
[upstream.circuit_breaker]
failure_threshold = 2
recovery_secs = 2

[[upstream]]
name = "dead"
backends = ["http://127.0.0.1:9417"]
[upstream.circuit_breaker]
failure_threshold = 1
recovery_secs = 3600
TOML
  route /w weighted
  route /s sticky
  route /r retry
  route /b breaker
  route /d dead
} > rws.config.toml

"$RWS" --check > check.out || fail "--check rejected the config: $(cat check.out)"
grep -q "configuration OK" check.out || fail "--check output: $(cat check.out)"
echo "ok: rws --check accepts weights, sticky, retry and circuit_breaker"

"$RWS" > rws.log 2>&1 &
PIDS+=($!)
for _ in $(seq 1 30); do
  curl -sf -o /dev/null http://127.0.0.1:8490/w/ && break
  sleep 0.5
done
url() { echo "http://127.0.0.1:8490$1"; }

# ── weights ──────────────────────────────────────────────────────────────────
: > weighted.out
for _ in $(seq 1 40); do curl -s "$(url /w/)" >> weighted.out; echo >> weighted.out; done
w1=$(grep -c '^w1$' weighted.out || true)
w2=$(grep -c '^w2$' weighted.out || true)
# One request went out before the loop (the readiness probe), so the split
# is 30:10 give or take that one.
[ $((w1 + w2)) -eq 40 ] && [ "$w1" -ge 29 ] && [ "$w1" -le 31 ] || fail "weights 3:1 gave w1=$w1 w2=$w2"
echo "ok: weights 3:1 → $w1:$w2 of 40"

# ── sticky cookie ────────────────────────────────────────────────────────────
curl -s -D head.out -o first.out "$(url /s/)"
cookie=$(grep -i '^set-cookie: lb=' head.out | tr -d '\r') || fail "no sticky cookie: $(cat head.out)"
echo "$cookie" | grep -q 'HttpOnly' || fail "cookie not HttpOnly: $cookie"
echo "$cookie" | grep -q 'Max-Age=600' || fail "cookie without Max-Age: $cookie"
value=$(echo "$cookie" | sed -E 's/^[Ss]et-[Cc]ookie: (lb=[0-9a-f]+).*/\1/')
first=$(cat first.out)
for _ in $(seq 1 10); do
  curl -s -D again.out -o body.out -H "Cookie: $value" "$(url /s/)"
  [ "$(cat body.out)" = "$first" ] || fail "sticky client moved from $first to $(cat body.out)"
  if grep -qi '^set-cookie: lb=' again.out; then fail "cookie set again for a client that holds it"; fi
done
echo "ok: sticky cookie ($value) keeps the client on $first"

# ── retries ──────────────────────────────────────────────────────────────────
for _ in $(seq 1 10); do
  code=$(curl -s -o body.out -w '%{http_code}' "$(url /r/)")
  [ "$code" = 200 ] && [ "$(cat body.out)" = s1 ] || fail "GET not retried away from the 503 backend: $code $(cat body.out)"
done
posts_503=0
for _ in $(seq 1 10); do
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -d x "$(url /r/)")
  [ "$code" = 503 ] && posts_503=$((posts_503 + 1))
done
[ "$posts_503" -ge 4 ] || fail "POST was retried (only $posts_503 of 10 got the 503)"
echo "ok: GETs retried to the healthy backend, POSTs not ($posts_503/10 saw 503)"

# ── passive health: circuit breaker ──────────────────────────────────────────
metric() { curl -s "$(url /metrics)" | grep -F "$1" | awk '{print $NF}' | head -1; }
for _ in $(seq 1 10); do curl -s -o /dev/null "$(url /b/)"; done
[ "$(metric 'rws_upstream_backend_up{upstream="breaker",backend="http://127.0.0.1:9415"}')" = 0 ] || fail "dead backend still up: $(curl -s "$(url /metrics)" | grep rws_upstream_)"
[ "$(metric 'rws_upstream_circuit_breaker_state{upstream="breaker",backend="http://127.0.0.1:9415"}')" = 2 ] || fail "breaker not open"
errors=$(metric 'rws_upstream_requests_total{upstream="breaker",backend="http://127.0.0.1:9415",code="error"}')
[ "$errors" = 2 ] || fail "expected 2 failed attempts before ejection, got $errors"
for _ in $(seq 1 10); do
  code=$(curl -s -o /dev/null -w '%{http_code}' "$(url /b/)")
  [ "$code" = 200 ] || fail "request to the open-breaker upstream got $code"
done
[ "$(metric 'rws_upstream_requests_total{upstream="breaker",backend="http://127.0.0.1:9415",code="error"}')" = 2 ] || fail "ejected backend still tried"
echo "ok: dead backend ejected after 2 failures, no attempts while open"

backend 9415 b2
sleep 2.5
seen=""
for _ in $(seq 1 20); do
  [ "$(curl -s "$(url /b/)")" = b2 ] && { seen=1; break; }
  sleep 0.2
done
[ -n "$seen" ] || fail "recovered backend never back in rotation"
[ "$(metric 'rws_upstream_backend_up{upstream="breaker",backend="http://127.0.0.1:9415"}')" = 1 ] || fail "recovered backend not up"
echo "ok: backend back in rotation after recovery_secs"

# ── every backend out → 503 ──────────────────────────────────────────────────
code=$(curl -s -o /dev/null -w '%{http_code}' "$(url /d/)")
[ "$code" = 502 ] || fail "first request to a dead upstream: $code"
curl -s -D head.out -o body.out "$(url /d/)"
head -1 head.out | grep -q ' 503' || fail "ejected upstream: $(head -1 head.out)"
grep -qi '^retry-after:' head.out || fail "503 without Retry-After"
echo "ok: an upstream with every backend ejected answers 503 + Retry-After"

# ── metrics series ───────────────────────────────────────────────────────────
curl -s "$(url /metrics)" > metrics.out
for series in 'rws_upstream_requests_total{upstream="weighted",backend="http://127.0.0.1:9411",code="200"}' \
  'rws_upstream_request_duration_seconds_bucket{upstream="weighted"' \
  'rws_upstream_retries_total{upstream="retry"}'; do
  grep -qF "$series" metrics.out || fail "missing $series"
done
echo "ok: per-upstream metrics on /metrics"
echo "proxy policies: all checks passed"
