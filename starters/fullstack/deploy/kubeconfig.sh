#!/usr/bin/env bash
# Write a kubeconfig for a kube-dc machine identity (client credentials) and
# print its path. Inputs (environment): KUBE_DC_API_SERVER, KUBE_DC_TOKEN_URL,
# KUBE_DC_CLIENT_ID, KUBE_DC_CLIENT_SECRET; optional KUBE_DC_CA_CERT (PEM)
# when the API server's certificate is not publicly trusted.
set -euo pipefail

token=$(curl -fsS "$KUBE_DC_TOKEN_URL" \
  -d grant_type=client_credentials \
  --data-urlencode "client_id=$KUBE_DC_CLIENT_ID" \
  --data-urlencode "client_secret=$KUBE_DC_CLIENT_SECRET" | jq -r .access_token)
[ -n "$token" ] && [ "$token" != null ] || { echo "token request refused" >&2; exit 1; }

config="${RUNNER_TEMP:-/tmp}/kube-dc.kubeconfig"
export KUBECONFIG="$config"
if [ -n "${KUBE_DC_CA_CERT:-}" ]; then
  printf '%s\n' "$KUBE_DC_CA_CERT" > "$config.ca"
  kubectl config set-cluster kube-dc --server="$KUBE_DC_API_SERVER" --certificate-authority="$config.ca" --embed-certs >/dev/null
else
  kubectl config set-cluster kube-dc --server="$KUBE_DC_API_SERVER" >/dev/null
fi
kubectl config set-credentials ci --token="$token" >/dev/null
kubectl config set-context kube-dc --cluster=kube-dc --user=ci >/dev/null
kubectl config use-context kube-dc >/dev/null
chmod 600 "$config"
echo "$config"
