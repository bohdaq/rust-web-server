#!/usr/bin/env bash
# OTLP export to a real OpenTelemetry Collector (contrib, file exporter):
# http.server.request.duration arrives with the semantic-convention
# attributes and counts that match the requests sent; server spans continue
# an incoming traceparent; the last metrics go out on SIGTERM.
#
#   ci/e2e/otlp.sh <path-to-rws> <path-to-otelcol-contrib>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
OTELCOL="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() {
  echo "FAIL: $*"
  echo "--- rws"; tail -20 "$WORK/rws.log" 2>/dev/null || true
  echo "--- collector"; tail -30 "$WORK/otelcol.log" 2>/dev/null || true
  echo "--- exported"; tail -c 3000 "$WORK/otlp.jsonl" 2>/dev/null || true
  exit 1
}
cd "$WORK"
echo hello > index.html

cat > otelcol.yaml <<'YAML'
receivers:
  otlp:
    protocols:
      http:
        endpoint: 127.0.0.1:4320
exporters:
  file:
    path: ./otlp.jsonl
service:
  telemetry:
    metrics:
      level: none
  pipelines:
    metrics:
      receivers: [otlp]
      exporters: [file]
    traces:
      receivers: [otlp]
      exporters: [file]
YAML
"$OTELCOL" --config=otelcol.yaml > otelcol.log 2>&1 &
PIDS+=($!)
for _ in $(seq 1 60); do
  curl -s -o /dev/null http://127.0.0.1:4320/ && break
  sleep 0.5
done

start_rws() { # <service name> <export interval ms>
  OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4320 OTEL_SERVICE_NAME="$1" OTEL_METRIC_EXPORT_INTERVAL="$2" \
    "$RWS" --port=8496 > rws.log 2>&1 &
  RWS_PID=$!
  PIDS+=($RWS_PID)
  for _ in $(seq 1 30); do
    curl -sf -o /dev/null http://127.0.0.1:8496/index.html && return 0
    sleep 0.5
  done
  fail "rws didn't start"
}

# The latest cumulative data point for a service, method and status:
# "<count> <sum of bucketCounts> <number of bounds> <scheme> <version>".
point() { # <service> <method> <status>
  python3 - "$@" <<'PY'
import json, sys
service, method, status = sys.argv[1], sys.argv[2], sys.argv[3]
best = None
try:
    lines = open("otlp.jsonl").read().splitlines()
except FileNotFoundError:
    lines = []
for line in lines:
    try:
        export = json.loads(line)
    except ValueError:
        continue  # the collector is still writing it
    for rm in export.get("resourceMetrics", []):
        attrs = {a["key"]: a["value"] for a in rm["resource"]["attributes"]}
        if attrs.get("service.name", {}).get("stringValue") != service:
            continue
        for sm in rm["scopeMetrics"]:
            for m in sm["metrics"]:
                if m["name"] != "http.server.request.duration" or m.get("unit") != "s":
                    continue
                h = m["histogram"]
                assert h["aggregationTemporality"] in (2, "AGGREGATION_TEMPORALITY_CUMULATIVE"), h
                for p in h["dataPoints"]:
                    a = {x["key"]: list(x["value"].values())[0] for x in p["attributes"]}
                    if a.get("http.request.method") == method and str(a.get("http.response.status_code")) == status:
                        if best is None or int(p["timeUnixNano"]) > int(best[0]["timeUnixNano"]):
                            best = (p, a)
if best:
    p, a = best
    print(p["count"], sum(int(c) for c in p["bucketCounts"]), len(p["explicitBounds"]), a["url.scheme"], a["network.protocol.version"])
PY
}

wait_for_point() { # <service> <method> <status> <expected>
  local got=""
  for _ in $(seq 1 40); do
    got=$(point "$1" "$2" "$3")
    [ "$got" = "$4" ] && return 0
    sleep 0.5
  done
  fail "$2 $3 data point for $1: '$got', expected '$4'"
}

# ── periodic export ──────────────────────────────────────────────────────────
start_rws rws-e2e 1000
for _ in $(seq 1 12); do curl -s -o /dev/null http://127.0.0.1:8496/missing; done
for _ in $(seq 1 4); do curl -s -o /dev/null --http2-prior-knowledge -X DELETE http://127.0.0.1:8496/missing; done
wait_for_point rws-e2e GET 404 "12 12 14 http 1.1"
wait_for_point rws-e2e DELETE 404 "4 4 14 http 2"
echo "ok: http.server.request.duration exported every second, counts and attributes match"

# ── traces ───────────────────────────────────────────────────────────────────
TRACE=4bf92f3577b34da6a3ce929d0e0e4736
PARENT=00f067aa0ba902b7
curl -s -o /dev/null -H "traceparent: 00-$TRACE-$PARENT-01" http://127.0.0.1:8496/index.html
found=""
for _ in $(seq 1 40); do
  found=$(python3 - "$TRACE" "$PARENT" <<'PY'
import json, sys
trace, parent = sys.argv[1], sys.argv[2]
for line in open("otlp.jsonl").read().splitlines():
    try:
        export = json.loads(line)
    except ValueError:
        continue
    for rs in export.get("resourceSpans", []):
        for ss in rs["scopeSpans"]:
            for s in ss["spans"]:
                if s["traceId"] == trace and s.get("parentSpanId") == parent and s["kind"] in (2, "SPAN_KIND_SERVER"):
                    print(s["name"])
PY
)
  [ -n "$found" ] && break
  sleep 0.5 # a partial batch goes out after 5 s
done
[ -n "$found" ] || fail "no server span continuing trace $TRACE"
kill -TERM "$RWS_PID"
wait "$RWS_PID" 2>/dev/null || true
echo "ok: server span continues the incoming traceparent ($found)"

# ── propagation through a proxied request ────────────────────────────────────
# A backend that answers with the trace headers it received.
cat > backend.py <<'PY'
import http.server
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        body = ("%s|%s" % (self.headers.get("traceparent", ""), self.headers.get("baggage", ""))).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 9497), H).serve_forever()
PY
python3 backend.py &
PIDS+=($!)
cat > rws.config.toml <<'TOML'
[[upstream]]
name = "echo"
backends = ["http://127.0.0.1:9497"]

[[route]]
  [route.match]
  path = "/echo/*"
  [route.action]
  type = "proxy"
  [route.action.proxy]
  upstream = "echo"
TOML
start_rws rws-e2e-proxy 1000
TRACE2=0af7651916cd43dd8448eb211c80319c
seen=$(curl -s -H "traceparent: 00-$TRACE2-b7ad6b7169203331-01" -H "baggage: tenant=acme,plan=pro" http://127.0.0.1:8496/echo/x)
upstream_parent=$(echo "$seen" | sed -nE "s/^00-$TRACE2-([0-9a-f]{16})-01\|.*/\1/p")
[ -n "$upstream_parent" ] || fail "upstream didn't get the trace: '$seen'"
[ "$upstream_parent" != b7ad6b7169203331 ] || fail "upstream got the client's span id, not ours: '$seen'"
echo "$seen" | grep -q '|tenant=acme,plan=pro$' || fail "baggage not passed on: '$seen'"
echo "ok: the upstream gets our traceparent (parent $upstream_parent) and the baggage"
spans=""
for _ in $(seq 1 40); do
  spans=$(python3 - "$TRACE2" "$upstream_parent" <<'PY'
import json, sys
trace, upstream_parent = sys.argv[1], sys.argv[2]
spans = {}
for line in open("otlp.jsonl").read().splitlines():
    try:
        export = json.loads(line)
    except ValueError:
        continue
    for rs in export.get("resourceSpans", []):
        for ss in rs["scopeSpans"]:
            for s in ss["spans"]:
                if s["traceId"] == trace:
                    spans[s["spanId"]] = s
client = spans.get(upstream_parent)
if client and client["kind"] in (3, "SPAN_KIND_CLIENT"):
    server = spans.get(client.get("parentSpanId"))
    attrs = {a["key"]: list(a["value"].values())[0] for a in client["attributes"]}
    if server and server["kind"] in (2, "SPAN_KIND_SERVER") and server.get("parentSpanId") == "b7ad6b7169203331":
        print(client["name"], attrs.get("server.port"), attrs.get("http.response.status_code"))
PY
)
  [ -n "$spans" ] && break
  sleep 0.5
done
[ "$spans" = "GET 9497 200" ] || fail "client span under the server span: '$spans'"
echo "ok: client span ($spans) under the server span, exported"
kill -TERM "$RWS_PID"
wait "$RWS_PID" 2>/dev/null || true
rm rws.config.toml

# ── last export on shutdown ──────────────────────────────────────────────────
start_rws rws-e2e-shutdown 600000
for _ in $(seq 1 5); do curl -s -o /dev/null -X POST -d x http://127.0.0.1:8496/missing; done
kill -TERM "$RWS_PID"
wait "$RWS_PID" 2>/dev/null || true
wait_for_point rws-e2e-shutdown POST 404 "5 5 14 http 1.1"
echo "ok: SIGTERM sends the last metrics (10-minute interval)"
echo "otlp: all checks passed"
