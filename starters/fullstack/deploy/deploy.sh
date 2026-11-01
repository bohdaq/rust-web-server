#!/usr/bin/env bash
# Deploy the app to a kube-dc project and print its HTTPS URL.
#
#   deploy/deploy.sh <namespace> <app> <image>
#
# Uses the current kubectl context (CI writes one with a short-lived token).
# Optional: DB_SECRET (default <app>-db-owner), PULL_USER + PULL_TOKEN to
# create/refresh the <app>-pull image pull Secret for a private registry,
# TIMEOUT (rollout, hostname and URL wait, default 300s), HEALTH_PATH
# (checked on the live URL, default /healthz), and for a preview
# PREVIEW_HOURS (kube-dc removes it that long after the last deploy),
# PREVIEW_REPOSITORY + PREVIEW_PULL_REQUEST (and when that pull request
# closes; public repositories), and PREVIEW_DATABASE: a plan such as
# postgresql-development for a fresh, empty database of the preview's own
# (<app>-db, removed with the preview; the app gets <app>-db-owner).
set -euo pipefail

NAMESPACE="$1" APP="$2" IMAGE="$3"
export APP IMAGE DB_SECRET="${DB_SECRET:-$APP-db-owner}"
TIMEOUT="${TIMEOUT:-300}"
cd "$(dirname "$0")"

if [ -n "${PREVIEW_DATABASE:-}" ]; then
  # A fresh, empty database of the preview's own, labelled like the rest of
  # it so it goes when the preview goes; Delete frees its data and capacity.
  db="$APP-db"
  label="${PREVIEW_HOURS:+preview.kube-dc.com/name: $APP}"
  kubectl -n "$NAMESPACE" apply -f - <<MANIFEST
apiVersion: services.kube-dc.com/v1alpha1
kind: ManagedService
metadata:
  name: $db
  labels: { $label }
spec:
  classRef: { name: ${PREVIEW_DATABASE%%-*} }
  planRef: { name: $PREVIEW_DATABASE }
  placement: { mode: ProviderShared }
  connectivity: { classRef: { name: tenant-native } }
  topology: { instances: 1 }
  deletionPolicy: Delete
MANIFEST
  uid=$(kubectl -n "$NAMESPACE" get managedservice "$db" -o jsonpath='{.metadata.uid}')
  kubectl -n "$NAMESPACE" apply -f - <<MANIFEST
apiVersion: services.kube-dc.com/v1alpha1
kind: ServiceBinding
metadata:
  name: $db-owner
  labels: { $label }
spec:
  serviceRef: { name: $db }
  serviceUID: $uid
  role: owner
  consumer: { kind: Namespace, name: $NAMESPACE, namespace: $NAMESPACE }
  delivery: { mode: DataPlaneLocalSecret, secretName: $db-owner }
MANIFEST
  DB_SECRET="$db-owner"
  # A new database takes a few minutes; the app starts once its credentials exist.
  echo "Waiting for the preview database $db..."
  for _ in $(seq 1 $((${DB_TIMEOUT:-600} / 10))); do
    kubectl -n "$NAMESPACE" get secret "$DB_SECRET" >/dev/null 2>&1 && break
    sleep 10
  done
  if ! kubectl -n "$NAMESPACE" get secret "$DB_SECRET" >/dev/null 2>&1; then
    echo "The preview database $db is not ready:"
    kubectl -n "$NAMESPACE" get managedservice "$db" -o jsonpath='{range .status.conditions[*]}{.type}={.status} {.reason}: {.message}{"\n"}{end}'
    exit 1
  fi
fi

if [ -n "${PULL_TOKEN:-}" ]; then
  kubectl -n "$NAMESPACE" create secret docker-registry "$APP-pull" \
    --docker-server="${IMAGE%%/*}" --docker-username="${PULL_USER:?PULL_USER}" --docker-password="$PULL_TOKEN" \
    --dry-run=client -o yaml | kubectl -n "$NAMESPACE" apply -f -
fi

envsubst '${APP} ${IMAGE} ${DB_SECRET}' < app.yaml | kubectl -n "$NAMESPACE" apply -f -

if [ -n "${PREVIEW_HOURS:-}" ]; then
  # Every object of the preview carries its name; the Deployment, when and why it goes.
  kubectl -n "$NAMESPACE" label --overwrite deployment/"$APP" service/"$APP" preview.kube-dc.com/name="$APP" >/dev/null
  [ -n "${PULL_TOKEN:-}" ] && kubectl -n "$NAMESPACE" label --overwrite secret/"$APP-pull" preview.kube-dc.com/name="$APP" >/dev/null
  kubectl -n "$NAMESPACE" annotate --overwrite deployment/"$APP" \
    preview.kube-dc.com/expires-at="$(date -u -d "+${PREVIEW_HOURS} hours" +%Y-%m-%dT%H:%M:%SZ)" \
    ${PREVIEW_REPOSITORY:+preview.kube-dc.com/repository="$PREVIEW_REPOSITORY"} \
    ${PREVIEW_PULL_REQUEST:+preview.kube-dc.com/pull-request="$PREVIEW_PULL_REQUEST"} >/dev/null
fi

if ! kubectl -n "$NAMESPACE" rollout status "deployment/$APP" --timeout="${TIMEOUT}s"; then
  echo "Rollout failed. Recent events and logs:"
  kubectl -n "$NAMESPACE" get events --sort-by=.lastTimestamp | tail -15 || true
  kubectl -n "$NAMESPACE" logs "deployment/$APP" --tail=30 || true
  exit 1
fi

# The platform writes the hostname once the certificate is issued.
for _ in $(seq 1 $((TIMEOUT / 5))); do
  host=$(kubectl -n "$NAMESPACE" get service "$APP" -o jsonpath='{.metadata.annotations.service\.nlb\.kube-dc\.com/route-hostname-status}')
  [ -n "$host" ] && break
  sleep 5
done
if [ -z "$host" ]; then
  echo "The Service has no hostname yet; check: kubectl -n $NAMESPACE get events"
  exit 1
fi

# Green only when the live URL answers (certificate served, route programmed,
# app healthy), so the GitHub deployment shows what users get.
for _ in $(seq 1 $((TIMEOUT / 5))); do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "https://$host${HEALTH_PATH:-/healthz}" || true)
  if [ "$code" -ge 200 ] 2>/dev/null && [ "$code" -lt 400 ]; then
    echo "url=https://$host"
    [ -n "${GITHUB_OUTPUT:-}" ] && echo "url=https://$host" >> "$GITHUB_OUTPUT"
    exit 0
  fi
  sleep 5
done
echo "https://$host${HEALTH_PATH:-/healthz} did not answer (last status: ${code:-none})."
kubectl -n "$NAMESPACE" logs "deployment/$APP" --tail=30 || true
exit 1
