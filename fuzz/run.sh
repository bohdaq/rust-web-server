#!/usr/bin/env bash
# Run every fuzz target for N seconds (default 60), seeded from seeds/<target>.
# Exits non-zero if any target finds a crash; crashing inputs land in
# artifacts/<target>/ (reproduce with `cargo +nightly fuzz run <target> <file>`).
#
#   fuzz/run.sh [seconds] [parallel jobs]
set -uo pipefail
cd "$(dirname "$0")"

SECONDS_PER_TARGET="${1:-60}"
JOBS="${2:-4}"

# Build for the real host triple: a prebuilt (static musl) cargo-fuzz would
# otherwise default to its own musl target, which ASan can't use.
TRIPLE="$(rustc +nightly -vV | sed -n 's/^host: //p')"
export TRIPLE

cargo +nightly fuzz build --target "$TRIPLE" || exit 1

run_one() {
  target="$1"
  mkdir -p "corpus/$target"
  if cargo +nightly fuzz run --target "$TRIPLE" "$target" "corpus/$target" "seeds/$target" -- \
       -max_total_time="$SECONDS_PER_TARGET" -close_fd_mask=1 > "logs/$target.log" 2>&1; then
    echo "ok    $target ($(grep -aoE 'Done [0-9]+ runs' "logs/$target.log" | head -1))"
  else
    echo "CRASH $target — see fuzz/logs/$target.log and fuzz/artifacts/$target/"
    grep -a -m1 -A2 "panicked at" "logs/$target.log"
    return 1
  fi
}
export -f run_one
export SECONDS_PER_TARGET

mkdir -p logs
ls fuzz_targets | sed 's/\.rs$//' | xargs -P "$JOBS" -I{} bash -c 'run_one {}'
