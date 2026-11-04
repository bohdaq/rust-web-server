#!/usr/bin/env bash
# The test app (ci/stage/app) and its services in the run's namespace ($NS):
# a per-run registry it's pushed to, a managed PostgreSQL (started early by
# `deploy_app.sh db`), a bucket, mailpit, and the app, IdP and SP. Per-run
# keys and secrets go to $REPORT/app (private keys stay on this machine).
#
#   deploy_app.sh db     order the managed PostgreSQL (returns at once)
#   deploy_app.sh build  build the image locally (run.sh does it in the background)
#   deploy_app.sh        registry, push, the rest; waits until all is ready
set -euo pipefail
STAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$STAGE_DIR/../.." && pwd)"
k() { kubectl -n "$NS" "$@"; }
log() { printf '== %s\n' "$*"; }
LOCAL_IMAGE=rws-e2e-app:local
eventually() { local deadline=$(( $(date +%s) + $1 )); shift; until "$@"; do [ "$(date +%s)" -ge "$deadline" ] && return 1; sleep 3; done; }

case "${1:-}" in
  db)
    k apply -f - >/dev/null <<YAML
apiVersion: services.kube-dc.com/v1alpha1
kind: ManagedService
metadata:
  name: e2e-pg
spec:
  classRef: {name: postgresql}
  connectivity:
    classRef: {name: tenant-native}
  placement: {mode: ProviderShared}
  planRef: {name: postgresql-development}
  deletionPolicy: Delete
  parameters: {database: app}
  topology: {instances: 1}
YAML
    exit 0 ;;
  build)
    DOCKER_BUILDKIT=1 docker build -q -f "$ROOT/ci/stage/app/Dockerfile" -t "$LOCAL_IMAGE" "$ROOT" >/dev/null
    exit 0 ;;
esac

APP_DIR="$REPORT/app"; mkdir -p "$APP_DIR"; chmod 700 "$APP_DIR"
rand() { head -c 24 /dev/urandom | base64 | tr -d '/+='; }

# ── registry ─────────────────────────────────────────────────────────────────
REG_PASS=$(rand)
python3 -I -c 'import bcrypt, sys; print("e2e:" + bcrypt.hashpw(sys.argv[1].encode(), bcrypt.gensalt(rounds=10)).decode())' "$REG_PASS" >"$APP_DIR/htpasswd"
k create secret generic rws-e2e-registry --from-file=htpasswd="$APP_DIR/htpasswd" --dry-run=client -o yaml | k apply -f - >/dev/null
k apply -f "$STAGE_DIR/manifests/registry.yaml" >/dev/null
# The registry reads htpasswd at startup: a reused project's pod has the old one.
k rollout restart deploy/registry >/dev/null
k rollout status deploy/registry --timeout=180s >/dev/null
REGISTRY="registry-$NS.stage.kube-dc.com"
eventually 300 sh -c "curl -s -o /dev/null -w '%{http_code}' --max-time 10 https://$REGISTRY/v2/ | grep -qx 401" || { echo "registry $REGISTRY not reachable" >&2; exit 1; }
k create secret docker-registry rws-e2e-registry-pull --docker-server="$REGISTRY" --docker-username=e2e --docker-password="$REG_PASS" \
  --dry-run=client -o yaml | k apply -f - >/dev/null

# The image: built by `deploy_app.sh build` (run.sh starts it first thing
# and waits for it before this).
docker image inspect "$LOCAL_IMAGE" >/dev/null 2>&1 || "$0" build
VERSION=$(awk '$1 == "appVersion:" {gsub(/"/, "", $2); print $2}' "$ROOT/helm/rws/Chart.yaml")
APP_IMAGE="$REGISTRY/rws-e2e-app:$VERSION-e2e-$RUN_ID"
docker tag "$LOCAL_IMAGE" "$APP_IMAGE"
DOCKER_CONFIG="$APP_DIR/docker"; export DOCKER_CONFIG; mkdir -p "$DOCKER_CONFIG"
eventually 120 sh -c "printf '%s' '$REG_PASS' | docker login -u e2e --password-stdin $REGISTRY >/dev/null 2>&1" ||
  { echo "docker login to $REGISTRY failed" >&2; exit 1; }
docker push -q "$APP_IMAGE" >/dev/null || { echo "docker push $APP_IMAGE failed" >&2; exit 1; }
DIGEST=$(docker image inspect "$APP_IMAGE" --format '{{index .RepoDigests 0}}')
docker rmi "$APP_IMAGE" >/dev/null
unset DOCKER_CONFIG
log "app image $DIGEST"
echo "$DIGEST" >"$APP_DIR/image"

# ── keys and secrets ─────────────────────────────────────────────────────────
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$APP_DIR/rs256.key" 2>/dev/null
openssl pkey -in "$APP_DIR/rs256.key" -pubout -out "$APP_DIR/rs256.pub"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$APP_DIR/es256.key" 2>/dev/null
openssl pkey -in "$APP_DIR/es256.key" -pubout -out "$APP_DIR/es256.pub"
# run.sh made these (rws's config needs the MCP token too).
printf '%s' "$E2E_APP_MCP_TOKEN" >"$APP_DIR/mcp-token"
for name in webhook-secret oidc-secret idp-signing; do rand >"$APP_DIR/$name"; done
k create secret generic rws-e2e-app \
  --from-file=mcp-token="$APP_DIR/mcp-token" --from-file=webhook-secret="$APP_DIR/webhook-secret" \
  --from-file=oidc-secret="$APP_DIR/oidc-secret" --from-file=idp-signing="$APP_DIR/idp-signing" \
  --from-file=rs256.pub="$APP_DIR/rs256.pub" --from-file=es256.pub="$APP_DIR/es256.pub" \
  --dry-run=client -o yaml | k apply -f - >/dev/null

# ── PostgreSQL (ordered at the start), then the app ──────────────────────────
pg_ready() { [ "$(k get managedservice e2e-pg -o jsonpath='{.status.phase}' 2>/dev/null)" = Ready ]; }
eventually 900 pg_ready || { echo "managed PostgreSQL not Ready after 15 min: $(k get managedservice e2e-pg -o jsonpath='{.status.phase} {.status.conditions[*].message}')" >&2; exit 1; }
# kube-dc doesn't let pods read the service's own Secrets (kdc-pg-v1-*):
# copy what the app needs into one of ours. The host is the endpoint's
# (the name its certificate carries), the CA comes without its key.
PG_CREDS=$(k get managedservice e2e-pg -o jsonpath='{.status.credentials[?(@.role=="owner")].secretName}')
PG_CA=$(k get managedservice e2e-pg -o jsonpath='{.status.endpoints[?(@.role=="read-write")].tls.caRef.name}')
PG_HOST=$(k get managedservice e2e-pg -o jsonpath='{.status.endpoints[?(@.role=="read-write")].addresses[0].host}')
PG_PORT=$(k get managedservice e2e-pg -o jsonpath='{.status.endpoints[?(@.role=="read-write")].addresses[0].port}')
eventually 120 sh -c "kubectl -n $NS get secret $PG_CREDS $PG_CA >/dev/null 2>&1" || { echo "no Secret $PG_CREDS / $PG_CA" >&2; exit 1; }
field() { k get secret "$1" -o jsonpath="{.data.$2}" | base64 -d; }
k create secret generic rws-e2e-pg --from-literal=host="$PG_HOST" --from-literal=port="$PG_PORT" \
  --from-literal=dbname="$(field "$PG_CREDS" dbname)" --from-literal=username="$(field "$PG_CREDS" username)" \
  --from-literal=password="$(field "$PG_CREDS" password)" --from-literal=ca.crt="$(field "$PG_CA" 'ca\.crt')" \
  --dry-run=client -o yaml | k apply -f - >/dev/null
PG_SECRET=rws-e2e-pg
log "managed PostgreSQL ready ($PG_SECRET)"

sed -e "s|@APP_IMAGE@|$DIGEST|g" -e "s|@PG_SECRET@|$PG_SECRET|g" -e "s|@NS@|$NS|g" "$STAGE_DIR/manifests/app.yaml" | k apply -f - >/dev/null
eventually 300 sh -c "kubectl -n $NS get configmap e2e-bucket >/dev/null 2>&1" || { echo "the bucket claim got no ConfigMap" >&2; exit 1; }
# Env comes from Secrets at start: a reused project's pods have the old ones.
k rollout restart deploy/app deploy/idp deploy/sp >/dev/null
for d in mailpit app idp sp; do k rollout status "deploy/$d" --timeout=300s >/dev/null; done
for h in app idp sp; do
  eventually 300 sh -c "curl -fsS --max-time 10 https://$h-$NS.stage.kube-dc.com/healthz >/dev/null" || { echo "$h not reachable through the gateway" >&2; exit 1; }
done
log "app, idp, sp ready"
