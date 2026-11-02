#!/usr/bin/env bash
# The crate's public API against the committed listing ci/public-api.txt.
# Any difference fails: a removed or changed line is a breaking change (a
# major release, or put it back); an added one only needs the listing
# updated in the same commit, so API changes are always visible in review.
#
#   ci/public_api.sh            check (CI)
#   ci/public_api.sh --update   rewrite ci/public-api.txt
#
# Needs cargo-public-api 0.52.0 and the nightly below (rustdoc JSON changes
# between nightlies; move both together).
set -euo pipefail
NIGHTLY=nightly-2026-10-24
FEATURES="sso sso-server sso-saml auth-asymmetric http-client acme serde macros openapi tower gateway-api model-sqlite storage-local storage-s3 storage-azure storage-gcs pkcs11"
cd "$(dirname "$0")/.."
BASELINE=ci/public-api.txt
current="$(mktemp)"
trap 'rm -f "$current"' EXIT
cargo "+$NIGHTLY" public-api --simplified --features "$FEATURES" >"$current"
# cargo-public-api has no --locked: refuse a listing built on a changed lock.
git diff --quiet -- Cargo.lock || { echo "Cargo.lock changed while building the listing"; git diff --stat -- Cargo.lock; exit 1; }
if [ "${1:-}" = --update ]; then
  cp "$current" "$BASELINE"
  echo "wrote $BASELINE ($(wc -l <"$BASELINE") lines)"
  exit 0
fi
if diff -u "$BASELINE" "$current" >api.diff; then
  echo "public API unchanged ($(wc -l <"$BASELINE") lines)"
  rm -f api.diff
  exit 0
fi
removed=$(grep -c '^-[^-]' api.diff || true)
added=$(grep -c '^+[^+]' api.diff || true)
cat api.diff
rm -f api.diff
echo
if [ "$removed" -gt 0 ]; then
  echo "BREAKING: $removed public API line(s) removed or changed (above, '-'). Restore them, or this needs a major release and a reference/migrating-*.md entry."
fi
echo "Public API differs from $BASELINE ($added added, $removed removed). If intended: ci/public_api.sh --update, commit the listing, and add a CHANGELOG entry."
exit 1
