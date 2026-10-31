#!/usr/bin/env bash
# Relationship-based authorization from the config file against the rws
# binary: JWT callers, a folder/doc/group schema, seed tuples; allowed
# directly, through a group and through the parent folder; 403 otherwise,
# 401 without a token; the same after a SIGHUP reload.
#
#   ci/e2e/authz.sh <path-to-rws>     (built with the `auth` feature)
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
fail() { echo "FAIL: $*"; tail -20 "$WORK"/rws.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir www
cat > rws.config.toml <<'TOML'
port = 18560

[authz]
tuples = [
  "doc:42#owner@user:olga",
  "group:eng#member@user:gina",
  "doc:42#editor@group:eng#member",
  "folder:specs#viewer@user:vic",
  "doc:42#parent@folder:specs",
]
[authz.type.group]
member = "user | group#member"
[authz.type.folder]
viewer = "user | group#member"
[authz.type.doc]
parent = "folder"
owner = "user"
editor = "user | group#member | owner"
viewer = "user | editor | parent->viewer"

[[route]]
name = "read"
[route.match]
path = "/docs/*"
method = "GET"
[route.action]
type = "respond"
body = "the document"
[route.middleware.auth]
type = "jwt"
secret_env = "AUTHZ_E2E_SECRET"
[route.middleware.authz]
relation = "viewer"
object = "doc:{path:2}"

[[route]]
name = "write"
[route.match]
path = "/docs/*"
method = "PUT"
[route.action]
type = "respond"
body = "saved"
[route.middleware.auth]
type = "jwt"
secret_env = "AUTHZ_E2E_SECRET"
[route.middleware.authz]
relation = "editor"
object = "doc:{path:2}"
TOML
export AUTHZ_E2E_SECRET="e2e-$RANDOM"
"$RWS" --check >check.log 2>&1 || { cat check.log; fail "rws --check rejected the config"; }
grep -q "warning" check.log && { cat check.log; fail "rws --check warned"; }
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml "$RWS" >../rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18560/healthz && break; sleep 0.1; done

token() {
  python3 -I - "$AUTHZ_E2E_SECRET" "$1" <<'PY'
import base64, hashlib, hmac, json, sys
b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
head = b64(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
body = b64(json.dumps({"sub": sys.argv[2], "exp": 4102444800}).encode())
sig = b64(hmac.new(sys.argv[1].encode(), f"{head}.{body}".encode(), hashlib.sha256).digest())
print(f"{head}.{body}.{sig}")
PY
}
status() { curl -s -o /dev/null -w '%{http_code}' -X "$1" ${3:+-H "Authorization: Bearer $(token "$3")"} "http://127.0.0.1:18560$2"; }

check_all() {
  [ "$(status GET /docs/42 olga)" = 200 ] || fail "owner can't read"
  [ "$(status GET /docs/42 gina)" = 200 ] || fail "group member can't read"
  [ "$(status GET /docs/42 vic)" = 200 ] || fail "folder viewer can't read"
  [ "$(status PUT /docs/42 gina)" = 200 ] || fail "group member can't edit"
  [ "$(status PUT /docs/42 vic)" = 403 ] || fail "folder viewer could edit"
  [ "$(status GET /docs/42 eve)" = 403 ] || fail "a stranger could read"
  [ "$(status GET /docs/7 olga)" = 403 ] || fail "the owner of 42 could read 7"
  [ "$(status GET /docs/42)" = 401 ] || fail "no token wasn't 401"
}
check_all
pass "owner, group member and folder viewer read; only editors write; others 403; no token 401"

kill -HUP "${PIDS[-1]}"
sleep 1
grep -q "ERROR" rws.log && fail "reload logged an error"
check_all
pass "the same after a SIGHUP reload"
echo "all authorization end-to-end checks passed"
