#!/usr/bin/env bash
# Benchmark Kubernetes Gateway API implementations against each other on one
# machine, one cluster, one backend, the same load scenarios (see measure()): rws and the gateways named in
# GATEWAYS (default: all). Prints a Markdown table (and writes results.md /
# results.json to OUT_DIR).
#
#   RWS=<static rws binary built with gateway-api> OHA=oha bench/gateway/run.sh
#
# Needs docker, kind, kubectl, helm, python3 with PyYAML; Istio's istioctl is
# downloaded. Tunables: DURATION (20s), CONNECTIONS ("64 256"), GATEWAYS.
#
# Like for like: one kind node (the gateways and the backend share its CPUs
# with the load generator, as a small production node would); every gateway
# at its chart defaults with one data-plane replica and no resource limits
# (rws's chart sets limits by default; the others don't, so the run drops
# them), access logs as they ship;
# the same HTTPRoute (one rule, prefix /) to the same nginx backend (2 pods,
# 1 KiB file, keep-alive, in its own namespace); MetalLB gives each Gateway a LoadBalancer address
# on kind's Docker network; oha runs HTTP/1.1 keep-alive from the runner, no
# compression. Each gateway is installed, measured and uninstalled in turn.
#
# Per row: req/s, p50/p99 latency, the share of 200s, the CPU time the
# gateway's pods (controller and data plane, from the kubelet's
# /stats/summary) spent per request, and their memory (working set) after the
# load.
set -euo pipefail

RWS="${RWS:?RWS: the rws binary to benchmark}"
RWS="$(cd "$(dirname "$RWS")" && pwd)/$(basename "$RWS")"
OHA="${OHA:-oha}"
DURATION="${DURATION:-20s}"
CONNECTIONS="${CONNECTIONS:-64 256}"
GATEWAYS="${GATEWAYS:-rws envoy-gateway nginx-gateway-fabric traefik kgateway istio}"
OUT_DIR="${OUT_DIR:-bench-gateway-results}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLUSTER=rws-gateway-bench
NODE="$CLUSTER-control-plane"
WORK="$(mktemp -d)"
RESULTS="$WORK/results.jsonl"
mkdir -p "$OUT_DIR"
: > "$RESULTS"

# Versions: each the newest release at least two weeks old (2026-10-28).
GATEWAY_API_VERSION=v1.6.3
METALLB_VERSION=v0.16.0
ENVOY_GATEWAY_VERSION=v1.9.2
NGF_VERSION=2.7.2
TRAEFIK_CHART_VERSION=41.7.1
KGATEWAY_VERSION=v2.4.6
ISTIO_VERSION=1.30.5
NGINX_IMAGE=nginx:1.28.0-alpine

log() { echo "── $*" >&2; }

# ── cluster ──────────────────────────────────────────────────────────────────

log "kind cluster"
kind create cluster --name "$CLUSTER" --wait 120s
kubectl label nodes --all node.kubernetes.io/exclude-from-external-load-balancers- || true
kubectl apply --server-side -f "https://github.com/kubernetes-sigs/gateway-api/releases/download/$GATEWAY_API_VERSION/standard-install.yaml"
kubectl wait --for=condition=Established crd --all --timeout=60s

log "MetalLB"
kubectl apply -f "https://raw.githubusercontent.com/metallb/metallb/$METALLB_VERSION/config/manifests/metallb-native.yaml"
kubectl wait -n metallb-system --for=condition=Available deployment --all --timeout=180s
kubectl wait -n metallb-system --for=condition=Ready pod -l component=speaker --timeout=180s
subnet=$(docker network inspect kind -f '{{range .IPAM.Config}}{{println .Subnet}}{{end}}' | grep -m1 '\.')
prefix=$(echo "$subnet" | cut -d. -f1-2)
for _ in $(seq 1 30); do
  kubectl apply -f - <<YAML && break || sleep 2
apiVersion: metallb.io/v1beta1
kind: IPAddressPool
metadata: {name: kind, namespace: metallb-system}
spec: {addresses: ["$prefix.255.200-$prefix.255.250"]}
---
apiVersion: metallb.io/v1beta1
kind: L2Advertisement
metadata: {name: kind, namespace: metallb-system}
YAML
done

log "backend"
# Its own namespace: Gateways (and data planes some implementations
# provision next to them) live in bench, whose pods are all counted.
kubectl create namespace backend
kubectl create namespace bench
kubectl apply -n backend -f - <<YAML
apiVersion: v1
kind: ConfigMap
metadata: {name: backend}
data:
  nginx.conf: |
    worker_processes auto;
    events { worker_connections 4096; }
    http {
      access_log off;
      keepalive_requests 1000000;
      server {
        listen 8080;
        location / { root /srv; }
      }
    }
---
apiVersion: apps/v1
kind: Deployment
metadata: {name: backend}
spec:
  replicas: 2
  selector: {matchLabels: {app: backend}}
  template:
    metadata: {labels: {app: backend}}
    spec:
      # A ConfigMap holds at most 1 MiB: the files are written at start.
      initContainers:
        - name: files
          image: $NGINX_IMAGE
          command: ["sh", "-c", "head -c 1024 /dev/zero > /srv/1k.bin && head -c 1048576 /dev/zero > /srv/1m.bin"]
          volumeMounts: [{name: files, mountPath: /srv}]
      containers:
        - name: nginx
          image: $NGINX_IMAGE
          args: ["nginx", "-c", "/etc/backend/nginx.conf", "-g", "daemon off;"]
          ports: [{containerPort: 8080}]
          readinessProbe: {httpGet: {path: /1k.bin, port: 8080}}
          volumeMounts:
            - {name: conf, mountPath: /etc/backend}
            - {name: files, mountPath: /srv}
      volumes:
        - {name: conf, configMap: {name: backend}}
        - {name: files, emptyDir: {}}
---
apiVersion: v1
kind: Service
metadata: {name: backend}
spec:
  selector: {app: backend}
  ports: [{name: http, port: 8080, targetPort: 8080}]
---
apiVersion: gateway.networking.k8s.io/v1beta1
kind: ReferenceGrant
metadata: {name: bench}
spec:
  from: [{group: gateway.networking.k8s.io, kind: HTTPRoute, namespace: bench}]
  to: [{group: "", kind: Service, name: backend}]
YAML
kubectl wait -n backend --for=condition=Available deployment/backend --timeout=180s

# The HTTPS listeners' certificate (self-signed; the load generator doesn't verify it).
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
  -subj /CN=bench.example -addext subjectAltName=DNS:bench.example \
  -keyout "$WORK/tls.key" -out "$WORK/tls.crt" 2>/dev/null
kubectl create secret tls bench-tls -n bench --cert "$WORK/tls.crt" --key "$WORK/tls.key"

# ── one gateway ──────────────────────────────────────────────────────────────

# A Gateway of <class> in namespace bench with an HTTP listener on <port>
# and an HTTPS one on <tls-port>; a plain route to the backend, and one under
# /f with filters (request and response header modifiers, a prefix rewrite).
gateway_and_route() { # <class> <port> <tls-port>
  kubectl apply -n bench -f - <<YAML
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata: {name: bench}
spec:
  gatewayClassName: $1
  listeners:
    - {name: http, port: $2, protocol: HTTP}
    - {name: https, port: $3, protocol: HTTPS, tls: {certificateRefs: [{name: bench-tls}]}}
---
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata: {name: bench}
spec:
  parentRefs: [{name: bench}]
  rules: [{backendRefs: [{name: backend, namespace: backend, port: 8080}]}]
---
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata: {name: filters}
spec:
  parentRefs: [{name: bench}]
  rules:
    - matches: [{path: {type: PathPrefix, value: /f}}]
      filters:
        - type: RequestHeaderModifier
          requestHeaderModifier:
            set: [{name: X-Bench, value: "1"}]
            add: [{name: X-Bench-Trace, value: gateway}]
            remove: [X-Unwanted]
        - type: ResponseHeaderModifier
          responseHeaderModifier:
            set: [{name: X-Served-By, value: bench}]
            remove: [Server]
        - type: URLRewrite
          urlRewrite: {path: {type: ReplacePrefixMatch, replacePrefixMatch: /}}
      backendRefs: [{name: backend, namespace: backend, port: 8080}]
YAML
  kubectl wait -n bench gateway/bench --for=condition=Programmed --timeout=300s
}

# The Gateway's address, once it has one (some report Programmed first).
gateway_address() {
  local address=""
  for _ in $(seq 1 90); do
    address=$(kubectl get -n bench gateway/bench -o jsonpath='{.status.addresses[0].value}')
    [ -n "$address" ] && break
    sleep 2
  done
  [ -n "$address" ] || { echo "no Gateway address" >&2; return 1; }
  echo "$address"
}

# Container restarts in <namespaces…> (a restart resets the CPU counters,
# so a row with restarts isn't comparable).
restarts() { # <namespaces…>
  local total=0 n
  for ns in "$@"; do
    n=$(kubectl get pods -n "$ns" -o jsonpath='{range .items[*]}{range .status.containerStatuses[*]}{.restartCount}{"\n"}{end}{end}' 2>/dev/null | awk '{s+=$1} END {print s+0}')
    total=$((total + n))
  done
  echo "$total"
}

# CPU nanoseconds the pods in <namespaces…> have used, and their working set.
pod_usage() { # <namespaces…>
  kubectl get --raw "/api/v1/nodes/$NODE/proxy/stats/summary" | python3 -I -c '
import json, sys
spaces = set(sys.argv[1:])
cpu = mem = 0
for pod in json.load(sys.stdin).get("pods", []):
    if pod["podRef"]["namespace"] in spaces:
        for c in pod.get("containers", []):
            cpu += (c.get("cpu") or {}).get("usageCoreNanoSeconds", 0)
            mem += (c.get("memory") or {}).get("workingSetBytes", 0)
print(cpu, mem)' "$@"
}

# One load run: <scenario> on <url> at <connections>, with oha's extra
# arguments in OHA_EXTRA; one JSON line into RESULTS.
run_one() { # <name> <scenario> <connections> <url> <namespaces…>
  local name="$1" scenario="$2" c="$3" url="$4"; shift 4
  restarts_before=$(restarts "$@")
  read -r cpu_before _ < <(pod_usage "$@")
  "$OHA" -z "$DURATION" -c "$c" --no-tui --disable-compression "${OHA_EXTRA[@]}" --output-format json "$url" > "$WORK/oha.json"
  sleep 1
  read -r cpu_after mem_after < <(pod_usage "$@")
  restarted=$(( $(restarts "$@") - restarts_before ))
  python3 -I - "$name" "$scenario" "$c" "$WORK/oha.json" "$((cpu_after - cpu_before))" "$mem_after" "$restarted" >> "$RESULTS" <<'PY'
import json, sys
name, scenario, conns, path = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
cpu_ns, mem, restarted = int(sys.argv[5]), int(sys.argv[6]), int(sys.argv[7])
d = json.load(open(path))
codes = d.get("statusCodeDistribution", {})
total = sum(codes.values()) or 1
print(json.dumps({
    "gateway": name, "scenario": scenario, "connections": conns,
    "rps": round(d["summary"]["requestsPerSec"]),
    "p50_ms": round(d["latencyPercentiles"]["p50"] * 1000, 2),
    "p99_ms": round(d["latencyPercentiles"]["p99"] * 1000, 2),
    "ok_pct": round(100 * codes.get("200", 0) / total, 2),
    "cpu_us_per_req": round(cpu_ns / 1000 / total, 1),
    "mem_mib": round(mem / 2**20, 1),
    "restarts": restarted,
    "other_codes": {k: v for k, v in codes.items() if k != "200"},
    # Requests in flight when the duration ends are "aborted due to
    # deadline" (one per connection): not errors.
    "errors": {k: v for k, v in (d.get("errorDistribution") or {}).items() if "deadline" not in k},
}))
PY
  log "$name $scenario c=$c: $(tail -1 "$RESULTS")"
}

# Every scenario through one gateway; <http> and <https> are the
# address:port of its two listeners.
#   http-1k     GET 1 KiB over HTTP/1.1, at each of CONNECTIONS
#   http-1m     GET 1 MiB over HTTP/1.1
#   https-1k    GET 1 KiB over HTTP/1.1 + TLS (terminated at the gateway)
#   h2-1k       GET 1 KiB over HTTP/2 + TLS
#   filters-1k  GET 1 KiB through the route with header modifiers and a rewrite
measure() { # <name> <http> <https> <namespaces…>
  local name="$1" http="$2" https="$3"; shift 3
  for _ in $(seq 1 60); do curl -fsS -o /dev/null "http://$http/1k.bin" && break || sleep 2; done
  for _ in $(seq 1 30); do curl -kfsS -o /dev/null "https://$https/1k.bin" && break || sleep 2; done
  "$OHA" -z 5s -c 64 --no-tui --disable-compression "http://$http/1k.bin" > /dev/null 2>&1 || true   # warm-up
  for c in $CONNECTIONS; do
    OHA_EXTRA=(); run_one "$name" http-1k "$c" "http://$http/1k.bin" "$@"
  done
  OHA_EXTRA=(); run_one "$name" http-1m 64 "http://$http/1m.bin" "$@"
  OHA_EXTRA=(--insecure); run_one "$name" https-1k 64 "https://$https/1k.bin" "$@"
  OHA_EXTRA=(--insecure --http2); run_one "$name" h2-1k 64 "https://$https/1k.bin" "$@"
  OHA_EXTRA=(); run_one "$name" filters-1k 64 "http://$http/f/1k.bin" "$@"
}

cleanup_bench() {
  kubectl delete -n bench gateway/bench httproute/bench httproute/filters --ignore-not-found --wait=true --timeout=120s || true
}

# ── the gateways ─────────────────────────────────────────────────────────────

bench_rws() {
  local build; build=$(mktemp -d)
  cp "$RWS" "$build/rws"
  docker build -q -t rws:bench -f "$ROOT/ci/e2e/gateway_api/Dockerfile" "$build" > /dev/null
  kind load docker-image rws:bench --name "$CLUSTER"
  helm install rws "$ROOT/helm/rws" -n rws-system --create-namespace \
    --set image.repository=rws --set image.tag=bench --set image.pullPolicy=Never \
    --set gatewayAPI.enabled=true --set tls.enabled=false --set quic.enabled=false \
    --set replicaCount=1 --set podDisruptionBudget.enabled=false \
    --set resources.limits=null \
    --wait --timeout 5m
  kubectl wait gatewayclass/rws --for=condition=Accepted --timeout=120s
  gateway_and_route rws 80 443
  address=$(gateway_address)
  measure rws "$address:80" "$address:443" rws-system bench
  cleanup_bench
  helm uninstall rws -n rws-system --wait
}

bench_envoy_gateway() {
  helm install eg oci://docker.io/envoyproxy/gateway-helm --version "$ENVOY_GATEWAY_VERSION" \
    -n envoy-gateway-system --create-namespace --wait --timeout 5m
  kubectl apply -f - <<YAML
apiVersion: gateway.networking.k8s.io/v1
kind: GatewayClass
metadata: {name: envoy-gateway}
spec: {controllerName: gateway.envoyproxy.io/gatewayclass-controller}
YAML
  kubectl wait gatewayclass/envoy-gateway --for=condition=Accepted --timeout=120s
  gateway_and_route envoy-gateway 80 443
  address=$(gateway_address)
  measure envoy-gateway "$address:80" "$address:443" envoy-gateway-system bench
  cleanup_bench
  kubectl delete gatewayclass envoy-gateway --wait=true
  helm uninstall eg -n envoy-gateway-system --wait
}

bench_nginx_gateway_fabric() {
  helm install ngf oci://ghcr.io/nginx/charts/nginx-gateway-fabric --version "$NGF_VERSION" \
    -n nginx-gateway --create-namespace --wait --timeout 5m
  kubectl wait gatewayclass/nginx --for=condition=Accepted --timeout=120s
  gateway_and_route nginx 80 443
  # NGF provisions the data plane in the Gateway's namespace (bench).
  address=$(gateway_address)
  measure nginx-gateway-fabric "$address:80" "$address:443" nginx-gateway bench
  cleanup_bench
  helm uninstall ngf -n nginx-gateway --wait
}

bench_traefik() {
  helm repo add traefik https://traefik.github.io/charts > /dev/null
  helm install traefik traefik/traefik --version "$TRAEFIK_CHART_VERSION" -n traefik --create-namespace \
    --set providers.kubernetesGateway.enabled=true --set providers.kubernetesIngress.enabled=false \
    --set gateway.enabled=false --set gatewayClass.enabled=true --wait --timeout 5m
  kubectl wait gatewayclass/traefik --for=condition=Accepted --timeout=120s
  # Traefik's listeners are its entrypoints: web listens on 8000 and
  # websecure on 8443; its Service maps 80 and 443 to them.
  gateway_and_route traefik 8000 8443
  local address; address=$(kubectl get -n traefik service traefik -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
  measure traefik "$address:80" "$address:443" traefik bench
  cleanup_bench
  helm uninstall traefik -n traefik --wait
}

bench_kgateway() {
  helm install kgateway-crds oci://cr.kgateway.dev/kgateway-dev/charts/kgateway-crds --version "$KGATEWAY_VERSION" \
    -n kgateway-system --create-namespace --wait --timeout 5m
  helm install kgateway oci://cr.kgateway.dev/kgateway-dev/charts/kgateway --version "$KGATEWAY_VERSION" \
    -n kgateway-system --wait --timeout 5m
  kubectl wait gatewayclass/kgateway --for=condition=Accepted --timeout=120s
  gateway_and_route kgateway 80 443
  address=$(gateway_address)
  measure kgateway "$address:80" "$address:443" kgateway-system bench
  cleanup_bench
  helm uninstall kgateway -n kgateway-system --wait
}

bench_istio() {
  local dir; dir=$(mktemp -d)
  curl -fsSL "https://github.com/istio/istio/releases/download/$ISTIO_VERSION/istioctl-$ISTIO_VERSION-linux-amd64.tar.gz" | tar xz -C "$dir"
  "$dir/istioctl" install --set profile=minimal -y
  kubectl wait gatewayclass/istio --for=condition=Accepted --timeout=180s
  gateway_and_route istio 80 443
  address=$(gateway_address)
  measure istio "$address:80" "$address:443" istio-system bench
  cleanup_bench
  "$dir/istioctl" uninstall --purge -y
}

for gateway in $GATEWAYS; do
  log "gateway: $gateway"
  # In a subshell outside any condition, so `set -e` stops this gateway's
  # steps at the first failure (it's ignored inside `if fn`).
  set +e
  ( set -e; "bench_${gateway//-/_}" )
  status=$?
  set -e
  if [ "$status" != 0 ]; then
    log "$gateway failed; skipping it"
    echo "{\"gateway\": \"$gateway\", \"failed\": true}" >> "$RESULTS"
    cleanup_bench
  fi
done

python3 -I - "$RESULTS" "$OUT_DIR" "$DURATION" <<'PY'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
out, duration = sys.argv[2], sys.argv[3]
json.dump(rows, open(f"{out}/results.json", "w"), indent=1)

def other(r):
    parts = [f"{code}: {n}" for code, n in sorted(r.get("other_codes", {}).items())]
    parts += [f"{kind}: {n}" for kind, n in sorted((r.get("errors") or {}).items())]
    return ", ".join(parts) or "—"

lines = [f"Gateway API data planes, one kind node, {duration} per run, oha from the runner (keep-alive, no compression).", ""]
failed = [r["gateway"] for r in rows if r.get("failed")]
scenarios = []
for r in rows:
    if not r.get("failed") and r["scenario"] not in scenarios:
        scenarios.append(r["scenario"])
for scenario in scenarios:
    lines += [f"### {scenario}", "",
              "| Gateway | Conns | req/s | p50 ms | p99 ms | 200s % | CPU µs/req | Memory MiB | Restarts | Other answers |",
              "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for r in rows:
        if r.get("failed") or r["scenario"] != scenario:
            continue
        lines.append(f"| {r['gateway']} | {r['connections']} | {r['rps']} | {r['p50_ms']} | {r['p99_ms']} | {r['ok_pct']} | {r['cpu_us_per_req']} | {r['mem_mib']} | {r['restarts']} | {other(r)} |")
    lines.append("")
if failed:
    lines.append("Failed to install or route: " + ", ".join(failed))
open(f"{out}/results.md", "w").write("\n".join(lines) + "\n")
print("\n".join(lines))
PY
