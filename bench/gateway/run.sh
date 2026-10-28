#!/usr/bin/env bash
# Benchmark Kubernetes Gateway API implementations against each other on one
# machine, one cluster, one backend, one load: rws and the gateways named in
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
# at its chart defaults with one data-plane replica, access logs as they ship;
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
  1k.bin: "$(head -c 1024 /dev/zero | tr '\0' 'x')"
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
      containers:
        - name: nginx
          image: $NGINX_IMAGE
          args: ["nginx", "-c", "/etc/backend/nginx.conf", "-g", "daemon off;"]
          ports: [{containerPort: 8080}]
          readinessProbe: {httpGet: {path: /1k.bin, port: 8080}}
          volumeMounts:
            - {name: conf, mountPath: /etc/backend}
            - {name: conf, mountPath: /srv}
      volumes: [{name: conf, configMap: {name: backend}}]
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

# ── one gateway ──────────────────────────────────────────────────────────────

# A Gateway of <class> in namespace bench on <port>, and the route to the backend.
gateway_and_route() { # <class> <port>
  kubectl apply -n bench -f - <<YAML
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata: {name: bench}
spec:
  gatewayClassName: $1
  listeners: [{name: http, port: $2, protocol: HTTP}]
---
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata: {name: bench}
spec:
  parentRefs: [{name: bench}]
  rules: [{backendRefs: [{name: backend, namespace: backend, port: 8080}]}]
YAML
  kubectl wait -n bench gateway/bench --for=condition=Programmed --timeout=300s
}

gateway_address() {
  kubectl get -n bench gateway/bench -o jsonpath='{.status.addresses[0].value}'
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

# Load <url> at each connection count; one JSON line per run into RESULTS.
measure() { # <name> <url> <namespaces…>
  local name="$1" url="$2"; shift 2
  for _ in $(seq 1 60); do curl -fsS -o /dev/null "$url" && break || sleep 2; done
  "$OHA" -z 5s -c 64 --no-tui --disable-compression "$url" > /dev/null 2>&1 || true   # warm-up
  for c in $CONNECTIONS; do
    read -r cpu_before _ < <(pod_usage "$@")
    "$OHA" -z "$DURATION" -c "$c" --no-tui --disable-compression --output-format json "$url" > "$WORK/oha.json"
    sleep 1
    read -r cpu_after mem_after < <(pod_usage "$@")
    python3 -I - "$name" "$c" "$WORK/oha.json" "$((cpu_after - cpu_before))" "$mem_after" >> "$RESULTS" <<'EOF'
import json, sys
name, conns, path, cpu_ns, mem = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
d = json.load(open(path))
codes = d.get("statusCodeDistribution", {})
total = sum(codes.values()) or 1
print(json.dumps({
    "gateway": name, "connections": conns,
    "rps": round(d["summary"]["requestsPerSec"]),
    "p50_ms": round(d["latencyPercentiles"]["p50"] * 1000, 2),
    "p99_ms": round(d["latencyPercentiles"]["p99"] * 1000, 2),
    "ok_pct": round(100 * codes.get("200", 0) / total, 2),
    "cpu_us_per_req": round(cpu_ns / 1000 / total, 1),
    "mem_mib": round(mem / 2**20, 1),
}))
EOF
    log "$name c=$c: $(tail -1 "$RESULTS")"
  done
}

cleanup_bench() {
  kubectl delete -n bench gateway/bench httproute/bench --ignore-not-found --wait=true --timeout=120s || true
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
    --wait --timeout 5m
  kubectl wait gatewayclass/rws --for=condition=Accepted --timeout=120s
  gateway_and_route rws 80
  measure rws "http://$(gateway_address)/1k.bin" rws-system bench
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
  gateway_and_route envoy-gateway 80
  measure envoy-gateway "http://$(gateway_address)/1k.bin" envoy-gateway-system bench
  cleanup_bench
  kubectl delete gatewayclass envoy-gateway --wait=true
  helm uninstall eg -n envoy-gateway-system --wait
}

bench_nginx_gateway_fabric() {
  helm install ngf oci://ghcr.io/nginx/charts/nginx-gateway-fabric --version "$NGF_VERSION" \
    -n nginx-gateway --create-namespace --wait --timeout 5m
  kubectl wait gatewayclass/nginx --for=condition=Accepted --timeout=120s
  gateway_and_route nginx 80
  # NGF provisions the data plane in the Gateway's namespace (bench).
  measure nginx-gateway-fabric "http://$(gateway_address)/1k.bin" nginx-gateway bench
  cleanup_bench
  helm uninstall ngf -n nginx-gateway --wait
}

bench_traefik() {
  helm repo add traefik https://traefik.github.io/charts > /dev/null
  helm install traefik traefik/traefik --version "$TRAEFIK_CHART_VERSION" -n traefik --create-namespace \
    --set providers.kubernetesGateway.enabled=true --set providers.kubernetesIngress.enabled=false \
    --set gateway.enabled=false --set gatewayClass.enabled=true --wait --timeout 5m
  kubectl wait gatewayclass/traefik --for=condition=Accepted --timeout=120s
  # Traefik's listeners are its entrypoints: the web entrypoint listens on
  # 8000, its Service maps 80 to it.
  gateway_and_route traefik 8000
  local address; address=$(kubectl get -n traefik service traefik -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
  measure traefik "http://$address/1k.bin" traefik bench
  cleanup_bench
  helm uninstall traefik -n traefik --wait
}

bench_kgateway() {
  helm install kgateway-crds oci://cr.kgateway.dev/kgateway-dev/charts/kgateway-crds --version "$KGATEWAY_VERSION" \
    -n kgateway-system --create-namespace --wait --timeout 5m
  helm install kgateway oci://cr.kgateway.dev/kgateway-dev/charts/kgateway --version "$KGATEWAY_VERSION" \
    -n kgateway-system --wait --timeout 5m
  kubectl wait gatewayclass/kgateway --for=condition=Accepted --timeout=120s
  gateway_and_route kgateway 80
  measure kgateway "http://$(gateway_address)/1k.bin" kgateway-system bench
  cleanup_bench
  helm uninstall kgateway -n kgateway-system --wait
}

bench_istio() {
  local dir; dir=$(mktemp -d)
  curl -fsSL "https://github.com/istio/istio/releases/download/$ISTIO_VERSION/istioctl-$ISTIO_VERSION-linux-amd64.tar.gz" | tar xz -C "$dir"
  "$dir/istioctl" install --set profile=minimal -y
  kubectl wait gatewayclass/istio --for=condition=Accepted --timeout=180s
  gateway_and_route istio 80
  measure istio "http://$(gateway_address)/1k.bin" istio-system bench
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

python3 -I - "$RESULTS" "$OUT_DIR" "$DURATION" <<'EOF'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
out, duration = sys.argv[2], sys.argv[3]
json.dump(rows, open(f"{out}/results.json", "w"), indent=1)
lines = [f"Gateway API data planes, HTTP/1.1 keep-alive GET of a 1 KiB file through each, {duration} per run, one kind node.", "",
         "| Gateway | Conns | req/s | p50 ms | p99 ms | 200s % | CPU µs/req | Memory MiB |",
         "|---|---:|---:|---:|---:|---:|---:|---:|"]
for r in rows:
    if r.get("failed"):
        lines.append(f"| {r['gateway']} | — | failed | | | | | |")
    else:
        lines.append(f"| {r['gateway']} | {r['connections']} | {r['rps']} | {r['p50_ms']} | {r['p99_ms']} | {r['ok_pct']} | {r['cpu_us_per_req']} | {r['mem_mib']} |")
open(f"{out}/results.md", "w").write("\n".join(lines) + "\n")
print("\n".join(lines))
EOF
