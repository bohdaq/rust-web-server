# S2 reverse proxy: three whoami backends (manifests/backends.yaml) behind
# the upstreams in config/rws.config.toml. Sourced by run.sh with lib.sh.

# Per-backend counts in many whoami answers: "a=… b=… c=…".
spread() { # <count> <path> [curl args…]
  local n=$1 path=$2; shift 2
  many "$n" "$RWS_URL$path" "$@" | tr -d '\r' | awk '/^Name: backend-/ {c[substr($2, 9)]++} END {printf "a=%d b=%d c=%d\n", c["a"], c["b"], c["c"]}'
}
# within <counts> <backend> <low> <high>
within() { local v; v=$(tr ' ' '\n' <<<"$1" | awk -F= -v k="$2" '$1 == k {print $2}'); [ "$v" -ge "$3" ] && [ "$v" -le "$4" ]; }

proxy_ok() {
  local body
  body=$(http S2.1 "$RWS_URL/e2e/proxy/hello")
  grep -q '^GET /e2e/proxy/hello ' <<<"$body" && header_of proxy-status | grep -q 'received-status=200'
}
check S2.1 "proxied to a backend, Proxy-Status received-status=200" proxy_ok

forwarded_ok() { # the client from the gateway's X-Forwarded-For, the proto from its X-Forwarded-Proto
  local body
  body=$(http S2.2 "$RWS_URL/e2e/proxy/fwd" | tr -d '\r')
  grep -q '^X-Forwarded-Proto: https' <<<"$body" &&
    grep -q "^X-Forwarded-Host: ${RWS_URL#https://}" <<<"$body" &&
    grep -q '^X-Forwarded-For: [0-9a-f.:]*, ' <<<"$body"
}
check S2.2 "X-Forwarded-For/-Proto/-Host reach the backend" forwarded_ok
forwarded_header_ok() { # RFC 7239 Forwarded agrees with X-Forwarded-Proto
  http S2.3 "$RWS_URL/e2e/proxy/fwd" | tr -d '\r' | grep '^Forwarded:' | grep -q 'proto=https'
}
check S2.3 "Forwarded says proto=https behind the TLS gateway" forwarded_header_ok

rr_ok() {
  local c; c=$(spread 300 /e2e/proxy/rr)
  echo "    round robin over 300: $c" >&2
  within "$c" a 85 115 && within "$c" b 85 115 && within "$c" c 85 115
}
check S2.4 "round robin: each backend a third (±5 %) of 300" rr_ok

canary_ok() {
  local c; c=$(spread 200 /e2e/canary/x)
  echo "    weights 9:1 over 200: $c" >&2
  within "$c" a 170 190 && within "$c" b 10 30
}
check S2.5 "weights 9:1: 90/10 (±5 %) of 200" canary_ok

sticky_ok() {
  local jar="$REPORT/S2.6.cookies" first c
  rm -f "$jar"
  first=$(curl -sS --max-time 20 -c "$jar" "$RWS_URL/e2e/sticky/x" | tr -d '\r' | awk '/^Name:/ {print $2}')
  grep -q rws_e2e_backend "$jar" || return 1
  c=$(spread 30 /e2e/sticky/x -b "$jar")
  echo "    sticky to $first: $c" >&2
  within "$c" "${first#backend-}" 30 30
}
check S2.6 "sticky cookie: 30 requests stay on the first backend" sticky_ok

dead_ok() {
  http S2.7 "$RWS_URL/e2e/dead/x" >/dev/null
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = 502 ] && header_of content-type | grep -q problem+json &&
    header_of proxy-status | grep -q 'error=connection_refused'
}
check S2.7 "unreachable upstream → 502 problem+json, Proxy-Status connection_refused" dead_ok

timeout_ok() { # whoami's ?wait= outlasts read_timeout_ms = 1000
  http S2.8 "$RWS_URL/e2e/slow/x?wait=2s" >/dev/null
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = 504 ] && header_of proxy-status | grep -q 'error=http_response_timeout'
}
check S2.8 "slow backend → 504 at read_timeout_ms" timeout_ok

# A backend goes away: the health check takes it out, retries hide the
# requests caught in between, and it comes back after it returns.
eject_ok() {
  local c codes
  k scale deploy/whoami-c --replicas=0 >/dev/null
  k wait --for=delete pod -l backend=c --timeout=60s >/dev/null 2>&1 || true
  sleep 8   # unhealthy_threshold 2 × interval 2 s, with room
  codes=$(many_status 60 "$RWS_URL/e2e/proxy/eject")
  c=$(spread 60 /e2e/proxy/eject)
  echo "    whoami-c down: $c, statuses $(sort <<<"$codes" | uniq -c | tr -s ' ' | tr '\n' ',')" >&2
  ! grep -vqx 200 <<<"$codes" && within "$c" c 0 0
}
check S2.9 "a stopped backend is taken out, no errors" eject_ok
back_ok() {
  k scale deploy/whoami-c --replicas=1 >/dev/null
  k rollout status deploy/whoami-c --timeout=120s >/dev/null || return 1
  eventually 60 bash -c "$(declare -f many); RWS_URL=$RWS_URL; many 30 \$RWS_URL/e2e/proxy/back | grep -q '^Name: backend-c'"
}
check S2.10 "it comes back once healthy" back_ok

# A backend rollout under traffic: every request answered 200.
rollout_ok() {
  local out="$REPORT/S2.11.codes"
  ( end=$(( $(date +%s) + 40 )); while [ "$(date +%s)" -lt "$end" ]; do many_status 10 "$RWS_URL/e2e/proxy/rollout"; done ) >"$out" &
  local loop=$!
  k rollout restart deploy/whoami-a >/dev/null
  k rollout status deploy/whoami-a --timeout=120s >/dev/null
  wait "$loop"
  echo "    during the rollout: $(sort "$out" | uniq -c | tr -s ' ' | tr '\n' ',')" >&2
  [ -s "$out" ] && ! grep -vqx 200 "$out"
}
check S2.11 "backend rollout under traffic: no errors" rollout_ok

metrics_ok() { # the upstream counters saw it: some 200s on each backend
  local b
  for b in a b c; do
    [ "$(metric_sum "rws_upstream_requests_total{upstream=\"whoami\",backend=\"whoami-$b:8080\",code=\"200\"}")" -gt 0 ] || return 1
  done
}
check S2.12 "rws_upstream_requests_total per backend" metrics_ok
