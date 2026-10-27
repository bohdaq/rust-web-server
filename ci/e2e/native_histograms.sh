#!/usr/bin/env bash
# Prometheus 3 scrapes rws's /metrics in the protobuf format and stores
# http_server_request_duration_seconds as a native histogram: histogram_count()
# (which only native histograms answer) must equal the requests sent, per
# protocol and status.
#
#   ci/e2e/native_histograms.sh <path-to-rws> <path-to-prometheus>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
PROMETHEUS="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
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
  echo "--- prometheus"; tail -20 "$WORK/prometheus.log" 2>/dev/null || true
  exit 1
}
cd "$WORK"
echo hello > index.html

# A backend behind a config-file proxy route, so upstream metrics (counters
# and a classic histogram from the text producers) go through the protobuf
# encoding too.
cat > backend.py <<'PY'
import http.server
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 9498), H).serve_forever()
PY
python3 backend.py &
PIDS+=($!)
cat > rws.config.toml <<'TOML'
[[upstream]]
name = "echo"
backends = ["http://127.0.0.1:9498"]

[[route]]
  [route.match]
  path = "/echo/*"
  [route.action]
  type = "proxy"
  [route.action.proxy]
  upstream = "echo"
TOML

# Plain HTTP/1.1 and h2c (prior knowledge) on one listener.
"$RWS" --port=8495 > rws.log 2>&1 &
PIDS+=($!)
for _ in $(seq 1 30); do
  curl -sf -o /dev/null http://127.0.0.1:8495/healthz && break
  sleep 0.5
done
# Series the scrapes (GET /metrics → 200 over HTTP/1.1) don't land in.
for _ in $(seq 1 20); do curl -s -o /dev/null http://127.0.0.1:8495/missing; done
for _ in $(seq 1 7); do curl -s -o /dev/null --http2-prior-knowledge http://127.0.0.1:8495/index.html; done
for _ in $(seq 1 5); do curl -s -o /dev/null http://127.0.0.1:8495/echo/x; done
echo "ok: sent 20 HTTP/1.1 GETs (404), 7 h2c GETs (200) and 5 proxied GETs"

# The protobuf answer itself.
ACCEPT='application/vnd.google.protobuf;proto=io.prometheus.client.MetricFamily;encoding=delimited;q=0.6,text/plain;version=0.0.4;q=0.3'
curl -s -D head.out -o metrics.pb -H "Accept: $ACCEPT" http://127.0.0.1:8495/metrics
grep -qi '^content-type: application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily; encoding=delimited' head.out \
  || fail "no protobuf answer: $(cat head.out)"
echo "ok: /metrics answers protobuf to a scraper that asks for it"

cat > prometheus.yml <<'YAML'
global:
  scrape_interval: 1s
scrape_configs:
  - job_name: rws
    scrape_native_histograms: true
    static_configs:
      - targets: ["127.0.0.1:8495"]
YAML
"$PROMETHEUS" --config.file=prometheus.yml --storage.tsdb.path=data --web.listen-address=127.0.0.1:9495 > prometheus.log 2>&1 &
PIDS+=($!)

query() { # <promql> → the first result's value, or empty
  curl -s --get --data-urlencode "query=$1" http://127.0.0.1:9495/api/v1/query \
    | python3 -c 'import json,sys; r=json.load(sys.stdin)["data"]["result"]; print(r[0]["value"][1] if r else "")'
}
wait_for() { # <promql> <expected>
  local got=""
  for _ in $(seq 1 60); do
    got=$(query "$1" 2>/dev/null || true)
    [ "$got" = "$2" ] && return 0
    sleep 1
  done
  fail "$1 = '$got', expected $2"
}

# Native only: histogram_count() is empty for a classic histogram.
wait_for 'sum(histogram_count(http_server_request_duration_seconds{http_request_method="GET",network_protocol_version="1.1",url_scheme="http",http_response_status_code="404"}))' 20
wait_for 'sum(histogram_count(http_server_request_duration_seconds{network_protocol_version="2",url_scheme="http",http_response_status_code="200"}))' 7
echo "ok: native histogram counts match (20 HTTP/1.1, 7 h2c)"
[ -z "$(query 'http_server_request_duration_seconds_bucket')" ] || fail "classic buckets stored too (scrape picked the text format?)"
q=$(query 'histogram_quantile(0.5, sum(http_server_request_duration_seconds{http_response_status_code="200"}))')
python3 -c "import sys; v=float('$q'); sys.exit(0 if 0 < v < 1 else 1)" || fail "median latency '$q'"
echo "ok: histogram_quantile over the native histogram: ${q}s"
# Everything else arrived too, through the same protobuf scrape.
wait_for 'rws_upstream_requests_total{upstream="echo",backend="http://127.0.0.1:9498",code="200"}' 5
wait_for 'rws_upstream_request_duration_seconds_count{upstream="echo",backend="http://127.0.0.1:9498"}' 5
wait_for 'rws_upstream_backend_up{upstream="echo",backend="http://127.0.0.1:9498"}' 1
[ "$(query 'rws_requests_total > 0')" != "" ] || fail "rws_requests_total missing from the protobuf scrape"
[ "$(query 'count(rws_active_connections)')" = 1 ] || fail "rws_active_connections missing from the protobuf scrape"
echo "ok: counters, gauges and classic histograms from the text producers arrive in the protobuf scrape"
echo "native histograms: all checks passed"
