#!/usr/bin/env bash
# Fleet config pull (#80) against the rws binary: an instance with [fleet]
# pulls a signed config from an HTTP source and applies it, then a newer
# one; a tampered config and one that fails rws --check are refused (the
# running config stays) and reported; `rws fleet keygen` / `sign` make the
# keys and signatures. With Docker, the same from S3 (SeaweedFS).
#
#   ci/e2e/fleet.sh <path-to-rws (with storage-s3 for the S3 part)>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
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
fail() { echo "FAIL: $*"; grep -h "fleet\]" "$WORK"/rws*.log 2>/dev/null | tail -10 || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"
mkdir -p www pub

"$RWS" fleet keygen ops >/dev/null || fail "rws fleet keygen"
PUB=$(cat ops.pub)
[ "$(stat -c %a ops.key)" = 600 ] || fail "ops.key isn't 0600"
pass "rws fleet keygen"

# The source: static files, and POST /report appended to reports.log.
cat > source.py <<'PY'
import http.server, functools, sys
class H(http.server.SimpleHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        open(sys.argv[2], "ab").write(body + b"\n")
        self.send_response(204); self.end_headers()
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 18581), functools.partial(H, directory=sys.argv[1])).serve_forever()
PY
python3 -I source.py "$WORK/pub" "$WORK/reports.log" & PIDS+=($!)

fleet_table() { # <source>
  printf '[fleet]\nsource = "%s"\npublic_key = "%s"\ninterval_secs = 5\nreport_url = "http://127.0.0.1:18581/report"\n' "$1" "$PUB"
}
publish() { # <body text> <version> → pub/web-1.toml + .sig
  { printf 'port = 18582\n'; fleet_table "http://127.0.0.1:18581/{node}.toml"; printf '[[route]]\n[route.match]\npath = "/*"\n[route.action]\ntype = "respond"\nstatus = 200\nbody = "%s"\n' "$1"; } > "pub/web-1.toml"
  "$RWS" fleet sign pub/web-1.toml --key ops.key >/dev/null || fail "rws fleet sign"
}
# The instance starts with only enough to find the fleet source.
{ printf 'port = 18582\n'; fleet_table "http://127.0.0.1:18581/{node}.toml"; printf '[[route]]\n[route.match]\npath = "/*"\n[route.action]\ntype = "respond"\nstatus = 200\nbody = "v0"\n'; } > rws.config.toml
publish v1
(cd www && exec env RWS_CONFIG_FILE=../rws.config.toml RWS_CONFIG_CLUSTER_NODE=web-1 "$RWS" >../rws.log 2>&1) & PIDS+=($!)
body() { curl -s http://127.0.0.1:18582/; }
eventually() { local until=$((SECONDS + $1)); until eval "$2"; do [ $SECONDS -lt $until ] || return 1; sleep 0.5; done; }
eventually 12 '[ "$(body)" = v1 ]' || fail "the signed config wasn't applied (got $(body))"
pass "a signed config from the source is applied and reloaded"

publish v2
eventually 12 '[ "$(body)" = v2 ]' || fail "the newer config wasn't applied"
pass "a newer signed config replaces it"

sed -i 's/body = "v2"/body = "tampered"/' pub/web-1.toml
sleep 7
[ "$(body)" = v2 ] || fail "a tampered config was applied"
grep -q "signature doesn't match" rws.log || fail "no log line for the bad signature"
pass "a tampered config is refused; v2 keeps serving"

{ printf 'port = 18582\n'; fleet_table "http://127.0.0.1:18581/{node}.toml"; printf '[[route]]\n[route.match]\npath = "/*"\n[route.action]\ntype = "proxy"\nupstream = "nowhere"\n'; } > pub/web-1.toml
cp pub/web-1.toml bad.toml
if "$RWS" fleet sign bad.toml --key ops.key >/dev/null 2>&1; then fail "rws fleet sign signed a config that fails --check"; fi
pass "rws fleet sign refuses a config that fails --check"

grep -q '"applied":true' reports.log && grep -q '"applied":false' reports.log && grep -q '"node":"web-1"' reports.log || fail "reports: $(cat reports.log)"
pass "outcomes reported to report_url"

kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true

if command -v docker >/dev/null && [ "${FLEET_S3:-1}" = 1 ]; then
  # SeaweedFS's S3 gateway (MinIO's images left Docker Hub); no identities
  # configured, so it accepts the signed requests as they are.
  CONTAINER=$(docker run -d --rm -p 127.0.0.1:18589:8333 chrislusf/seaweedfs:3.80 server -s3 -s3.port=8333 -dir=/data)
  for _ in $(seq 1 120); do curl -s -o /dev/null -X PUT http://127.0.0.1:18589/rws-config && break; sleep 0.5; done
  export AWS_ACCESS_KEY_ID=fleetkey AWS_SECRET_ACCESS_KEY=fleetsecret123 AWS_REGION=us-east-1
  { printf 'port = 18582\n'; printf '[fleet]\nsource = "s3://rws-config/{node}.toml"\npublic_key = "%s"\ninterval_secs = 5\ns3_endpoint = "http://127.0.0.1:18589"\n' "$PUB"; printf '[[route]]\n[route.match]\npath = "/*"\n[route.action]\ntype = "respond"\nstatus = 200\nbody = "from-s3"\n'; } > s3.toml
  "$RWS" fleet sign s3.toml --key ops.key >/dev/null
  # Create the bucket and upload with the S3 API (signed by a tiny Python SigV4 client).
  python3 -I - <<'PY' || fail "uploading to the S3 server"
import datetime, hashlib, hmac, urllib.request
def sign(key, msg): return hmac.new(key, msg.encode(), hashlib.sha256).digest()
def put(path, body):
    host = "127.0.0.1:18589"; now = datetime.datetime.utcnow(); amz = now.strftime("%Y%m%dT%H%M%SZ"); day = now.strftime("%Y%m%d")
    h = hashlib.sha256(body).hexdigest()
    canon = f"PUT\n{path}\n\nhost:{host}\nx-amz-content-sha256:{h}\nx-amz-date:{amz}\n\nhost;x-amz-content-sha256;x-amz-date\n{h}"
    scope = f"{day}/us-east-1/s3/aws4_request"
    sts = f"AWS4-HMAC-SHA256\n{amz}\n{scope}\n{hashlib.sha256(canon.encode()).hexdigest()}"
    k = sign(sign(sign(sign(b"AWS4fleetsecret123", day), "us-east-1"), "s3"), "aws4_request")
    sig = hmac.new(k, sts.encode(), hashlib.sha256).hexdigest()
    req = urllib.request.Request(f"http://{host}{path}", data=body, method="PUT", headers={"x-amz-date": amz, "x-amz-content-sha256": h, "Authorization": f"AWS4-HMAC-SHA256 Credential=fleetkey/{scope}, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature={sig}"})
    urllib.request.urlopen(req).read()
put("/rws-config/web-1.toml", open("s3.toml", "rb").read())
put("/rws-config/web-1.toml.sig", open("s3.toml.sig", "rb").read())
PY
  # SeaweedFS accepts the PUTs before it can serve them: a GET right after
  # can wait on volume placement for longer than the check below allows, and
  # rws's first pull then hasn't returned when it gives up. Start rws once
  # both objects read back.
  for _ in $(seq 1 120); do
    curl -sf -o /dev/null http://127.0.0.1:18589/rws-config/web-1.toml && curl -sf -o /dev/null http://127.0.0.1:18589/rws-config/web-1.toml.sig && break
    sleep 0.5
  done
  curl -sf -o /dev/null http://127.0.0.1:18589/rws-config/web-1.toml.sig || fail "the uploaded config never read back from the S3 server"
  { printf 'port = 18582\n'; printf '[fleet]\nsource = "s3://rws-config/{node}.toml"\npublic_key = "%s"\ninterval_secs = 5\ns3_endpoint = "http://127.0.0.1:18589"\n' "$PUB"; printf '[[route]]\n[route.match]\npath = "/*"\n[route.action]\ntype = "respond"\nstatus = 200\nbody = "v0"\n'; } > rws.config.toml
  (cd www && exec env RWS_CONFIG_FILE=../rws.config.toml RWS_CONFIG_CLUSTER_NODE=web-1 "$RWS" >../rws-s3.log 2>&1) & PIDS+=($!)
  eventually 15 '[ "$(body)" = from-s3 ]' || { grep -h "fleet" rws-s3.log | tail -5; fail "the config from S3 wasn't applied"; }
  pass "a signed config from S3 (SeaweedFS) is applied"
fi

echo "all fleet end-to-end checks passed"
