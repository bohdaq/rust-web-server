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
#
# Workspace crates the facade re-exports (spec/WORKSPACE_SPLIT.md) are in
# CRATES: `<crate>=<facade path>[:<features>]`. A re-exported module shows
# up in the facade's listing as one `pub use` line, so each crate is listed
# too, its paths rewritten to the facade's (`rws_mime::` with facade path
# `mime_type` becomes `rust_web_server::mime_type::`; an empty facade path
# maps the crate root to the facade root), and the result merged — the
# listing users see, whichever crate an item lives in.
set -euo pipefail
NIGHTLY=nightly-2026-10-24
FEATURES="sso sso-server sso-saml auth-asymmetric http-client acme serde macros openapi tower gateway-api model-sqlite storage-local storage-s3 storage-azure storage-gcs pkcs11"
cd "$(dirname "$0")/.."
CRATES="rws-base= rws-http=:serde,brotli,zstd rws-server=:http3,sso,sso-server,sso-saml,auth-asymmetric,http-client,acme,serde,macros,openapi,gateway-api,model-sqlite,storage-local,storage-s3,storage-azure,storage-gcs,pkcs11 rws-acme= rws-storage=:storage-local,storage-s3,storage-azure,storage-gcs rws-tower= rws-proxy=:http3,sso,auth,http-client,acme,serde,model-sqlite,storage-s3,gateway-api rws-gateway-api="
BASELINE=ci/public-api.txt
current="$(mktemp)"
parts="$(mktemp -d)"
trap 'rm -rf "$current" "$parts"' EXIT
cargo "+$NIGHTLY" public-api --simplified --features "$FEATURES" >"$parts/facade.txt"
maps=()
for entry in $CRATES; do
  krate=${entry%%=*}; rest=${entry#*=}; facade=${rest%%:*}; feats=""
  [ "$rest" != "$facade" ] && feats=${rest#*:}
  cargo "+$NIGHTLY" public-api -p "$krate" --simplified ${feats:+--features "${feats//,/ }"} >"$parts/$krate.txt"
  maps+=("${krate//-/_}=$facade")
done
python3 ci/public_api_merge.py "$parts" "${maps[@]}" >"$current"
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
