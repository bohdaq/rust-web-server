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

# Phase 2: Redis for [cluster], the WebSocket echo, php-fpm, the OTLP
# collector, the in-cluster curl runner, and the extra Services on rws.
k apply -f "$STAGE_DIR/manifests/services.yaml" >/dev/null
for d in valkey echo php otelcol runner; do k rollout status "deploy/$d" --timeout=180s >/dev/null; done

# The example WebAssembly plugin (examples/wasm_plugin), built here if needed.
WASM="$STAGE_DIR/../../examples/wasm_plugin/target/wasm32-unknown-unknown/release/rws_wasm_plugin_example.wasm"
[ -f "$WASM" ] || PATH="$HOME/.cargo/bin:$PATH" cargo build --quiet --release --target wasm32-unknown-unknown --manifest-path "$STAGE_DIR/../../examples/wasm_plugin/Cargo.toml"
k create configmap rws-e2e-wasm --from-file=tag.wasm="$WASM" --dry-run=client -o yaml | k apply -f - >/dev/null
