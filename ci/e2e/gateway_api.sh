#!/usr/bin/env bash
# The Gateway API conformance suite against rws in a kind cluster: Gateway API
# CRDs (standard channel), MetalLB on kind's Docker network so every Gateway's
# LoadBalancer Service gets an address the runner can reach, rws installed with
# the Helm chart (gatewayAPI.enabled), then the suite's own `go test`.
#
#   ci/e2e/gateway_api.sh <rws-binary> <gateway-api-source> <report.yaml> <profiles> <supported-features>
#
# <gateway-api-source> is a checkout of kubernetes-sigs/gateway-api at the
# pinned tag; kind, kubectl, helm, docker and go must be on PATH.
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
GWAPI="$(cd "$2" && pwd)"
REPORT="$(cd "$(dirname "$3")" && pwd)/$(basename "$3")"
PROFILES="$4"
FEATURES="$5"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLUSTER=rws-gateway-api
METALLB_VERSION=v0.16.0

dump() {
  echo "── rws pods"; kubectl get pods -A -o wide || true
  echo "── rws logs"; kubectl logs -l app.kubernetes.io/name=rws --tail=200 --prefix || true
  echo "── gateways"; kubectl get gateways -A -o yaml | head -200 || true
}
trap dump ERR

kind create cluster --name "$CLUSTER" --wait 120s
kubectl apply --server-side -f "$GWAPI/standard-install.yaml"
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
helm install rws "$ROOT/helm/rws" --namespace rws-system --create-namespace \
  --set image.repository=rws --set image.tag=e2e --set image.pullPolicy=Never \
  --set gatewayAPI.enabled=true --set tls.enabled=false --set quic.enabled=false \
  --set replicaCount=2 --set podDisruptionBudget.enabled=false --wait --timeout 5m
kubectl wait --for=condition=Accepted gatewayclass/rws --timeout=60s

cd "$GWAPI/conformance"
version=$(git -C "$ROOT" describe --tags --always)
go test -timeout 90m -run TestConformance . -args \
  --gateway-class=rws \
  --conformance-profiles="$PROFILES" \
  --supported-features="$FEATURES" \
  --report-output="$REPORT" \
  --organization=bohdaq --project=rust-web-server --url=https://rws8.tech \
  --version="$version" --contact=@bohdaq
