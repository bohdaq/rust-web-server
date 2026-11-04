#!/usr/bin/env bash
# Every starter under starters/ is a standalone project that builds and
# passes its own tests against this checkout: rendered as cargo-generate
# would ({{project-name}} and {{crate_name}} filled in), its rust-web-server
# dependency patched to the repository (rws-macros and the workspace crates
# too), then `cargo test` — which includes each starter's tests/dev_mcp.rs:
# the dev MCP endpoint answers every tool it lists.
#
# Then the generators (`rws gen`, 18.29.0): in each rendered starter a model,
# routes and a test for a sample resource and a migration are generated; the
# same commands again must change nothing, a different model must be refused
# as a conflict (with its diff), and `cargo test` must pass with the generated
# code — every generator's output compiles against this checkout and passes
# its generated test. A starter without an HTTP `app()` (the MCP server) must
# have `gen route` and `gen test` refused instead.
#
#   [RWS_BIN=target/debug/rws] ci/e2e/starters.sh [starter ...]     (default: all)
#
# Without RWS_BIN the script builds the rws binary first.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# One target directory for all starters: rust-web-server compiles once.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORK/target}"
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)
RWS="${RWS_BIN:-}"
if [ -z "$RWS" ]; then
  cargo build --quiet --manifest-path "$ROOT/Cargo.toml" --bin rws
  RWS="$CARGO_TARGET_DIR/debug/rws"
fi
RWS="$(cd "$(dirname "$RWS")" && pwd)/$(basename "$RWS")"

# `rws gen <args>` in the current directory; the run fails unless it exits
# with $1.
gen() {
  local want="$1"; shift
  local code=0
  "$RWS" gen "$@" > "$WORK/gen.out" 2>&1 || code=$?
  if [ "$code" != "$want" ]; then
    echo "FAIL: rws gen $* exited $code, expected $want:"; cat "$WORK/gen.out"; exit 1
  fi
}

# The generators in a rendered starter: a model, routes and a test for
# posts, a migration; the same again (nothing changes); a conflicting model.
generate() {
  local starter="$1" routes=1 code=0
  gen 0 model Post title:String views:i64 published:bool 'note:Option<String>'
  # No fields given: the routes take the model's.
  "$RWS" gen route posts > "$WORK/gen.out" 2>&1 || code=$?
  if [ "$code" = 1 ] && grep -q 'has no `pub fn app(' "$WORK/gen.out"; then
    routes=0
    echo "   gen route refused, as it should be: $starter builds no HTTP app()"
    gen 1 test posts
  elif [ "$code" != 0 ]; then
    echo "FAIL: rws gen route posts exited $code:"; cat "$WORK/gen.out"; exit 1
  else
    gen 0 test posts
  fi
  gen 0 migration add_index_to_posts
  # The repository's conventions: compiled and round-tripped by the tests below.
  gen 0 model Comment body:String --timestamps --soft-delete
  # Idempotent: the same commands change nothing.
  gen 0 model Post title:String views:i64 published:bool 'note:Option<String>'
  grep -q 'nothing to do' "$WORK/gen.out" || { echo "FAIL: a second gen model changed something:"; cat "$WORK/gen.out"; exit 1; }
  if [ "$routes" = 1 ]; then
    gen 0 route posts
    gen 0 test posts
  fi
  gen 0 migration add_index_to_posts --json
  grep -q '"status":"unchanged"' "$WORK/gen.out" || { echo "FAIL: gen migration --json:"; cat "$WORK/gen.out"; exit 1; }
  # Never overwritten: other fields are a conflict, shown as a diff.
  local before; before="$(cat src/models/post.rs)"
  gen 1 model Post title:String
  grep -q '^-    pub views: i64,' "$WORK/gen.out" || { echo "FAIL: the conflict shows no diff:"; cat "$WORK/gen.out"; exit 1; }
  [ "$before" = "$(cat src/models/post.rs)" ] || { echo "FAIL: a conflict changed src/models/post.rs"; exit 1; }
  gen 2 model Post title:u8
}

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
  (cd "$dest" && generate "$starter")
  (cd "$dest" && cargo test --quiet 2>&1 | tail -4)
  echo "ok: $starter passes its tests with the generated code"
done
echo "starters: all checks passed"
