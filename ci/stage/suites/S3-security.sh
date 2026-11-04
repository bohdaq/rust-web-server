# S3 security: bearer, JWT (HS256) and basic auth, IP filters with the
# client address from the gateway's X-Forwarded-For, the rate limit, the
# WAF. Not here yet (phase 2, the test app): CSRF, RS/ES JWT, sessions,
# ReBAC. Sourced by run.sh with lib.sh; E2E_* secrets come from run.sh.

code_and_problem() { # <id> <expected status> <path> [curl args…]
  local id=$1 want=$2 path=$3; shift 3
  http "$id" "$@" "$RWS_URL$path" >/dev/null
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = "$want" ] && header_of content-type | grep -q problem+json
}

bearer_ok() {
  [ "$(http S3.1 -H "Authorization: Bearer $E2E_BEARER" "$RWS_URL/e2e/auth/bearer")" = bearer-ok ] &&
    code_and_problem S3.1 401 /e2e/auth/bearer &&
    code_and_problem S3.1 401 /e2e/auth/bearer -H "Authorization: Bearer wrong-$E2E_BEARER"
}
check S3.1 "bearer: right token 200, none or wrong 401 problem+json" bearer_ok

jwt_ok() {
  local now; now=$(date +%s)
  [ "$(http S3.2 -H "Authorization: Bearer $(jwt_hs256 "$E2E_JWT_SECRET" $((now + 300)))" "$RWS_URL/e2e/auth/jwt")" = jwt-ok ] &&
    code_and_problem S3.2 401 /e2e/auth/jwt -H "Authorization: Bearer $(jwt_hs256 "forged-$E2E_JWT_SECRET" $((now + 300)))" &&
    code_and_problem S3.2 401 /e2e/auth/jwt -H "Authorization: Bearer $(jwt_hs256 "$E2E_JWT_SECRET" $((now - 600)))" &&
    code_and_problem S3.2 401 /e2e/auth/jwt
}
check S3.2 "JWT HS256: valid 200; forged, expired, missing 401" jwt_ok

basic_ok() {
  [ "$(http S3.3 -u "e2e:$E2E_BASIC_PASS" "$RWS_URL/e2e/auth/basic")" = basic-ok ] &&
    code_and_problem S3.3 401 /e2e/auth/basic -u "e2e:wrong" &&
    header_of www-authenticate | grep -qi '^basic'
}
check S3.3 "basic ({SHA256} htpasswd): right 200, wrong 401 with WWW-Authenticate" basic_ok

# A client on the internet can't pick its address: 203.0.113.9 in its own
# X-Forwarded-For is left of the gateway's entry and ignored.
spoof_ok() {
  [ "$(http S3.4 -H 'X-Forwarded-For: 203.0.113.9' "$RWS_URL/e2e/ip/deny-spoof")" = not-spoofed ]
}
check S3.4 "a spoofed X-Forwarded-For doesn't change the client address" spoof_ok

# allow/deny the client's real address: the routes name 192.0.2.1 until the
# ConfigMap is edited to the address the backends see, then hot-reloaded.
CLIENT=$(client_address)
echo "    client address: ${CLIENT:-?}" >&2
if [ -z "$CLIENT" ]; then
  blocked S3.5 "couldn't read the client address from whoami"
else
  config_edit "s|allow = \[\"192.0.2.1\"\]|allow = [\"$CLIENT\"]|; s|deny = \[\"192.0.2.1\"\]|deny = [\"$CLIENT\"]|" 3
  if config_wait 3; then
    check S3.5 "allow = [client] lets the client in" test "$(status_of S3.5 /e2e/ip/allow-me)" = 200
    check S3.6 "deny = [client] keeps the client out (403 problem+json)" code_and_problem S3.6 403 /e2e/ip/deny-me
  else
    blocked S3.5 "the ConfigMap edit didn't reach the pods in 4 min"
  fi
fi

ratelimit_ok() { # 5 per minute per client on each replica (no cluster Redis yet)
  local codes ok limited
  codes=$(many_status 20 "$RWS_URL/e2e/limited")
  ok=$(grep -cx 200 <<<"$codes"); limited=$(grep -cx 429 <<<"$codes")
  echo "    20 requests: $ok × 200, $limited × 429" >&2
  [ "$limited" -gt 0 ] && [ "$ok" -le 15 ] && [ $((ok + limited)) -eq 20 ] || return 1
  # The budget is per replica: the next pod may still have some.
  local i
  for i in $(seq 10); do
    code_and_problem S3.7 429 /e2e/limited && [ -n "$(header_of retry-after)" ] && return 0
  done
  return 1
}
check S3.7 "rate limit: over 5/min → 429 problem+json with Retry-After" ratelimit_ok

waf_ok() {
  [ "$(http S3.8 "$RWS_URL/e2e/waf/page?q=hello")" = waf-ok ] &&
    code_and_problem S3.8 403 "/e2e/waf/page?q=1%27%20OR%201%3D1--" &&
    code_and_problem S3.8 403 "/e2e/waf/page?q=%3Cscript%3Ealert(1)%3C%2Fscript%3E" &&
    code_and_problem S3.8 403 /e2e/waf/page -H 'Cookie: s=1%27%20UNION%20SELECT%20password%20FROM%20users--' &&
    code_and_problem S3.8 403 /e2e/waf/page -X POST -H 'Content-Type: application/json' --data '{"q":"1'"'"' OR 1=1--"}' &&
    code_and_problem S3.8 403 /e2e/waf/wp-login.php
}
check S3.8 "WAF: clean passes; SQLi/XSS in query, cookie, JSON body and an own rule → 403" waf_ok
waf_metrics_ok() { [ "$(metric_sum 'rws_waf_rule_matches_total')" -gt 0 ]; }
check S3.9 "rws_waf_rule_matches_total counts the matches" waf_metrics_ok

# ReBAC ([authz] + [route.middleware.authz]) behind JWT: olga owns doc 42,
# gina views it through group eng, mallory has nothing.
as() { # <user> <method> <path> — the status
  local now; now=$(date +%s)
  curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -X "$2" -H "Authorization: Bearer $(jwt_hs256 "$E2E_JWT_SECRET" $((now + 300)) "$1")" "$RWS_URL$3"
}
authz_read_ok() {
  [ "$(as olga GET /e2e/docs/42)" = 200 ] && [ "$(as gina GET /e2e/docs/42)" = 200 ] &&
    [ "$(as olga GET /e2e/docs/43)" = 403 ]
}
check S3.10 "authz: owner and group member read doc 42; nobody has doc 43" authz_read_ok
authz_deny_ok() {
  local now; now=$(date +%s)
  code_and_problem S3.11 403 /e2e/docs/42 -H "Authorization: Bearer $(jwt_hs256 "$E2E_JWT_SECRET" $((now + 300)) mallory)" &&
    [ "$(as gina PUT /e2e/docs/42)" = 403 ] && [ "$(as olga PUT /e2e/docs/42)" = 200 ] &&
    [ "$(status_of S3.11 /e2e/docs/42)" = 401 ]
}
check S3.11 "authz: a stranger 403 problem+json, viewer can't write, owner can, no token 401" authz_deny_ok

# ── the test app (library features the release image lacks) ──────────────────
session_ok() { # a session made on one replica is seen by every one (valkey)
  local jar="$REPORT/S3.12.jar" i me
  rm -f "$jar"
  [ "$(curl -sS --max-time 20 -c "$jar" -X POST "$APP_URL/app/session/login?user=olga")" = olga ] || return 1
  [ "$(curl -sS --max-time 20 "$APP_URL/app/session/store")" = redis ] || return 1
  for i in $(seq 9); do me=$(curl -sS --max-time 20 -b "$jar" "$APP_URL/app/session/me"); [ "$me" = olga ] || { echo "    request $i: $me" >&2; return 1; }; done
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$APP_URL/app/session/me")" = 401 ]
}
app_check S3.12 "sessions in valkey: one login holds on all 3 replicas; none 401" session_ok
csrf_ok() {
  local jar="$REPORT/S3.13.jar" token
  rm -f "$jar"
  curl -sS --max-time 20 -c "$jar" "$APP_URL/app/csrf/form" >/dev/null
  token=$(awk '$6 == "_csrf" {print $7}' "$jar")
  [ -n "$token" ] || return 1
  [ "$(curl -sS --max-time 20 -b "$jar" -d "_csrf=$token" "$APP_URL/app/csrf/submit")" = csrf-ok ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -b "$jar" -d "_csrf=wrong" "$APP_URL/app/csrf/submit")" = 403 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -d "_csrf=$token" "$APP_URL/app/csrf/submit")" = 403 ]
}
app_check S3.13 "CSRF: the form's token posts; a wrong one or none 403" csrf_ok
jwt_asym_ok() { # <alg> <path>
  local now key; now=$(date +%s); key="$REPORT/app/$(tr 'A-Z' 'a-z' <<<"$1").key"
  [ "$(curl -sS --max-time 20 -H "Authorization: Bearer $(node "$STAGE_DIR/jwt.mjs" "$1" "$key" olga $((now + 300)))" "$APP_URL$2")" = olga ] || return 1
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $(node "$STAGE_DIR/jwt.mjs" "$1" "$key" olga $((now - 600)))" "$APP_URL$2")" = 401 ] || return 1
  # A key of the same kind that isn't the configured one.
  local other="$REPORT/app/other-$1.key"
  if [ "$1" = RS256 ]; then openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$other" 2>/dev/null; else openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$other" 2>/dev/null; fi
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $(node "$STAGE_DIR/jwt.mjs" "$1" "$other" olga $((now + 300)))" "$APP_URL$2")" = 401 ]
}
app_check S3.14 "JWT RS256: valid 200; expired and another key 401" jwt_asym_ok RS256 /app/jwt/rs256
app_check S3.15 "JWT ES256: valid 200; expired and another key 401" jwt_asym_ok ES256 /app/jwt/es256
webhook_ok() {
  local body='{"action":"opened"}' sig
  sig=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$(app_secret webhook-secret)" | awk '{print $NF}')
  [ "$(curl -sS --max-time 20 -H "X-Hub-Signature-256: sha256=$sig" --data-binary "$body" "$APP_URL/app/webhook/github")" = webhook-ok ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -H "X-Hub-Signature-256: sha256=$sig" --data-binary '{"action":"closed"}' "$APP_URL/app/webhook/github")" = 401 ]
}
app_check S3.16 "incoming webhook: GitHub signature checked (tampered body 401)" webhook_ok
