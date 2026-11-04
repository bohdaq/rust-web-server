# S4 identity: rws as OAuth 2.0 server (the app's idp role, AuthServer) and
# rws as OIDC client (the sp role, OidcAuth), each on its own gateway
# hostname. The SP learns the user from the IdP's userinfo: the standard
# path (id_token against the JWKS) is a known gap, S4.6. Sourced by run.sh.

redirect_of() { curl -sS --max-time 20 -o /dev/null -w '%{redirect_url}' "$@"; }
# The authorization code flow as a browser does it, through the SP: prints
# the SP's final page. code_flow <user> <sp jar> <idp jar>
code_flow() {
  local l
  l=$(redirect_of -c "$2" "$SP_URL/sp/me")                 # → /auth/login
  l=$(redirect_of -b "$2" -c "$2" "$l")                     # → IdP /oauth/authorize
  l=$(redirect_of "$l")                                      # → IdP /login (no session there)
  l=$(redirect_of -c "$3" "$l&user=$1")                      # → /oauth/authorize, now signed in
  l=$(redirect_of -b "$3" "$l")                              # → SP callback with the code
  echo "$l" >"$REPORT/S4.callback"   # a file: this runs in $(…)
  redirect_of -b "$2" -c "$2" "$l" >/dev/null                # the SP takes the code
  curl -sS --max-time 20 -b "$2" "$SP_URL/sp/me"
}
sp_flow_ok() {
  local out; out=$(code_flow alice "$REPORT/S4.1.sp" "$REPORT/S4.1.idp")
  echo "    /sp/me: $out" >&2
  python3 -I -c 'import json, sys; assert json.loads(sys.argv[1])["sub"] == "alice"' "$out"
}
app_check S4.1 "OIDC code flow: SP → IdP login → callback → a session for alice" sp_flow_ok
sp_two_replicas_ok() { # the SP's session is a signed cookie: both replicas know it
  local i
  for i in $(seq 6); do curl -sS --max-time 20 -b "$REPORT/S4.1.sp" "$SP_URL/sp/me" | grep -q '"sub":"alice"' || return 1; done
}
app_check S4.2 "the SP session holds on both SP replicas" sp_two_replicas_ok
replay_ok() { # the code from S4.1 was used: the SP's callback with it again fails
  local code
  code=$(sed -n 's/.*[?&]code=\([^&]*\).*/\1/p' "$REPORT/S4.callback" 2>/dev/null)
  [ -n "$code" ] || return 1
  # Straight at the token endpoint: a used code is refused.
  local out; out=$(curl -sS --max-time 20 "$IDP_URL/oauth/token" -d grant_type=authorization_code -d "code=$code" \
    -d client_id=e2e-sp -d "client_secret=$(app_secret oidc-secret)" --data-urlencode "redirect_uri=$SP_URL/auth/callback" -w '\n%{http_code}')
  echo "    replayed code: $(tail -1 <<<"$out")" >&2
  [ "$(tail -1 <<<"$out")" = 400 ]
}
app_check S4.3 "a used authorization code is refused (400)" replay_ok
bad_redirect_ok() {
  local code
  code=$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$IDP_URL/oauth/authorize?response_type=code&client_id=e2e-sp&redirect_uri=https%3A%2F%2Fevil.example%2Fcb&state=x")
  echo "    foreign redirect_uri: $code" >&2
  [ "$code" = 400 ]
}
app_check S4.4 "authorize with a foreign redirect_uri: 400, no redirect" bad_redirect_ok
logout_ok() {
  curl -sS --max-time 20 -b "$REPORT/S4.1.sp" -c "$REPORT/S4.1.sp" -o /dev/null "$SP_URL/auth/logout"
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -b "$REPORT/S4.1.sp" "$SP_URL/sp/me")" = 302 ]
}
app_check S4.5 "logout ends the SP session" logout_ok
oidc_standard_ok() { # the id_token verified against the IdP's JWKS
  local keys; keys=$(curl -sS --max-time 20 "$IDP_URL/.well-known/jwks.json")
  echo "    jwks: $keys" >&2
  python3 -I -c 'import json, sys; assert json.loads(sys.argv[1])["keys"]' "$keys"
}
app_check S4.6 "the IdP publishes signing keys (an RP can verify its id_token)" oidc_standard_ok
client_credentials_ok() {
  local out; out=$(curl -sS --max-time 20 "$IDP_URL/oauth/token" -d grant_type=client_credentials -d client_id=e2e-sp -d "client_secret=$(app_secret oidc-secret)")
  python3 -I -c 'import json, sys; assert json.loads(sys.argv[1])["access_token"]' "$out" &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$IDP_URL/oauth/token" -d grant_type=client_credentials -d client_id=e2e-sp -d client_secret=wrong)" = 401 ]
}
app_check S4.7 "client_credentials: a token for the right secret, 401 for a wrong one" client_credentials_ok
