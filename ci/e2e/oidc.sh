#!/usr/bin/env bash
# OIDC sign-in and forward-auth in the config file, against the rws binary
# (built with the sso and auth features): a real Keycloak (docker) with a
# realm of two users, a route protected by `type = "oidc"` admitting the
# `ops` group, sign-in by posting Keycloak's login form with curl, the
# X-Auth-Request-* headers upstream, a user outside the group refused,
# sessions surviving a restart (signed cookies), logout; then a route behind
# `type = "forward"` with a small Python auth service.
#
#   ci/e2e/oidc.sh <path-to-rws>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
PIDS=()
CONTAINER=""
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  [ -n "$CONTAINER" ] && docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; tail -20 "$WORK"/rws*.log >&2 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

CONTAINER=$(docker run -d --rm -p 127.0.0.1:18080:8080 \
  -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin \
  -v "$HERE/oidc/realm.json:/opt/keycloak/data/import/realm.json:ro" \
  quay.io/keycloak/keycloak:26.0.7 start-dev --import-realm)
ISSUER=http://127.0.0.1:18080/realms/rws
for _ in $(seq 1 180); do curl -sf -o /dev/null "$ISSUER/.well-known/openid-configuration" && break; sleep 1; done
curl -sf -o /dev/null "$ISSUER/.well-known/openid-configuration" || { docker logs "$CONTAINER" | tail -20; fail "Keycloak didn't start"; }
pass "Keycloak up with the rws realm"

# An upstream that echoes the request headers; an auth service for forward-auth.
cat > echo.py <<'PY'
import http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = "".join(f"{k}: {v}\n" for k, v in self.headers.items()).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
cat > auth.py <<'PY'
import http.server
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.headers.get("X-Token") == "good" and self.headers.get("X-Forwarded-Uri", "").startswith("/fwd/"):
            self.send_response(200); self.send_header("Remote-User", "carol"); self.send_header("Content-Length", "0"); self.end_headers()
        else:
            self.send_response(401); self.send_header("WWW-Authenticate", 'Bearer realm="e2e"'); self.send_header("Content-Length", "6"); self.end_headers(); self.wfile.write(b"denied")
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 18503), H).serve_forever()
PY
python3 -I echo.py 18502 & PIDS+=($!)
python3 -I auth.py & PIDS+=($!)

export OIDC_SECRET=rws-e2e-secret
export OIDC_COOKIE="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
cat > rws.config.toml <<TOML
port = 18501

[[upstream]]
name = "app"
backends = ["127.0.0.1:18502"]

[[route]]
name = "forward"
[route.match]
path = "/fwd/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.auth]
type = "forward"
url = "http://127.0.0.1:18503/verify"
copy_headers = ["Remote-User"]

[[route]]
name = "app"
[route.match]
path = "/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.auth]
type = "oidc"
issuer = "$ISSUER"
client_id = "rws"
client_secret_env = "OIDC_SECRET"
cookie_secret_env = "OIDC_COOKIE"
allowed_groups = ["ops"]
exclude = ["/healthz"]
TOML
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check: $(cat check.log)"
start() {
  (exec env RWS_CONFIG_FILE=rws.config.toml "$RWS" >"rws-$1.log" 2>&1) & PIDS+=($!)
  for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18501/healthz && return 0; sleep 0.1; done
  fail "rws didn't start"
}
start first

[ "$(curl -s http://127.0.0.1:18501/healthz | grep -c '^Host:')" = 1 ] || fail "an excluded path should reach the app without signing in"
[ "$(curl -s -o /dev/null -w '%{http_code} %{redirect_url}' http://127.0.0.1:18501/app)" = "302 http://127.0.0.1:18501/auth/login?return_to=%2Fapp" ] || fail "no redirect to the login"
pass "unauthenticated: excluded path served, others sent to /auth/login"

login() { # <user> <password> <cookie jar> → the callback's status line
  local jar=$3
  rm -f "$jar"
  local idp form action
  idp=$(curl -s -c "$jar" -b "$jar" -o /dev/null -w '%{redirect_url}' "http://127.0.0.1:18501/auth/login?return_to=/app")
  [[ "$idp" == "$ISSUER/protocol/openid-connect/auth?"* ]] || fail "login didn't go to Keycloak: $idp"
  form=$(curl -s -c "$jar" -b "$jar" "$idp")
  action=$(grep -o 'id="kc-form-login"[^>]*action="[^"]*"' <<<"$form" | sed 's/.*action="//; s/"$//; s/&amp;/\&/g')
  [ -n "$action" ] || action=$(grep -o 'action="[^"]*login-actions/authenticate[^"]*"' <<<"$form" | head -1 | sed 's/action="//; s/"$//; s/&amp;/\&/g')
  [ -n "$action" ] || fail "no login form from Keycloak"
  local callback
  callback=$(curl -s -c "$jar" -b "$jar" -o /dev/null -w '%{redirect_url}' --data-urlencode "username=$1" --data-urlencode "password=$2" --data "credentialId=" "$action")
  [[ "$callback" == "http://127.0.0.1:18501/auth/callback?"* ]] || fail "Keycloak didn't redirect back: $callback"
  curl -s -c "$jar" -b "$jar" -o /dev/null -w '%{http_code} %{redirect_url}' "$callback"
}

got=$(login ann ann-pass ann.jar); [ "$got" = "302 http://127.0.0.1:18501/app" ] || fail "ann sign-in: $got"
grep -q "_rws_oidc" ann.jar || fail "no session cookie"
seen=$(curl -s -b ann.jar -H "X-Auth-Request-Email: forged@x.test" http://127.0.0.1:18501/app)
grep -qi "^X-Auth-Request-Email: ann@example.com$" <<<"$seen" || fail "no email header upstream: $seen"
grep -qi "^X-Auth-Request-Groups: ops$" <<<"$seen" || fail "no groups header upstream: $seen"
! grep -qi "forged@x.test" <<<"$seen" || fail "a client's X-Auth-Request-Email reached the app"
pass "ann (group ops) signs in through Keycloak; identity headers upstream, forged ones dropped"

status=$(login bob bob-pass bob.jar)
[ "${status%% *}" = 403 ] || fail "bob (not in ops) should be refused, got $status"
[ "$(curl -s -o /dev/null -w '%{http_code}' -b bob.jar http://127.0.0.1:18501/app)" = 302 ] || fail "bob got a session"
pass "bob (no ops group) is refused after signing in"

kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
start second
grep -qi "^X-Auth-Request-Email: ann@example.com$" <<<"$(curl -s -b ann.jar http://127.0.0.1:18501/app)" || fail "the session didn't survive a restart"
pass "signed-cookie sessions survive a restart"

curl -s -c ann.jar -b ann.jar -o /dev/null http://127.0.0.1:18501/auth/logout
[ "$(curl -s -o /dev/null -w '%{http_code}' -b ann.jar http://127.0.0.1:18501/app)" = 302 ] || fail "still signed in after logout"
pass "logout"

[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18501/fwd/x)" = 401 ] || fail "forward auth let a request without a token in"
seen=$(curl -s -H "X-Token: good" -H "Remote-User: mallory" http://127.0.0.1:18501/fwd/x)
grep -qi "^Remote-User: carol$" <<<"$seen" || fail "forward auth: Remote-User from the auth service missing: $seen"
! grep -qi "mallory" <<<"$seen" || fail "forward auth: the client's Remote-User reached the app"
pass "forward auth: 401 passed through, Remote-User from the auth service only"

echo "all OIDC and forward-auth end-to-end checks passed"
