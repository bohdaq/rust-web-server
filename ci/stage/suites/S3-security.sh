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
