#!/usr/bin/env bash
# Deploy the app to a kube-dc project and print its HTTPS URL.
#
#   deploy/deploy.sh <namespace> <app> <image>
#
# Uses the current kubectl context (CI writes one with a short-lived token).
# Optional: DB_SECRET (default <app>-db-owner), PULL_USER + PULL_TOKEN to
# create/refresh the <app>-pull image pull Secret for a private registry,
# TIMEOUT (rollout and hostname wait, default 300s).
set -euo pipefail

NAMESPACE="$1" APP="$2" IMAGE="$3"
export APP IMAGE DB_SECRET="${DB_SECRET:-$APP-db-owner}"
TIMEOUT="${TIMEOUT:-300}"
cd "$(dirname "$0")"

if [ -n "${PULL_TOKEN:-}" ]; then
  kubectl -n "$NAMESPACE" create secret docker-registry "$APP-pull" \
    --docker-server="${IMAGE%%/*}" --docker-username="${PULL_USER:?PULL_USER}" --docker-password="$PULL_TOKEN" \
    --dry-run=client -o yaml | kubectl -n "$NAMESPACE" apply -f -
fi

envsubst '${APP} ${IMAGE} ${DB_SECRET}' < app.yaml | kubectl -n "$NAMESPACE" apply -f -

if ! kubectl -n "$NAMESPACE" rollout status "deployment/$APP" --timeout="${TIMEOUT}s"; then
  echo "Rollout failed. Recent events and logs:"
  kubectl -n "$NAMESPACE" get events --sort-by=.lastTimestamp | tail -15 || true
  kubectl -n "$NAMESPACE" logs "deployment/$APP" --tail=30 || true
  exit 1
fi

# The platform writes the hostname once the certificate is issued.
for _ in $(seq 1 $((TIMEOUT / 5))); do
  host=$(kubectl -n "$NAMESPACE" get service "$APP" -o jsonpath='{.metadata.annotations.service\.nlb\.kube-dc\.com/route-hostname-status}')
  if [ -n "$host" ]; then
    echo "url=https://$host"
    [ -n "${GITHUB_OUTPUT:-}" ] && echo "url=https://$host" >> "$GITHUB_OUTPUT"
    exit 0
  fi
  sleep 5
done
echo "The Service has no hostname yet; check: kubectl -n $NAMESPACE get events"
exit 1
