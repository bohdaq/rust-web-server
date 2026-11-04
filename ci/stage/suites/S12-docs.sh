# S12 docs coverage: every rws config block on the docs site goes through
# the run's image with `rws --check`, and every documented RWS_CONFIG_* is
# named somewhere in the sources (docs_coverage.py). Runs on the
# workstation with docker; the cluster isn't involved. Sourced by run.sh.

S12_IMAGE="$(awk '$1 == "repository:" {print $2}' "$ROOT/helm/rws/values.yaml" | head -1):${IMAGE_TAG:-$(awk '$1 == "appVersion:" {gsub(/"/, "", $2); print $2}' "$ROOT/helm/rws/Chart.yaml")}"
docs_ok() {
  python3 "$STAGE_DIR/docs_coverage.py" docker run --rm -v '{dir}:/c' "$S12_IMAGE" --out "$REPORT/S12" >"$REPORT/S12.log" 2>&1
  local rc=$?
  grep -E '^(docs toml blocks|env-vars.md):' "$REPORT/S12.log" | sed 's/^/    /' >&2
  grep -vE '^ok ' "$REPORT/S12.log" | grep -vE '^(docs toml blocks|env-vars.md):' | head -10 | sed 's/^/    /' >&2
  return $rc
}
if command -v docker >/dev/null; then
  check S12.1 "docs: every config block passes rws --check, no unknown keys; every documented setting is read" docs_ok
else
  blocked S12.1 "no docker on this machine"
fi
