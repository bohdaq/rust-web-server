#!/usr/bin/env bash
# Write a kubeconfig for a kube-dc machine identity. In GitHub Actions it
# masks the token and exports KUBECONFIG for later steps; elsewhere it prints
# the path: KUBECONFIG=$(deploy/kubeconfig.sh).
# Inputs (environment): KUBE_DC_API_SERVER, KUBE_DC_TOKEN_URL, KUBE_DC_CLIENT_ID;
# optional KUBE_DC_CA_CERT (PEM) when the API server's certificate is not
# publicly trusted. Sign-in:
#   - keyless (an identity with spec.github): KUBE_DC_AUDIENCE set; the
#     workflow needs `permissions: id-token: write`;
#   - otherwise KUBE_DC_CLIENT_SECRET.
set -euo pipefail

if [ -n "${KUBE_DC_AUDIENCE:-}" ]; then
  assertion=$(curl -fsS -H "Authorization: bearer $ACTIONS_ID_TOKEN_REQUEST_TOKEN" \
    "$ACTIONS_ID_TOKEN_REQUEST_URL&audience=$KUBE_DC_AUDIENCE" | jq -r .value)
  token=$(curl -fsS "$KUBE_DC_TOKEN_URL" \
    -d grant_type=client_credentials \
    --data-urlencode "client_id=$KUBE_DC_CLIENT_ID" \
    -d client_assertion_type=urn:ietf:params:oauth:client-assertion-type:jwt-bearer \
    --data-urlencode "client_assertion=$assertion" | jq -r .access_token)
else
  token=$(curl -fsS "$KUBE_DC_TOKEN_URL" \
    -d grant_type=client_credentials \
    --data-urlencode "client_id=$KUBE_DC_CLIENT_ID" \
    --data-urlencode "client_secret=${KUBE_DC_CLIENT_SECRET:?KUBE_DC_CLIENT_SECRET or KUBE_DC_AUDIENCE}" | jq -r .access_token)
fi
[ -n "$token" ] && [ "$token" != null ] || { echo "token request refused" >&2; exit 1; }
[ -n "${GITHUB_ENV:-}" ] && echo "::add-mask::$token"

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
if [ -n "${GITHUB_ENV:-}" ]; then
  echo "KUBECONFIG=$config" >> "$GITHUB_ENV"
else
  echo "$config"
fi
