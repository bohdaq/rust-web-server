#!/usr/bin/env bash
# `cargo metadata --features <f>` at the workspace root for every feature of
# the facade: cargo-auditable (release binaries) runs it with the binary's
# features, and it fails when any member has an optional dependency of that
# name hidden behind `dep:` (18.35.0's musl builds: rws-k8s and `serde`).
set -euo pipefail
cd "$(dirname "$0")/.."
features=$(cargo metadata --format-version 1 --no-deps | python3 -c '
import json, sys
meta = json.load(sys.stdin)
root = next(p for p in meta["packages"] if p["name"] == "rust-web-server")
print("\n".join(sorted(root["features"])))')
failed=0
for f in $features; do
  if ! out=$(cargo metadata --format-version 1 --locked --features "$f" 2>&1 >/dev/null); then
    echo "::error::cargo metadata --features $f: $(echo "$out" | head -1)"
    failed=1
  fi
done
exit $failed
