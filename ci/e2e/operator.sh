#!/usr/bin/env bash
# starters/operator in a real cluster: its image, its own manifests
# (`--manifests`), then what the API server does with them — the validating
# webhook refuses a Greeting without a message (the API server calling the
# operator over TLS with the CA the operator put into the configuration at
# startup), the mutating webhook labels the rest, the controller writes the
# owned ConfigMap and the status, deletion runs the finalizer, and one of two
# replicas holds the Lease.
#
#   ci/e2e/operator.sh <kind|k3d> <cluster name>     (KUBECONFIG: that cluster)
#
# The cluster must be a throwaway one: the script installs a CRD, a
# ClusterRole and webhook configurations.
set -euo pipefail

TOOL="$1" CLUSTER="$2"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
NS=greeter-e2e
IMAGE=rws-starter-operator:e2e
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/starters}"
fail() {
  echo "FAIL: $*"
  kubectl -n "$NS" get pods,events 2>/dev/null | tail -30 || true
  kubectl -n "$NS" logs -l app.kubernetes.io/name=rws-starter-operator --tail=60 --prefix 2>/dev/null || true
  exit 1
}
cleanup() {
  kubectl delete ns "$NS" --wait=false >/dev/null 2>&1 || true
  kubectl delete validatingwebhookconfiguration,mutatingwebhookconfiguration rws-starter-operator >/dev/null 2>&1 || true
  kubectl delete crd greetings.example.com --wait=false >/dev/null 2>&1 || true
  kubectl delete clusterrole,clusterrolebinding rws-starter-operator >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

# The starter, rendered as cargo-generate would and pinned to this checkout.
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)
cp -r "$ROOT/starters/operator" "$WORK/op"
grep -rl '{{project-name}}' "$WORK/op" | xargs sed -i "s/{{project-name}}/rws-starter-operator/g"
grep -rl '{{crate_name}}' "$WORK/op" | xargs -r sed -i "s/{{crate_name}}/rws_starter_operator/g"
sed -i -E "s/^rust-web-server = \{ version = \"[^\"]+\"/rust-web-server = { version = \"=$VERSION\"/" "$WORK/op/Cargo.toml"
{
  echo; echo "[patch.crates-io]"
  echo "rust-web-server = { path = \"$ROOT\" }"
  echo "rws-macros = { path = \"$ROOT/rws-macros\" }"
  for c in "$ROOT"/crates/*/; do echo "$(basename "$c") = { path = \"$c\" }"; done
} >> "$WORK/op/Cargo.toml"
(cd "$WORK/op" && cargo build --quiet --release)
echo "ok: the operator builds"

# The image: the binary on the host's own distribution (same glibc).
. /etc/os-release
mkdir "$WORK/img"
cp "$CARGO_TARGET_DIR/release/operator" "$WORK/img/operator"
cat > "$WORK/img/Dockerfile" <<DOCKER
FROM ${ID}:${VERSION_ID}
COPY operator /operator
USER 65532
ENTRYPOINT ["/operator"]
DOCKER
docker build --quiet -t "$IMAGE" "$WORK/img" >/dev/null
case "$TOOL" in
  kind) kind load docker-image "$IMAGE" --name "$CLUSTER" >/dev/null ;;
  k3d) k3d image import "$IMAGE" -c "$CLUSTER" >/dev/null ;;
  *) echo "usage: $0 <kind|k3d> <cluster>"; exit 2 ;;
esac
echo "ok: the image is in the cluster"

kubectl create namespace "$NS" >/dev/null
"$CARGO_TARGET_DIR/release/operator" --manifests --image "$IMAGE" --namespace "$NS" > "$WORK/manifests.yaml"
kubectl apply -f "$WORK/manifests.yaml" >/dev/null
kubectl wait --for condition=established crd/greetings.example.com --timeout=60s >/dev/null
kubectl -n "$NS" rollout status deploy/rws-starter-operator --timeout=180s >/dev/null || fail "the operator didn't become ready"
echo "ok: --manifests applied; two replicas ready (the webhook CA installed)"

greeting() { # <name> <message>
  printf 'apiVersion: example.com/v1\nkind: Greeting\nmetadata:\n  name: %s\n  namespace: %s\nspec:\n  message: "%s"\n' "$1" "$NS" "$2"
}
if out=$(greeting empty "" | kubectl apply -f - 2>&1); then fail "a Greeting without a message was accepted"; fi
echo "$out" | grep -q "spec.message is required" || fail "not the webhook's refusal: $out"
echo "ok: the API server called the validating webhook: $(echo "$out" | sed -n 's/.*denied the request: //p')"

greeting hello "hi there" | kubectl apply -f - >/dev/null
[ "$(kubectl -n "$NS" get greeting hello -o jsonpath='{.metadata.labels.example\.com/greeted}')" = true ] || fail "the mutating webhook didn't label it"
echo "ok: the mutating webhook labelled it"
for _ in $(seq 1 60); do
  msg=$(kubectl -n "$NS" get configmap hello-greeting -o jsonpath='{.data.message}' 2>/dev/null || true)
  ready=$(kubectl -n "$NS" get greeting hello -o jsonpath='{.status.ready}' 2>/dev/null || true)
  [ "$msg" = "hi there" ] && [ "$ready" = true ] && break
  sleep 1
done
[ "$msg" = "hi there" ] && [ "$ready" = true ] || fail "no ConfigMap / status (message '$msg', ready '$ready')"
[ "$(kubectl -n "$NS" get configmap hello-greeting -o jsonpath='{.metadata.ownerReferences[0].kind}')" = Greeting ] || fail "the ConfigMap isn't owned"
echo "ok: the controller wrote the owned ConfigMap and the status"

holder=$(kubectl -n "$NS" get lease rws-starter-operator -o jsonpath='{.spec.holderIdentity}')
kubectl -n "$NS" get pod "$holder" >/dev/null 2>&1 || fail "the Lease holder '$holder' isn't one of the pods"
echo "ok: $holder leads"

kubectl -n "$NS" delete greeting hello --timeout=60s >/dev/null || fail "the Greeting wasn't deleted (finalizer stuck?)"
for _ in $(seq 1 30); do kubectl -n "$NS" get configmap hello-greeting >/dev/null 2>&1 || break; sleep 1; done
kubectl -n "$NS" get configmap hello-greeting >/dev/null 2>&1 && fail "the owned ConfigMap outlived the Greeting"
echo "ok: deleting ran the finalizer and took the ConfigMap"
echo "operator: all end-to-end checks passed"
