#!/usr/bin/env bash
# Every starter under starters/ is a standalone project that builds and
# passes its own tests against this checkout: rendered as cargo-generate
# would ({{project-name}} and {{crate_name}} filled in), its rust-web-server
# dependency patched to the repository (rws-macros and the workspace crates
# too), then `cargo test` — which includes each starter's tests/dev_mcp.rs:
# the dev MCP endpoint answers every tool it lists.
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
  # The conventional layout (rws.app.toml): wiring in main.rs, the app in
  # lib.rs, integration tests in tests/.
  for file in AGENTS.md README.md cargo-generate.toml .gitignore rws.app.toml src/main.rs src/lib.rs tests/dev_mcp.rs; do
    [ -f "$src/$file" ] || { echo "FAIL: $starter has no $file"; exit 1; }
  done
  dest="$WORK/rws-starter-$starter"
  cp -r "$src" "$dest"
  grep -rl '{{project-name}}' "$dest" | xargs sed -i "s/{{project-name}}/rws-starter-$starter/g"
  grep -rl '{{crate_name}}' "$dest" | xargs -r sed -i "s/{{crate_name}}/rws_starter_$starter/g"
  # Anything else cargo-generate would read as Liquid fails the render.
  if grep -rnF -e '{{' -e '{%' "$dest" --include='*.rs' --include='*.toml' --include='*.md' --exclude=cargo-generate.toml; then
    echo "FAIL: $starter has text cargo-generate would read as a placeholder (above)"; exit 1
  fi
  # Between releases the checkout's version is the last release's, older than
  # the requirement the starters ship with: require exactly this checkout.
  sed -i -E "s/^rust-web-server = \{ version = \"[^\"]+\"/rust-web-server = { version = \"=$VERSION\"/" "$dest/Cargo.toml"
  cat >> "$dest/Cargo.toml" <<TOML

[patch.crates-io]
rust-web-server = { path = "$ROOT" }
rws-macros = { path = "$ROOT/rws-macros" }
$(for c in "$ROOT"/crates/*/; do [ -f "$c/Cargo.toml" ] && echo "$(basename "$c") = { path = \"$c\" }"; done)
TOML
  echo "── $starter"
  (cd "$dest" && cargo test --quiet 2>&1 | tail -4)
  (cd "$dest" && cargo build --quiet)
  echo "ok: $starter builds and passes its tests"
done
echo "starters: all checks passed"
