# Shared helpers for the stage suites (sourced by run.sh and ci/stage/suites/*.sh).
#
# A case is a function that calls `pass`, `fail`, `blocked` or `known` with its
# case id (e.g. S1.3) and a short line; run.sh collects the lines into the
# report. Assertions talk to rws through the kube-dc gateway ($RWS_URL) or, for
# what only the cluster sees (metrics per pod, logs), through the API server.

# Results go to $REPORT/results.tsv: id <TAB> status <TAB> message.
_record() { # <status> <id> <message…>
  local status=$1 id=$2; shift 2
  printf '%s\t%s\t%s\n' "$id" "$status" "$*" >>"$REPORT/results.tsv"
  printf '%-8s %-8s %s\n' "$status" "$id" "$*"
}
pass()    { _record ok "$@"; }
fail()    { _record FAIL "$@"; }
blocked() { _record BLOCKED "$@"; }
# A failure ci/stage/known.yaml lists (the spec/roadmap line it comes from).
known()   { _record KNOWN "$@"; }

# Expected failures: `<id>: <reason>` lines in known.yaml (no YAML parser on
# purpose: one flat map).
is_known() { grep -qE "^$1:" "$STAGE_DIR/known.yaml" 2>/dev/null; }
# check <id> <description> <command…>: runs the command (a test), records ok
# or FAIL (KNOWN when known.yaml lists the id).
check() {
  local id=$1 what=$2; shift 2
  if "$@"; then pass "$id" "$what"
  elif is_known "$id"; then known "$id" "$what ($(grep -E "^$id:" "$STAGE_DIR/known.yaml" | cut -d: -f2- | sed 's/^ *//'))"
  else fail "$id" "$what"
  fi
}

# HTTP through the gateway. Every request names its case in a header so the
# access log ties a log line to a case.
LAST_HEADERS="$REPORT/last-headers"
http() { # <id> <curl args…> — prints the body; headers in $LAST_HEADERS
  local id=$1; shift
  # A fixed file, not a variable: `body=$(http …)` runs in a subshell.
  curl -sS --max-time 20 -D "$LAST_HEADERS" -H "X-Rws-E2e-Case: $id" "$@"
}
status_of() { # <id> <path> [curl args…] — prints the status code
  local id=$1 path=$2; shift 2
  curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -H "X-Rws-E2e-Case: $id" "$@" "$RWS_URL$path"
}
header_of() { # <name> — from the last `http` call, value only, case-insensitive
  tr -d '\r' <"$LAST_HEADERS" | awk -v n="$(echo "$1" | tr 'A-Z' 'a-z')" 'BEGIN{IGNORECASE=1} tolower($0) ~ "^"n":" {sub(/^[^:]*: */,""); v=$0} END{print v}'
}

# The cluster's view.
k() { kubectl -n "$NS" "$@"; }
rws_pods() { k get pods -l app.kubernetes.io/name=rws -o jsonpath='{range .items[?(@.status.phase=="Running")]}{.metadata.name}{"\n"}{end}'; }
# GET a path on one pod, through the API server's pod proxy (no port-forward).
pod_get() { # <pod> <path>
  kubectl get --raw "/api/v1/namespaces/$NS/pods/http:$1:7878/proxy$2"
}
# One metric's value summed over every rws pod: metric_sum '<name>{labels…}' (a
# prefix of the exposition line, before the value).
metric_sum() {
  local pod total=0 v
  for pod in $(rws_pods); do
    v=$(pod_get "$pod" /metrics | awk -v m="$1" 'index($0, m) == 1 {s += $NF} END {print s + 0}')
    total=$(awk -v a="$total" -v b="$v" 'BEGIN {print a + b}')
  done
  echo "$total"
}

# Wait until a command succeeds: eventually <seconds> <command…>
eventually() {
  local deadline=$(( $(date +%s) + $1 )); shift
  until "$@"; do
    [ "$(date +%s)" -ge "$deadline" ] && return 1
    sleep 2
  done
}

# Many GETs over one connection (keep-alive through the gateway): prints the
# bodies one after another. Capped at 40 requests a second: the gateway is
# shared (the design's limit is 50). many <count> <url> [curl args…]
many() {
  local n=$1 url=$2; shift 2
  local args=() i
  for ((i = 0; i < n; i++)); do args+=("$url"); done
  curl -sS --max-time 120 --rate 40/s "$@" "${args[@]}"
}
# Status codes of many GETs, one per line.
many_status() { # <count> <url> [curl args…]
  local n=$1 url=$2; shift 2
  local args=() i
  for ((i = 0; i < n; i++)); do args+=(-o /dev/null "$url"); done
  curl -sS --max-time 120 --rate 40/s -w '%{http_code}\n' "$@" "${args[@]}"
}

# The client's public address as the backends see it: the first hop of the
# X-Forwarded-For whoami echoes back (we send none of our own).
client_address() {
  curl -sS --max-time 20 "$RWS_URL/e2e/proxy/whoami" | tr -d '\r' | awk -F': ' '$1 == "X-Forwarded-For" {split($2, a, ", *"); print a[1]; exit}'
}

# The rws config in the ConfigMap the chart renders (rws-config), and an edit
# of it: config_edit <sed expression> <version>. The pods see it once the
# kubelet syncs the volume (up to a minute or so), then `watch = true`
# reloads it; config_wait <version> waits until every pod answers
# /e2e/config-version with it.
config_get() { k get configmap rws-config -o jsonpath='{.data.rws\.config\.toml}'; }
config_put() { # <file>
  k create configmap rws-config --from-file=rws.config.toml="$1" --dry-run=client -o yaml | k replace -f - >/dev/null
}
config_edit() {
  local f; f=$(mktemp -p "$REPORT")
  config_get | sed "$1; s|e2e-config-[0-9A-Za-z]*|e2e-config-$2|" >"$f"
  config_put "$f"
}
config_wait() {
  eventually 300 all_pods_serve /e2e/config-version "e2e-config-$1" && return 0
  local pod
  for pod in $(rws_pods); do echo "    $pod serves $(pod_get "$pod" /e2e/config-version 2>&1 | head -c 40), wanted e2e-config-$1" >&2; done
  return 1
}
# The marker of the config in the ConfigMap now.
config_version() { config_get | sed -n 's/.*"e2e-config-\([0-9A-Za-z]*\)".*/\1/p' | head -1; }
# Every running rws pod answers <path> with a body containing <text>.
all_pods_serve() { # <path> <text>
  local pod
  for pod in $(rws_pods); do pod_get "$pod" "$1" 2>/dev/null | grep -q "$2" || return 1; done
}

# HS256 JWT: jwt_hs256 <secret> <exp unix time> [sub]
b64url() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
jwt_hs256() {
  local head body sig
  head=$(printf '{"alg":"HS256","typ":"JWT"}' | b64url)
  body=$(printf '{"sub":"%s","exp":%s}' "${3:-e2e}" "$2" | b64url)
  sig=$(printf '%s.%s' "$head" "$body" | openssl dgst -sha256 -hmac "$1" -binary | b64url)
  printf '%s.%s.%s' "$head" "$body" "$sig"
}

# The admin API through the rws-admin Service: admin <method> <path> [curl args…]
# — prints the body (headers in $LAST_HEADERS).
admin() {
  local method=$1 path=$2; shift 2
  curl -sS --max-time 20 -D "$LAST_HEADERS" -X "$method" -H "Authorization: Bearer $E2E_ADMIN_TOKEN" "$@" "$ADMIN_URL/admin/api/v1$path"
}
# A JSON field: json_get <python expression on d> (stdin: the document)
json_get() { python3 -I -c "import json, sys; d = json.load(sys.stdin); print($1)"; }
