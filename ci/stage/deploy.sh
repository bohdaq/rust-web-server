#!/usr/bin/env bash
# Test fixtures and backends in the run's namespace ($NS), before rws.
set -euo pipefail
STAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
k() { kubectl -n "$NS" "$@"; }

# The static tree rws serves from /srv (a ConfigMap: files up to 1 MiB in all).
k create configmap rws-e2e-www --from-file="$STAGE_DIR/www" --dry-run=client -o yaml | k apply -f - >/dev/null

# Backends for the proxy suite: three that answer with their own name.
k apply -f "$STAGE_DIR/manifests/backends.yaml" >/dev/null
for b in a b c; do k rollout status "deploy/whoami-$b" --timeout=180s >/dev/null; done
