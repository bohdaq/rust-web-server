#!/usr/bin/env bash
# The Gateway API conformance suite against rws in a kind cluster: Gateway API
# CRDs (standard channel), MetalLB on kind's Docker network so every Gateway's
# LoadBalancer Service gets an address the runner can reach, rws installed with
# the Helm chart (gatewayAPI.enabled), then the suite's own `go test`.
#
#   ci/e2e/gateway_api.sh <rws-binary> <gateway-api-source> <report.yaml> <profiles> <supported-features> [shared|provisioned|mesh]
#
# `mesh` runs the mesh profile (spec/GATEWAY_MESH.md): rws with the mesh on,
# its injection webhook selecting the suite's two mesh namespaces, and the
# rws-mesh-init image built from packaging/mesh-init.
#
# <gateway-api-source> is a checkout of kubernetes-sigs/gateway-api at the
# pinned tag; kind, kubectl, helm, docker and go must be on PATH.
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
GWAPI="$(cd "$2" && pwd)"
REPORT="$(cd "$(dirname "$3")" && pwd)/$(basename "$3")"
PROFILES="$4"
FEATURES="$5"
MODE="${6:-shared}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLUSTER=rws-gateway-api
METALLB_VERSION=v0.16.0

dump() {
  echo "── pods"; kubectl get pods -A -o wide || true
  if [ "$MODE" = mesh ]; then
    echo "── mesh sidecars"; kubectl logs -n gateway-conformance-mesh -l app=echo -c rws-mesh --tail=100 --prefix || true
    echo "── mesh init"; kubectl logs -n gateway-conformance-mesh -l app=echo -c rws-mesh-init --tail=20 --prefix || true
  fi
  echo "── rws logs"; kubectl logs -n rws-system -l app.kubernetes.io/name=rws --tail=300 --prefix || true
  echo "── gateway Services"; kubectl get services -A -l app.kubernetes.io/managed-by=rws-gateway -o wide || true
  echo "── gateways"; kubectl get gateways -A -o yaml | head -300 || true
}
trap dump ERR

# GATEWAY_API_CHANNEL=experimental: the experimental CRDs, and the
# ClusterTrustBundle API (beta: a feature gate and its API version on).
CHANNEL="${GATEWAY_API_CHANNEL:-standard}"
if [ "$CHANNEL" = experimental ]; then
  kind_config=$(mktemp)
  cat >"$kind_config" <<'KIND'
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
featureGates:
  ClusterTrustBundle: true
runtimeConfig:
  "certificates.k8s.io/v1beta1/clustertrustbundles": "true"
KIND
  kind create cluster --name "$CLUSTER" --config "$kind_config" --wait 120s
else
  kind create cluster --name "$CLUSTER" --wait 120s
fi
# kind marks its single node as excluded from external load balancers, and
# MetalLB honours that: without this no LoadBalancer address answers ARP.
kubectl label nodes --all node.kubernetes.io/exclude-from-external-load-balancers- || true
kubectl apply --server-side -f "$GWAPI/$CHANNEL-install.yaml"
kubectl wait --for=condition=Established crd --all --timeout=60s

# MetalLB, with addresses from the top of kind's IPv4 subnet.
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

# rws, from the binary built on the runner.
build=$(mktemp -d)
cp "$RWS" "$build/rws"
docker build -q -t rws:e2e -f "$ROOT/ci/e2e/gateway_api/Dockerfile" "$build"
kind load docker-image rws:e2e --name "$CLUSTER"
extra_values=()
gateway_mode="$MODE"
if [ "$MODE" = mesh ]; then
  gateway_mode=shared
  docker build -q -t rws-mesh-init:e2e "$ROOT/packaging/mesh-init"
  kind load docker-image rws-mesh-init:e2e --name "$CLUSTER"
  extra_values=(--set gatewayAPI.mesh.enabled=true
    --set gatewayAPI.mesh.injection.sidecarImage=rws:e2e
    --set gatewayAPI.mesh.injection.initImage=rws-mesh-init:e2e
    --set gatewayAPI.mesh.injection.namespaceSelector.matchLabels=null
    --set-json 'gatewayAPI.mesh.injection.namespaceSelector.matchExpressions=[{"key":"gateway-conformance","operator":"In","values":["mesh","mesh-consumer"]}]')
fi
[ "$CHANNEL" = experimental ] && extra_values+=(--set gatewayAPI.experimental=true)
helm install rws "$ROOT/helm/rws" --namespace rws-system --create-namespace \
  --set image.repository=rws --set image.tag=e2e --set image.pullPolicy=Never \
  --set gatewayAPI.enabled=true --set tls.enabled=false --set quic.enabled=false \
  --set gatewayAPI.mode="$gateway_mode" "${extra_values[@]}" \
  --set replicaCount=2 --set podDisruptionBudget.enabled=false --wait --timeout 5m
kubectl wait --for=condition=Accepted gatewayclass/rws --timeout=60s

# The experimental features the suite has no tests for yet (v1.7): checked
# here before they're claimed.
if [ "$CHANNEL" = experimental ]; then
  # ClusterTrustBundle is advertised only where the API is served.
  for _ in $(seq 1 30); do
    kubectl get gatewayclass rws -o jsonpath='{.status.supportedFeatures[*].name}' | tr ' ' '\n' | grep -qx ClusterTrustBundle && break
    sleep 2
  done
  kubectl get gatewayclass rws -o jsonpath='{.status.supportedFeatures[*].name}' | tr ' ' '\n' | grep -qx ClusterTrustBundle \
    || { echo "ClusterTrustBundle isn't advertised: is the API served?" >&2; exit 1; }
  # A Cluster-scoped address is the Gateway Service's ClusterIP.
  kubectl apply -f - <<'YAML'
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata: {name: routability-check, namespace: default}
spec:
  gatewayClassName: rws
  addresses: [{routability: Cluster}]
  listeners: [{name: http, port: 80, protocol: HTTP}]
YAML
  kubectl wait --for=condition=AddressesAssigned gateway/routability-check -n default --timeout=120s
  [ "$(kubectl get gateway routability-check -n default -o jsonpath='{.status.addresses[0].routability}')" = Cluster ] \
    || { echo "the Cluster address wasn't reported" >&2; exit 1; }
  kubectl delete gateway routability-check -n default --wait=false
fi

cd "$GWAPI/conformance"
version=$(git -C "$ROOT" describe --tags --always)
go test -v -timeout 150m -run TestConformance . -args \
  --gateway-class=rws \
  --conformance-profiles="$PROFILES" \
  --supported-features="$FEATURES" \
  --report-output="$REPORT" \
  --organization=bohdaq --project=rust-web-server --url=https://rws8.tech \
  --version="$version" --mode="$MODE" --usable-address="$prefix.255.240" --unusable-address=10.255.255.1 \
  --contact=@bohdaq
