#!/usr/bin/env bash
# Build the admin UI into src/admin/ui/ (committed; embedded by src/admin/ui.rs).
#   admin-ui/build.sh           build
#   admin-ui/build.sh --check   build into a temp dir and fail if it differs
set -euo pipefail
cd "$(dirname "$0")"
npm ci --no-audit --no-fund >/dev/null
if [ "${1:-}" = "--check" ]; then
  out="$(mktemp -d)"
  trap 'rm -rf "$out"' EXIT
  npx vite build --outDir "$out" --emptyOutDir >/dev/null
  diff -r "$out" ../src/admin/ui || { echo "src/admin/ui is stale: run admin-ui/build.sh"; exit 1; }
  echo "src/admin/ui is up to date"
else
  npx vite build >/dev/null
  ls -l ../src/admin/ui
fi
