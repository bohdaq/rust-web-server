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
# Like for like: nginx under test gets open_file_cache (rws caches open
# files by default, nginx doesn't) with open_file_cache_valid 1s, so it
# trusts a cached file's size and mtime for a second; rws gets the same
# with open_file_cache_strict = false (its default checks with an fstat on
# every request). No compression anywhere: oha runs with --disable-compression (no
# Accept-Encoding), since the Caddy and nginx configs below don't compress and
# rws compresses text by default — otherwise rws alone would be deflating
# every response.
#
# Scenarios (HTTP/1.1, keep-alive, access logs off everywhere):
#   static-1k  GET a 1 KiB file          static-1m  GET a 1 MiB file
#   proxy-1k   GET /api/1k.bin, proxied to the backend
#
# Besides req/s, each row reports the server's CPU time per request (process
# and children; the shared nginx backend isn't counted). With the load
# generator on the same machine that's what decides throughput on a small CI
# runner — and it can be measured on any machine.
#
# EXTENDED=1 adds the paths real deployments hit that the three scenarios
# above don't (each server gets equivalent settings; a scenario a server
# can't run, e.g. nginx without its ssl module, is skipped):
#   text-gzip  GET a 26 KiB JavaScript file with Accept-Encoding: gzip
#              (gzip level 6 everywhere)
#   log-1k     static-1k with an access log written to a file
#   https-1k   static-1k over TLS (HTTP/1.1)   h2-1k  the same over HTTP/2
#   https-1m   static-1m over TLS (no sendfile under TLS)
# A self-signed certificate is made with openssl.
#
# PERF_DIR=<dir> also records a `perf` profile of rws for each scenario
# (<dir>/<scenario>.data, sampled at 999 Hz with call graphs; PERF overrides
# the command, e.g. PERF="sudo perf"), and of nginx's workers
# (<dir>/nginx-<scenario>.data). Sampling costs throughput, so don't
# compare the numbers of a profiled run with an unprofiled one.
#
# RWS_BASE=<binary> also benches a second rws build ("rws-base") in the same
# run, right after the first: an A/B on the same machine, which numbers from
# separate runs (different runner hardware) can't give.
set -euo pipefail

RWS="${RWS:?set RWS to the rws binary}"
RWS_BASE="${RWS_BASE:-}"
OHA="${OHA:?set OHA to the oha binary}"
CADDY="${CADDY:-}"
NGINX="${NGINX:-}"
# nginx itself reads an NGINX environment variable (inherited listen sockets)
# and warns about a binary path in it: keep ours out of its environment.
export -n NGINX 2>/dev/null || true
DURATION="${DURATION:-15s}"
CONNECTIONS="${CONNECTIONS:-64}"
PERF_DIR="${PERF_DIR:-}"
EXTENDED="${EXTENDED:-}"
# RWS_WORKERS=<n> runs rws with n tokio worker threads (TOKIO_WORKER_THREADS)
# instead of one per CPU.
RWS_WORKERS="${RWS_WORKERS:-}"
[ -n "$RWS_WORKERS" ] && export TOKIO_WORKER_THREADS="$RWS_WORKERS"
PERF="${PERF:-perf}"
[ -n "$PERF_DIR" ] && mkdir -p "$PERF_DIR" && PERF_DIR="$(cd "$PERF_DIR" && pwd)"

abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
RWS="$(abs "$RWS")"
[ -n "$RWS_BASE" ] && RWS_BASE="$(abs "$RWS_BASE")"
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
# Compressible text for text-gzip: JavaScript-like tokens (~4:1 with gzip).
python3 -I -c '
import random
random.seed(1)
words = ["function", "return", "const", "let", "var", "if", "else", "for", "while",
         "document", "window", "value", "length", "push", "map"]
print(" ".join(random.choice(words) + str(random.randint(0, 99)) for _ in range(3500)))' > "$SITE/app.js"
if [ -n "$EXTENDED" ]; then
  openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
    -subj /CN=localhost -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null
fi

# Fail loudly instead of benchmarking the wrong process: a port that's already
# taken (a leftover server would answer wait_ready while ours exits with
# "address in use"), or a server that died after starting.
port_free() { # <port>
  if (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; then
    echo "port $1 is already in use — stop whatever is listening there first" >&2
    exit 1
  fi
}

wait_ready() { # <url> <pid>
  for _ in $(seq 1 50); do
    kill -0 "$2" 2>/dev/null || { echo "server for $1 (pid $2) exited during startup" >&2; exit 1; }
    curl -skf -o /dev/null "$1" && return 0
    sleep 0.2
  done
  echo "server at $1 did not start" >&2
  exit 1
}

# CPU time (ms) used so far by a process and its children: /proc on Linux,
# ps elsewhere.
cpu_ms() { # <pid>
  local total=0 pid ms
  for pid in "$1" $(pgrep -P "$1" 2>/dev/null || true); do
    if [ -r "/proc/$pid/stat" ]; then
      ms=$(awk -v hz="$(getconf CLK_TCK)" '{ print int(($14 + $15) * 1000 / hz) }' "/proc/$pid/stat")
    else
      ms=$(ps -o time= -p "$pid" 2>/dev/null | awk -F'[:.]' '{ if (NF==3) print ($1*60+$2)*1000+$3*10; else if (NF==4) print (($1*60+$2)*60+$3)*1000+$4*10; else print 0 }')
    fi
    total=$((total + ${ms:-0}))
  done
  echo "$total"
}

# Resident memory of a process and its children, in KiB.
rss_tree() {
  local total=0 pid
  for pid in "$1" $(pgrep -P "$1" 2>/dev/null || true); do
    total=$((total + $(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo 0)))
  done
  echo "$total"
}

# oha arguments for the next run_load (reset after each call).
OHA_ARGS=(--disable-compression)

RESULTS="$WORK/results.jsonl"
: > "$RESULTS"

run_load() { # <server-name> <pid> <scenario> <url>
  local name="$1" pid="$2" scenario="$3" url="$4" peak=0 rss cpu_before cpu_after perf_pid=""
  "$OHA" -z 2s -c "$CONNECTIONS" --no-tui "${OHA_ARGS[@]}" "$url" > /dev/null 2>&1 || true   # warm-up
  if [ -n "$PERF_DIR" ] && [ "$name" = rws ]; then
    $PERF record -F 999 -g -p "$pid" -o "$PERF_DIR/$scenario.data" -- sleep "${DURATION%s}" > /dev/null 2>&1 &
    perf_pid=$!
  elif [ -n "$PERF_DIR" ] && [ "$name" = nginx ]; then
    # The workers do the work; the master only supervises.
    $PERF record -F 999 -g -p "$(pgrep -d, -P "$pid")" -o "$PERF_DIR/nginx-$scenario.data" -- sleep "${DURATION%s}" > /dev/null 2>&1 &
    perf_pid=$!
  fi
  cpu_before=$(cpu_ms "$pid")
  "$OHA" -z "$DURATION" -c "$CONNECTIONS" --no-tui "${OHA_ARGS[@]}" --output-format json "$url" > "$WORK/oha.json" &
  local oha_pid=$!
  while kill -0 "$oha_pid" 2>/dev/null; do
    rss="$(rss_tree "$pid")"
    [ "$rss" -gt "$peak" ] && peak="$rss"
    sleep 0.25
  done
  wait "$oha_pid"
  cpu_after=$(cpu_ms "$pid")
  [ -n "$perf_pid" ] && wait "$perf_pid"
  kill -0 "$pid" 2>/dev/null || { echo "$name exited during the $scenario run" >&2; exit 1; }
  OHA_ARGS=(--disable-compression)
  python3 -I - "$name" "$scenario" "$peak" "$WORK/oha.json" "$((cpu_after - cpu_before))" >> "$RESULTS" <<'EOF'
import json, sys
name, scenario, peak_kib, path, cpu_ms = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], int(sys.argv[5])
d = json.load(open(path))
codes = d.get("statusCodeDistribution", {})
total = sum(codes.values()) or 1
print(json.dumps({
    "server": name, "scenario": scenario,
    "rps": round(d["summary"]["requestsPerSec"]),
    "p50_ms": round(d["latencyPercentiles"]["p50"] * 1000, 2),
    "p99_ms": round(d["latencyPercentiles"]["p99"] * 1000, 2),
    "ok_pct": round(100 * codes.get("200", 0) / total, 2),
    "cpu_us_per_req": round(cpu_ms * 1000 / total, 1),
    "peak_rss_mib": round(peak_kib / 1024, 1),
}))
EOF
}

bench_server() { # <name> <pid> <port>
  local name="$1" pid="$2" port="$3"
  wait_ready "http://127.0.0.1:$port/1k.bin" "$pid"
  run_load "$name" "$pid" static-1k "http://127.0.0.1:$port/1k.bin"
  run_load "$name" "$pid" static-1m "http://127.0.0.1:$port/1m.bin"
  if [ -n "$NGINX" ]; then
    run_load "$name" "$pid" proxy-1k "http://127.0.0.1:$port/api/1k.bin"
  fi
  if [ -n "$EXTENDED" ]; then
    OHA_ARGS=(-H "Accept-Encoding: gzip")
    run_load "$name" "$pid" text-gzip "http://127.0.0.1:$port/app.js"
  fi
}

# EXTENDED: a second instance with an access log (log-1k), a third with TLS.
bench_log_server() { # <name> <pid> <port>
  wait_ready "http://127.0.0.1:$3/1k.bin" "$2"
  run_load "$1" "$2" log-1k "http://127.0.0.1:$3/1k.bin"
}
bench_tls_server() { # <name> <pid> <port>
  wait_ready "https://127.0.0.1:$3/1k.bin" "$2"
  OHA_ARGS=(--disable-compression --insecure)
  run_load "$1" "$2" https-1k "https://127.0.0.1:$3/1k.bin"
  OHA_ARGS=(--disable-compression --insecure --http2)
  run_load "$1" "$2" h2-1k "https://127.0.0.1:$3/1k.bin"
  OHA_ARGS=(--disable-compression --insecure)
  run_load "$1" "$2" https-1m "https://127.0.0.1:$3/1m.bin"
}

# ── shared backend for the proxy scenario ─────────────────────────────────────
if [ -n "$NGINX" ]; then
  cat > "$WORK/backend.conf" <<EOF
worker_processes 2; daemon off; pid $WORK/backend.pid; error_log $WORK/backend-error.log;
events { worker_connections 4096; }
http { access_log off; sendfile on; keepalive_requests 1000000;
  server { listen 127.0.0.1:9000; root $SITE; } }
EOF
  port_free 9000
  "$NGINX" -c "$WORK/backend.conf" -p "$WORK" &
  BACKEND_PID=$!; PIDS+=($BACKEND_PID)
  wait_ready "http://127.0.0.1:9000/api/1k.bin" "$BACKEND_PID"
fi

# ── rws ───────────────────────────────────────────────────────────────────────
bench_rws() { # <name> <binary>
  mkdir -p "$WORK/rws" && cd "$SITE"
  cat > "$WORK/rws.config.toml" <<'EOF'
port       = 8081
log_format = "off"
open_file_cache_strict = false

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
  port_free 8081
  RWS_CONFIG_FILE="$WORK/rws.config.toml" "$2" > "$WORK/rws.log" 2>&1 &
  RWS_PID=$!; PIDS+=($RWS_PID)
  bench_server "$1" "$RWS_PID" 8081
  kill "$RWS_PID"; wait "$RWS_PID" 2>/dev/null || true
  if [ -n "$EXTENDED" ]; then
    printf 'port = 8181\nlog_format = "json"\nopen_file_cache_strict = false\n' > "$WORK/rws-log.toml"
    port_free 8181
    RWS_CONFIG_FILE="$WORK/rws-log.toml" "$2" > "$WORK/rws-access.log" 2>&1 &
    RWS_PID=$!; PIDS+=($RWS_PID)
    bench_log_server "$1" "$RWS_PID" 8181
    kill "$RWS_PID"; wait "$RWS_PID" 2>/dev/null || true
    printf 'port = 8281\nlog_format = "off"\nopen_file_cache_strict = false\ntls_cert_file = "%s"\ntls_key_file = "%s"\n' "$WORK/cert.pem" "$WORK/key.pem" > "$WORK/rws-tls.toml"
    port_free 8281
    RWS_CONFIG_FILE="$WORK/rws-tls.toml" "$2" > "$WORK/rws-tls.log" 2>&1 &
    RWS_PID=$!; PIDS+=($RWS_PID)
    bench_tls_server "$1" "$RWS_PID" 8281
    kill "$RWS_PID"; wait "$RWS_PID" 2>/dev/null || true
  fi
}
bench_rws rws "$RWS"
if [ -n "$RWS_BASE" ]; then bench_rws rws-base "$RWS_BASE"; fi

# ── Caddy ─────────────────────────────────────────────────────────────────────
if [ -n "$CADDY" ]; then
  cat > "$WORK/Caddyfile" <<EOF
{
  admin off
  auto_https off
}
:8082 {
  root * $SITE
  encode {
    gzip 6
  }
  handle /api/* {
    reverse_proxy 127.0.0.1:9000
  }
  handle {
    file_server
  }
}
EOF
  port_free 8082
  "$CADDY" run --config "$WORK/Caddyfile" --adapter caddyfile > "$WORK/caddy.log" 2>&1 &
  CADDY_PID=$!; PIDS+=($CADDY_PID)
  bench_server caddy "$CADDY_PID" 8082
  kill "$CADDY_PID"
  if [ -n "$EXTENDED" ]; then
    cat > "$WORK/Caddyfile.log" <<CADDY
{
  admin off
  auto_https off
}
:8182 {
  root * $SITE
  file_server
  log {
    output file $WORK/caddy-access.log
  }
}
CADDY
    port_free 8182
    "$CADDY" run --config "$WORK/Caddyfile.log" --adapter caddyfile > "$WORK/caddy-log.log" 2>&1 &
    CADDY_PID=$!; PIDS+=($CADDY_PID)
    bench_log_server caddy "$CADDY_PID" 8182
    kill "$CADDY_PID"
    cat > "$WORK/Caddyfile.tls" <<CADDY
{
  admin off
  auto_https disable_redirects
}
https://127.0.0.1:8282 {
  tls $WORK/cert.pem $WORK/key.pem
  root * $SITE
  file_server
}
CADDY
    port_free 8282
    "$CADDY" run --config "$WORK/Caddyfile.tls" --adapter caddyfile > "$WORK/caddy-tls.log" 2>&1 &
    CADDY_PID=$!; PIDS+=($CADDY_PID)
    bench_tls_server caddy "$CADDY_PID" 8282
    kill "$CADDY_PID"
  fi
fi

# ── nginx ─────────────────────────────────────────────────────────────────────
if [ -n "$NGINX" ]; then
  cat > "$WORK/nginx.conf" <<EOF
worker_processes auto; daemon off; pid $WORK/nginx.pid; error_log $WORK/nginx-error.log;
events { worker_connections 4096; }
http { access_log off; sendfile on; tcp_nopush on; keepalive_requests 1000000;
  open_file_cache max=128 inactive=20s; open_file_cache_valid 1s;
  types { application/javascript js; }
  gzip on; gzip_comp_level 6; gzip_types application/javascript;
  upstream backend { server 127.0.0.1:9000; keepalive 64; }
  server { listen 127.0.0.1:8083; root $SITE;
    location /api/ { proxy_pass http://backend; proxy_http_version 1.1; proxy_set_header Connection ""; } } }
EOF
  port_free 8083
  "$NGINX" -c "$WORK/nginx.conf" -p "$WORK" &
  NGINX_PID=$!; PIDS+=($NGINX_PID)
  bench_server nginx "$NGINX_PID" 8083
  kill "$NGINX_PID"
  if [ -n "$EXTENDED" ]; then
    NGINX_COMMON="events { worker_connections 4096; }
http { sendfile on; tcp_nopush on; keepalive_requests 1000000;
  open_file_cache max=128 inactive=20s; open_file_cache_valid 1s;"
    cat > "$WORK/nginx-log.conf" <<NGX
worker_processes auto; daemon off; pid $WORK/nginx-log.pid; error_log $WORK/nginx-log-error.log;
$NGINX_COMMON
  access_log $WORK/nginx-access.log;
  server { listen 127.0.0.1:8183; root $SITE; } }
NGX
    port_free 8183
    "$NGINX" -c "$WORK/nginx-log.conf" -p "$WORK" &
    NGINX_PID=$!; PIDS+=($NGINX_PID)
    bench_log_server nginx "$NGINX_PID" 8183
    kill "$NGINX_PID"
    if "$NGINX" -V 2>&1 | grep -q http_ssl_module && "$NGINX" -V 2>&1 | grep -q http_v2_module; then
      cat > "$WORK/nginx-tls.conf" <<NGX
worker_processes auto; daemon off; pid $WORK/nginx-tls.pid; error_log $WORK/nginx-tls-error.log;
$NGINX_COMMON
  access_log off;
  server { listen 127.0.0.1:8283 ssl http2; root $SITE;
    ssl_certificate $WORK/cert.pem; ssl_certificate_key $WORK/key.pem; } }
NGX
      port_free 8283
      "$NGINX" -c "$WORK/nginx-tls.conf" -p "$WORK" &
      NGINX_PID=$!; PIDS+=($NGINX_PID)
      bench_tls_server nginx "$NGINX_PID" 8283
      kill "$NGINX_PID"
    else
      echo "nginx has no ssl/http2 module: skipping its TLS scenarios" >&2
    fi
  fi
fi

# ── report ────────────────────────────────────────────────────────────────────
python3 -I - "$RESULTS" "$OUT_DIR" "$DURATION" "$CONNECTIONS" <<'EOF'
import json, platform, os, sys
path, out_dir, duration, conns = sys.argv[1:5]
rows = [json.loads(l) for l in open(path) if l.strip()]
cpus = os.cpu_count()
lines = [
    f"Load: oha, {conns} connections, {duration} per scenario, HTTP/1.1 keep-alive, no compression, access logs off "
    f"(text-gzip: Accept-Encoding gzip; log-1k: access log to a file; https-1k/https-1m: TLS; h2-1k: TLS + HTTP/2). "
    f"Machine: {platform.system()} {platform.machine()}, {cpus} CPUs (load generator on the same machine).",
    "",
    "| Scenario | Server | Requests/s | p50 (ms) | p99 (ms) | 200 OK | Server CPU/request (µs) | Peak RSS (MiB) |",
    "|---|---|---:|---:|---:|---:|---:|---:|",
]
for scenario in ["static-1k", "static-1m", "proxy-1k", "text-gzip", "log-1k", "https-1k", "h2-1k", "https-1m"]:
    for r in [r for r in rows if r["scenario"] == scenario]:
        lines.append(f"| {scenario} | {r['server']} | {r['rps']:,} | {r['p50_ms']} | {r['p99_ms']} | {r['ok_pct']}% | {r['cpu_us_per_req']} | {r['peak_rss_mib']} |")
table = "\n".join(lines)
print(table)
open(os.path.join(out_dir, "bench-results.md"), "w").write(table + "\n")
json.dump(rows, open(os.path.join(out_dir, "bench-results.json"), "w"), indent=2)
EOF
