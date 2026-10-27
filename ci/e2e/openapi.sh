#!/usr/bin/env bash
# The OpenAPI document rws generates for examples/mcp_inventory.rs (REST
# routes described with JsonSchema request/response types) is valid
# OpenAPI 3.1, per openapi-spec-validator.
#
#   ci/e2e/openapi.sh <path-to-mcp_inventory-example> <python with openapi-spec-validator>
set -euo pipefail

EXAMPLE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
PYTHON="$2"
WORK="$(mktemp -d)"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -20 "$WORK/example.log" 2>/dev/null || true; exit 1; }
cd "$WORK"

MCP_TOKEN=e2e-token RWS_CONFIG_PORT=8499 "$EXAMPLE" > example.log 2>&1 &
PID=$!
for _ in $(seq 1 30); do
  curl -sf -o openapi.json http://127.0.0.1:8499/openapi.json && break
  sleep 0.5
done
[ -s openapi.json ] || fail "no /openapi.json"

"$PYTHON" - openapi.json <<'PY' || fail "openapi-spec-validator rejected the document"
import json, sys
from openapi_spec_validator import validate
from openapi_spec_validator.versions import consts, get_spec_version

spec = json.load(open(sys.argv[1]))
version = get_spec_version(spec)
assert version == consts.OPENAPIV31, version
validate(spec)  # raises on any error
ops = [(m, p) for p, item in spec["paths"].items() for m in item]
assert ("post", "/items/{sku}/adjust") in ops, ops
body = spec["paths"]["/items/{sku}/adjust"]["post"]["requestBody"]["content"]["application/json"]["schema"]
assert body["required"] == ["delta"], body
print("valid OpenAPI 3.1:", len(ops), "operations")
PY
echo "ok: /openapi.json is valid OpenAPI 3.1 with request and response schemas"
curl -sf -o docs.html http://127.0.0.1:8499/docs || fail "no /docs"
grep -q 'swagger-ui' docs.html || fail "/docs isn't Swagger UI"
echo "ok: /docs serves Swagger UI"
echo "openapi: all checks passed"
