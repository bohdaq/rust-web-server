# S8 operations: health endpoints, Prometheus metrics per pod, the JSON
# access log, hot reload from the ConfigMap (and a bad config kept out), a
# rolling restart under traffic, the admin API through the gateway, cluster
# state across replicas (valkey), OTLP traces and metrics to a collector.
# Not here yet: the admin UI in a browser, native histograms.
# Sourced by run.sh with lib.sh.

health_ok() { [ "$(status_of S8.1 /healthz)" = 200 ] && [ "$(status_of S8.1 /readyz)" = 200 ]; }
check S8.1 "/healthz and /readyz through the gateway" health_ok

pod_has_series() { # <pod>: the core series the docs list
  local body name
  body=$(pod_get "$1" /metrics) || return 1
  for name in rws_requests_total rws_active_connections rws_upstream_requests_total rws_upstream_request_duration_seconds_bucket rws_upstream_backend_up; do
    grep -q "^$name" <<<"$body" || { MISSING="$1: no $name"; return 1; }
  done
}
metrics_names_ok() { # on every pod; upstream series appear with proxied traffic
  local pod
  for pod in $(rws_pods); do
    # The gateway picks the pod: proxied requests, a connection each, until
    # this one has seen some.
    eventually 60 bash -c "$(declare -f pod_get pod_has_series status_of); NS=$NS; RWS_URL=$RWS_URL; pod_has_series $pod || { status_of S8.2 /e2e/proxy/metrics >/dev/null; false; }" ||
      { pod_has_series "$pod"; echo "    $MISSING" >&2; return 1; }
  done
}
check S8.2 "/metrics on every pod has the core series" metrics_names_ok

counter_moves_ok() {
  local before after
  before=$(metric_sum 'rws_requests_total ')
  many_status 60 "$RWS_URL/e2e/respond" >/dev/null
  after=$(metric_sum 'rws_requests_total ')
  echo "    rws_requests_total: $before → $after" >&2
  awk -v a="$before" -v b="$after" 'BEGIN {exit !(b - a >= 60)}'
}
check S8.3 "rws_requests_total over all pods counts 60 requests" counter_moves_ok

# /metrics names upstreams and backend addresses: not for the internet.
check S8.4 "/metrics isn't served to the internet through the gateway" sh -c "[ \"\$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 $RWS_URL/metrics)\" != 200 ]"

access_log_ok() { # one JSON line for our request, with the client's address
  local marker line pod
  marker="s8-$(head -c 6 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  [ "$(status_of S8.5 "/e2e/respond?case=$marker")" = 200 ] || return 1
  sleep 2
  for pod in $(rws_pods); do
    line=$(k logs "$pod" --since=2m 2>/dev/null | grep -F "$marker" | head -1)
    [ -n "$line" ] && break
  done
  [ -n "$line" ] || { echo "    no access log line for $marker" >&2; return 1; }
  python3 -I -c 'import json, sys; d = json.loads(sys.argv[1]); assert d["status"] == 200 and d["method"] == "GET", d' "$line" || return 1
  ACCESS_LINE=$line
}
check S8.5 "access log: a JSON line per request" access_log_ok
access_client_ok() {
  local client line=${ACCESS_LINE:-'{}'}; client=$(client_address)
  python3 -I -c 'import json, sys; d = json.loads(sys.argv[1]); sys.exit(d.get("remote_addr") != sys.argv[2])' "$line" "$client"
}
check S8.6 "access log remote_addr is the client, not the gateway" access_client_ok

# Hot reload: change the respond route's body, under traffic.
ORIGINAL_CONFIG="$REPORT/rws.config.original.toml"
config_get >"$ORIGINAL_CONFIG"
ORIGINAL_VERSION=$(config_version)
reload_ok() {
  local out="$REPORT/S8.7.codes" loop
  ( end=$(( $(date +%s) + 150 )); while [ "$(date +%s)" -lt "$end" ]; do many_status 10 "$RWS_URL/e2e/respond"; sleep 1; done ) >"$out" &
  loop=$!
  config_edit 's|"respond-ok"|"respond-reloaded"|' 8
  config_wait 8 && all_pods_serve /e2e/respond respond-reloaded || { kill "$loop" 2>/dev/null; return 1; }
  kill "$loop" 2>/dev/null; wait "$loop" 2>/dev/null
  echo "    during the reload: $(sort "$out" | uniq -c | tr -s ' ' | tr '\n' ',')" >&2
  ! grep -vqx 200 "$out"
}
check S8.7 "hot reload from the ConfigMap, no failed requests" reload_ok

bad_config_ok() { # an upstream that doesn't exist: the running routes stay
  config_edit '0,/upstream = "whoami"/s//upstream = "no-such-upstream"/' 9
  logged() {
    local pod
    for pod in $(rws_pods); do k logs "$pod" --since=6m 2>/dev/null | grep -q 'keeping the running routes' || return 1; done
  }
  eventually 300 logged || { echo "    not every pod logged the bad config" >&2; return 1; }
  all_pods_serve /e2e/config-version e2e-config-8
}
check S8.8 "a bad config on reload keeps the running routes and logs why" bad_config_ok
config_put "$ORIGINAL_CONFIG"
config_wait "$ORIGINAL_VERSION" || blocked S8.8 "the original config didn't come back"

# A rolling restart of rws under traffic: SIGTERM drain behind the gateway.
restart_ok() {
  local out="$REPORT/S8.9.codes" loop
  ( end=$(( $(date +%s) + 90 )); while [ "$(date +%s)" -lt "$end" ]; do many_status 10 "$RWS_URL/e2e/respond"; sleep 1; done ) >"$out" &
  loop=$!
  k rollout restart deploy/rws >/dev/null
  k rollout status deploy/rws --timeout=240s >/dev/null
  wait "$loop"
  echo "    during the restart: $(sort "$out" | uniq -c | tr -s ' ' | tr '\n' ',')" >&2
  ! grep -vqx 200 "$out"
}
check S8.9 "rolling restart of rws under traffic: no failed requests" restart_ok

# ── the admin API through the gateway (the rws-admin Service) ────────────────
admin_ok() {
  [ -n "$ADMIN_URL" ] || { echo "    no rws-admin hostname" >&2; return 1; }
  admin GET /status | json_get 'd["version"]' | grep -q . || return 1
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$ADMIN_URL/admin/api/v1/status")" = 401 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -H 'Authorization: Bearer wrong' "$ADMIN_URL/admin/api/v1/status")" = 401 ]
}
check S8.10 "admin API: status with the token; none or wrong 401" admin_ok

# [cluster]: the replicas see each other through valkey.
cluster_ok() {
  local nodes
  nodes=$(admin GET /status | json_get 'len((d.get("cluster") or {}).get("nodes", []))')
  [ "$nodes" = 3 ] || echo "    cluster nodes: $nodes" >&2
  [ "$nodes" = 3 ]
}
# A restarted pod stays listed for a minute after its last heartbeat.
check S8.11 "cluster: the status lists 3 replicas" eventually 120 cluster_ok

# A block taken by one replica (whichever the admin Service picks) refuses
# the client on every pod; the unblock reaches them all too.
statuses() { local i; for i in $(seq 9); do status_of S8.12 /e2e/respond; echo; done; }   # a connection each
all_are() { local out; out=$(statuses | grep . | sort -u); [ "$out" = "$1" ]; }
block_ok() {
  local client; client=$(client_address)
  [ -n "$client" ] || return 1
  admin POST /blocklist -H 'Content-Type: application/json' -d "{\"ip\":\"$client\"}" >/dev/null
  eventually 20 all_are 403; local blocked=$?
  admin DELETE "/blocklist/$client" >/dev/null
  eventually 20 all_are 200 || { echo "    the unblock didn't reach every pod" >&2; return 1; }
  [ "$blocked" = 0 ] || { echo "    the block didn't reach every pod" >&2; return 1; }
}
check S8.12 "cluster: a block on one replica refuses the client on all, unblock too" block_ok

admin_reload_ok() { admin POST /reload | json_get 'd["applied"]' | grep -qx True; }
check S8.13 "admin API: POST /reload applies" admin_reload_ok

# ── OTLP: tracing turned on (a rolling restart), spans reach the collector ───
otlp_ok() {
  local trace span
  trace=$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n'); span=$(head -c 8 /dev/urandom | od -An -tx1 | tr -d ' \n')
  k set env deploy/rws OTEL_EXPORTER_OTLP_ENDPOINT=http://otelcol:4318 OTEL_SERVICE_NAME=rws-e2e >/dev/null
  k rollout status deploy/rws --timeout=240s >/dev/null || return 1
  eventually 60 sh -c "curl -fsS --max-time 10 $RWS_URL/healthz >/dev/null" || return 1
  # Old pods (no tracing) drain for shutdown_delay_secs after the rollout and
  # the gateway may still pick them: send a traced request each try.
  logged() {
    curl -sS --max-time 20 -o /dev/null -H "traceparent: 00-$trace-$span-01" "$RWS_URL/e2e/proxy/otlp"
    sleep 3
    k logs deploy/otelcol --since=10m 2>/dev/null | grep -q "Trace ID *: $trace"
  }
  eventually 180 logged || { echo "    trace $trace not at the collector" >&2; return 1; }
  k logs deploy/otelcol --since=10m | grep -A12 "Trace ID *: $trace" | grep -q "Parent ID *: $span"
}
check S8.14 "OTLP: a request's span reaches the collector, continuing its traceparent" otlp_ok
otlp_metrics_ok() {
  eventually 90 sh -c "kubectl -n $NS logs deploy/otelcol --since=5m 2>/dev/null | grep -q 'http.server.request.duration'"
}
check S8.15 "OTLP: http.server.request.duration metrics reach the collector" otlp_metrics_ok
