#!/usr/bin/env bash
# Every starter under starters/ is a standalone project that builds and
# passes its own tests against this checkout: rendered as cargo-generate
# would ({{project-name}} filled in), its rust-web-server dependency patched
# to the repository, then `cargo test`.
#
#   ci/e2e/starters.sh [starter ...]     (default: all)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# One target directory for all starters: rust-web-server compiles once.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORK/target}"
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)

starters=("$@")
if [ ${#starters[@]} -eq 0 ]; then
  for dir in "$ROOT"/starters/*/; do starters+=("$(basename "$dir")"); done
fi

for starter in "${starters[@]}"; do
  src="$ROOT/starters/$starter"
  [ -f "$src/Cargo.toml" ] || { echo "FAIL: no starter $starter"; exit 1; }
  for file in AGENTS.md README.md cargo-generate.toml .gitignore src/main.rs; do
    [ -f "$src/$file" ] || { echo "FAIL: $starter has no $file"; exit 1; }
  done
  dest="$WORK/rws-starter-$starter"
  cp -r "$src" "$dest"
  grep -rl '{{project-name}}' "$dest" | xargs sed -i "s/{{project-name}}/rws-starter-$starter/g"
  # Between releases the checkout's version is the last release's, older than
  # the requirement the starters ship with: require exactly this checkout.
  sed -i -E "s/^rust-web-server = \{ version = \"[^\"]+\"/rust-web-server = { version = \"=$VERSION\"/" "$dest/Cargo.toml"
  cat >> "$dest/Cargo.toml" <<TOML

[patch.crates-io]
rust-web-server = { path = "$ROOT" }
TOML
  echo "── $starter"
  (cd "$dest" && cargo test --quiet 2>&1 | tail -4)
  (cd "$dest" && cargo build --quiet)
  echo "ok: $starter builds and passes its tests"
done
echo "starters: all checks passed"
