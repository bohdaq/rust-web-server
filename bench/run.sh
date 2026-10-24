#!/usr/bin/env bash
# Benchmark rws against Caddy and nginx on the same machine, same files,
# same load. Prints a Markdown table (and writes bench-results.md / .json).
#
#   RWS=target/release/rws OHA=oha CADDY=caddy NGINX=nginx bench/run.sh
#
# CADDY and NGINX are optional (a missing one is skipped). The reverse-proxy
# scenario needs NGINX: an nginx instance on :9000 is the shared backend for
# every server under test. Tunables: DURATION (default 15s), CONNECTIONS (64).
#
# No compression anywhere: oha runs with --disable-compression (no
# Accept-Encoding), since the Caddy and nginx configs below don't compress and
# rws compresses text by default — otherwise rws alone would be deflating
# every response.
#
# Scenarios (HTTP/1.1, keep-alive, access logs off everywhere):
#   static-1k  GET a 1 KiB file          static-1m  GET a 1 MiB file
#   proxy-1k   GET /api/1k.bin, proxied to the backend
set -euo pipefail

RWS="${RWS:?set RWS to the rws binary}"
OHA="${OHA:?set OHA to the oha binary}"
CADDY="${CADDY:-}"
NGINX="${NGINX:-}"
DURATION="${DURATION:-15s}"
CONNECTIONS="${CONNECTIONS:-64}"

abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$RWS")"
OUT_DIR="$(pwd)"
WORK="$(mktemp -d)"
SITE="$WORK/site"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

mkdir -p "$SITE/api"
head -c 1024 /dev/urandom > "$SITE/1k.bin"
head -c 1048576 /dev/urandom > "$SITE/1m.bin"
cp "$SITE/1k.bin" "$SITE/api/1k.bin"

wait_ready() { # <url>
  for _ in $(seq 1 50); do
    curl -sf -o /dev/null "$1" && return 0
    sleep 0.2
  done
  echo "server at $1 did not start" >&2
  return 1
}

# Resident memory of a process and its children, in KiB.
rss_tree() {
  local total=0 pid
  for pid in "$1" $(pgrep -P "$1" 2>/dev/null || true); do
    total=$((total + $(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo 0)))
  done
  echo "$total"
}

RESULTS="$WORK/results.jsonl"
: > "$RESULTS"

run_load() { # <server-name> <pid> <scenario> <url>
  local name="$1" pid="$2" scenario="$3" url="$4" peak=0 rss
  "$OHA" -z 2s -c "$CONNECTIONS" --no-tui --disable-compression "$url" > /dev/null 2>&1 || true   # warm-up
  "$OHA" -z "$DURATION" -c "$CONNECTIONS" --no-tui --disable-compression --output-format json "$url" > "$WORK/oha.json" &
  local oha_pid=$!
  while kill -0 "$oha_pid" 2>/dev/null; do
    rss="$(rss_tree "$pid")"
    [ "$rss" -gt "$peak" ] && peak="$rss"
    sleep 0.25
  done
  wait "$oha_pid"
  python3 -I - "$name" "$scenario" "$peak" "$WORK/oha.json" >> "$RESULTS" <<'EOF'
import json, sys
name, scenario, peak_kib, path = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
d = json.load(open(path))
codes = d.get("statusCodeDistribution", {})
total = sum(codes.values()) or 1
print(json.dumps({
    "server": name, "scenario": scenario,
    "rps": round(d["summary"]["requestsPerSec"]),
    "p50_ms": round(d["latencyPercentiles"]["p50"] * 1000, 2),
    "p99_ms": round(d["latencyPercentiles"]["p99"] * 1000, 2),
    "ok_pct": round(100 * codes.get("200", 0) / total, 2),
    "peak_rss_mib": round(peak_kib / 1024, 1),
}))
EOF
}

bench_server() { # <name> <pid> <port>
  local name="$1" pid="$2" port="$3"
  wait_ready "http://127.0.0.1:$port/1k.bin"
  run_load "$name" "$pid" static-1k "http://127.0.0.1:$port/1k.bin"
  run_load "$name" "$pid" static-1m "http://127.0.0.1:$port/1m.bin"
  if [ -n "$NGINX" ]; then
    run_load "$name" "$pid" proxy-1k "http://127.0.0.1:$port/api/1k.bin"
  fi
}

# ── shared backend for the proxy scenario ─────────────────────────────────────
if [ -n "$NGINX" ]; then
  cat > "$WORK/backend.conf" <<EOF
worker_processes 2; daemon off; pid $WORK/backend.pid; error_log $WORK/backend-error.log;
events { worker_connections 4096; }
http { access_log off; sendfile on; keepalive_requests 1000000;
  server { listen 127.0.0.1:9000; root $SITE; } }
EOF
  "$NGINX" -c "$WORK/backend.conf" -p "$WORK" &
  PIDS+=($!)
  wait_ready "http://127.0.0.1:9000/api/1k.bin"
fi

# ── rws ───────────────────────────────────────────────────────────────────────
mkdir -p "$WORK/rws" && cd "$SITE"
cat > "$WORK/rws.config.toml" <<'EOF'
port       = 8081
log_format = "off"

[[upstream]]
name     = "backend"
backends = ["127.0.0.1:9000"]

[[route]]
  [route.match]
  path = "/api/*"

  [route.action]
  type     = "proxy"
  upstream = "backend"
EOF
RWS_CONFIG_FILE="$WORK/rws.config.toml" "$RWS" > "$WORK/rws.log" 2>&1 &
RWS_PID=$!; PIDS+=($RWS_PID)
bench_server rws "$RWS_PID" 8081
kill "$RWS_PID"

# ── Caddy ─────────────────────────────────────────────────────────────────────
if [ -n "$CADDY" ]; then
  cat > "$WORK/Caddyfile" <<EOF
{
  admin off
  auto_https off
}
:8082 {
  root * $SITE
  handle /api/* {
    reverse_proxy 127.0.0.1:9000
  }
  handle {
    file_server
  }
}
EOF
  "$CADDY" run --config "$WORK/Caddyfile" --adapter caddyfile > "$WORK/caddy.log" 2>&1 &
  CADDY_PID=$!; PIDS+=($CADDY_PID)
  bench_server caddy "$CADDY_PID" 8082
  kill "$CADDY_PID"
fi

# ── nginx ─────────────────────────────────────────────────────────────────────
if [ -n "$NGINX" ]; then
  cat > "$WORK/nginx.conf" <<EOF
worker_processes auto; daemon off; pid $WORK/nginx.pid; error_log $WORK/nginx-error.log;
events { worker_connections 4096; }
http { access_log off; sendfile on; tcp_nopush on; keepalive_requests 1000000;
  upstream backend { server 127.0.0.1:9000; keepalive 64; }
  server { listen 127.0.0.1:8083; root $SITE;
    location /api/ { proxy_pass http://backend; proxy_http_version 1.1; proxy_set_header Connection ""; } } }
EOF
  "$NGINX" -c "$WORK/nginx.conf" -p "$WORK" &
  NGINX_PID=$!; PIDS+=($NGINX_PID)
  bench_server nginx "$NGINX_PID" 8083
  kill "$NGINX_PID"
fi

# ── report ────────────────────────────────────────────────────────────────────
python3 -I - "$RESULTS" "$OUT_DIR" "$DURATION" "$CONNECTIONS" <<'EOF'
import json, platform, os, sys
path, out_dir, duration, conns = sys.argv[1:5]
rows = [json.loads(l) for l in open(path) if l.strip()]
cpus = os.cpu_count()
lines = [
    f"Load: oha, {conns} connections, {duration} per scenario, HTTP/1.1 keep-alive, no compression, access logs off. "
    f"Machine: {platform.system()} {platform.machine()}, {cpus} CPUs (load generator on the same machine).",
    "",
    "| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | 200 OK | Peak RSS (MiB) |",
    "|---|---|---:|---:|---:|---:|---:|",
]
for scenario in ["static-1k", "static-1m", "proxy-1k"]:
    for r in [r for r in rows if r["scenario"] == scenario]:
        lines.append(f"| {scenario} | {r['server']} | {r['rps']:,} | {r['p50_ms']} | {r['p99_ms']} | {r['ok_pct']}% | {r['peak_rss_mib']} |")
table = "\n".join(lines)
print(table)
open(os.path.join(out_dir, "bench-results.md"), "w").write(table + "\n")
json.dump(rows, open(os.path.join(out_dir, "bench-results.json"), "w"), indent=2)
EOF
