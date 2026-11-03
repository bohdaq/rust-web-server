#!/usr/bin/env bash
# rws end to end on kube-dc stage (design: the "rws on kube-dc stage — E2E
# test design" doc; findings: spec/STAGE_E2E_FINDINGS.md).
#
# One run = one throwaway kube-dc project, rws from the checkout's Helm chart
# with 3 replicas behind the shared gateway (bucket A: kube-dc terminates TLS),
# the suites, a report, then the project is deleted.
#
#   ci/stage/run.sh [--suites S0,S1,…|all] [--image <tag>] [--keep] [--project <name>]
#
#   --suites    which suites (default: all that are written)
#   --image     rws image tag (default: the chart's appVersion)
#   --keep      leave the project for debugging (prints the delete command)
#   --project   reuse an existing project made by a --keep run
#
# Needs: kubectl and helm, KUBECONFIG for stage (default ~/.kube/stage_config).
set -euo pipefail

STAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$STAGE_DIR/../.." && pwd)"
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/stage_config}"
ORG=shalb
SUITES=all IMAGE_TAG="" KEEP=0 PROJECT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --suites) SUITES=$2; shift ;;
    --image) IMAGE_TAG=$2; shift ;;
    --keep) KEEP=1 ;;
    --project) PROJECT=$2; KEEP=1; shift ;;
    -h|--help) sed -n 2,16p "$0"; exit 0 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

RUN_ID=$(date -u +%Y%m%d%H%M%S)
CREATED=0
if [ -z "$PROJECT" ]; then PROJECT="rws-e2e-$RUN_ID"; CREATED=1; fi
export NS="$ORG-$PROJECT"
export REPORT="$ROOT/ci/stage/report/$RUN_ID"
export STAGE_DIR ROOT
mkdir -p "$REPORT"
: >"$REPORT/results.tsv"
log() { printf '== %s\n' "$*"; }
. "$STAGE_DIR/lib.sh"

cleanup() {
  local rc=$?
  # Logs of every rws pod, whatever happened.
  for pod in $(k get pods -o name 2>/dev/null); do
    k logs "$pod" --all-containers --tail=2000 >"$REPORT/${pod#pod/}.log" 2>&1 || true
  done
  k get events --sort-by=.lastTimestamp >"$REPORT/events.txt" 2>&1 || true
  if [ "$KEEP" = 1 ]; then
    log "kept: kubectl -n $ORG delete projects.kube-dc.com $PROJECT"
  elif [ "$CREATED" = 1 ]; then
    log "deleting project $PROJECT"
    kubectl -n "$ORG" delete projects.kube-dc.com "$PROJECT" --wait=false >/dev/null 2>&1 || true
    if eventually 300 sh -c "! kubectl get ns $NS >/dev/null 2>&1"; then
      log "namespace $NS gone"
    else
      log "WARNING: namespace $NS still there after 5 min"; rc=1
    fi
  fi
  summary
  exit "$rc"
}

summary() {
  local f="$REPORT/summary.md" n
  {
    echo "# Stage E2E run $RUN_ID"
    echo
    echo "Project \`$PROJECT\`, image \`${IMAGE_TAG:-chart default}\`, rws at $RWS_URL."
    echo
    for s in ok FAIL BLOCKED KNOWN; do
      n=$(awk -F'\t' -v s=$s '$2 == s' "$REPORT/results.tsv" | wc -l)
      printf '%s %s · ' "$n" "$s"
    done
    echo
    echo
    echo "| Case | Result | Check |"
    echo "|---|---|---|"
    awk -F'\t' '{printf "| %s | %s | %s |\n", $1, $2, $3}' "$REPORT/results.tsv"
  } >"$f"
  log "report: $f"
  awk -F'\t' '{c[$2]++} END {for (s in c) printf "%s=%d ", s, c[s]; print ""}' "$REPORT/results.tsv"
}

# ── project ───────────────────────────────────────────────────────────────────
RWS_URL=""
trap cleanup EXIT

if [ "$CREATED" = 1 ]; then
  # A /22 no project uses (the reconciler refuses overlaps in an org).
  used=$(kubectl get projects.kube-dc.com -A -o jsonpath='{range .items[*]}{.spec.cidrBlock}{"\n"}{end}')
  CIDR=""
  for third in 100 104 108 112 116 120 124; do
    c="10.0.$third.0/22"
    if ! grep -qx "$c" <<<"$used"; then CIDR=$c; break; fi
  done
  [ -n "$CIDR" ] || { blocked S0.0 "no free CIDR in the pool"; exit 1; }
  log "project $PROJECT ($CIDR)"
  kubectl apply -f - >/dev/null <<YAML
apiVersion: kube-dc.com/v1
kind: Project
metadata:
  name: $PROJECT
  namespace: $ORG
  labels:
    rws8.tech/stage-e2e: "$RUN_ID"
spec:
  cidrBlock: $CIDR
  egressNetworkType: cloud
YAML
  if ! eventually 300 sh -c "kubectl get ns $NS >/dev/null 2>&1 && kubectl -n $ORG get projects.kube-dc.com $PROJECT -o jsonpath='{.metadata.annotations.kube-dc\.com/namespace-uid}' | grep -q ."; then
    blocked S0.0 "project $PROJECT never got its namespace"; exit 1
  fi
fi

# ── deploy ────────────────────────────────────────────────────────────────────
# Per-run secrets for the auth routes (the suites read them from the env).
rand() { head -c 24 /dev/urandom | base64 | tr -d '/+='; }
export E2E_BEARER=$(rand) E2E_JWT_SECRET=$(rand) E2E_BASIC_PASS=$(rand)

log "fixtures and backends"
"$STAGE_DIR/deploy.sh"
k create secret generic rws-e2e-auth --from-literal=htpasswd="e2e:{SHA256}$(printf '%s' "$E2E_BASIC_PASS" | openssl dgst -sha256 -binary | openssl base64)" \
  --dry-run=client -o yaml | k apply -f - >/dev/null

log "rws (helm)"
helm_args=(--namespace "$NS" --set replicaCount=3 --set tls.enabled=false --set quic.enabled=false
  --set podDisruptionBudget.enabled=false
  --set-string 'service.annotations.service\.nlb\.kube-dc\.com/expose-route=https'
  --set-file configFile="$STAGE_DIR/config/rws.config.toml"
  --set 'extraVolumes[0].name=www' --set 'extraVolumes[0].configMap.name=rws-e2e-www'
  --set 'extraVolumeMounts[0].name=www' --set 'extraVolumeMounts[0].mountPath=/srv'
  --set 'extraVolumeMounts[0].readOnly=true'
  --set 'extraVolumes[1].name=auth' --set 'extraVolumes[1].secret.secretName=rws-e2e-auth'
  --set 'extraVolumeMounts[1].name=auth' --set 'extraVolumeMounts[1].mountPath=/etc/rws-e2e'
  --set-string env.E2E_BEARER="$E2E_BEARER" --set-string env.E2E_JWT_SECRET="$E2E_JWT_SECRET")
[ -n "$IMAGE_TAG" ] && helm_args+=(--set image.tag="$IMAGE_TAG")
helm upgrade --install rws "$ROOT/helm/rws" "${helm_args[@]}" >/dev/null
if ! k rollout status deploy/rws --timeout=300s >/dev/null 2>&1; then
  if k get pods -l app.kubernetes.io/name=rws -o jsonpath='{.items[*].status.conditions[?(@.reason=="Unschedulable")].message}' | grep -q .; then
    blocked S0.1 "rws pods unschedulable (stage capacity)"; exit 1
  fi
  fail S0.1 "rws rollout didn't finish"; exit 1
fi
pass S0.1 "rws: 3 replicas ready"

# The gateway hostname, as kube-dc reports it on the Service.
if ! eventually 300 sh -c "kubectl -n $NS get svc rws -o jsonpath='{.metadata.annotations.service\.nlb\.kube-dc\.com/route-hostname-status}' | grep -q '\.'"; then
  blocked S0.2 "the gateway gave the Service no hostname"; exit 1
fi
HOST=$(k get svc rws -o jsonpath='{.metadata.annotations.service\.nlb\.kube-dc\.com/route-hostname-status}')
export RWS_URL="https://$HOST"
if eventually 300 sh -c "curl -fsS --max-time 10 $RWS_URL/healthz >/dev/null"; then
  pass S0.2 "reachable through the gateway at $RWS_URL"
else
  blocked S0.2 "$RWS_URL/healthz not reachable through the gateway"; exit 1
fi

# ── suites ────────────────────────────────────────────────────────────────────
for file in "$STAGE_DIR"/suites/S*.sh; do
  name=$(basename "$file" .sh); id=${name%%-*}
  if [ "$SUITES" != all ] && ! grep -qw "$id" <<<"${SUITES//,/ }"; then continue; fi
  log "suite $name"
  # Each suite runs in a subshell: one suite's `exit` can't end the run.
  ( . "$file" ) || fail "$id" "suite aborted (exit $?)"
done
