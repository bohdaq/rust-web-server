# S1 core serving: the static tree (/srv, the rws-e2e-www ConfigMap), ranges,
# ETag, compression, respond/redirect routes, rewrite, the response cache,
# CORS, traversal. Sourced by run.sh with lib.sh loaded.
WWW="$STAGE_DIR/www"
sha() { sha256sum | cut -d' ' -f1; }

same_bytes() { # <path> <local file> [curl args…]
  local path=$1 file=$2; shift 2
  [ "$(curl -sS --max-time 20 "$@" "$RWS_URL$path" | sha)" = "$(sha <"$file")" ]
}
check S1.1 "GET / serves index.html byte for byte" same_bytes / "$WWW/index.html"
files_ok() { same_bytes /text.txt "$WWW/text.txt" && same_bytes /blob.bin "$WWW/blob.bin" && same_bytes /data.json "$WWW/data.json"; }
check S1.2 "text and binary files byte for byte" files_ok

range_ok() {
  local body
  body=$(http S1.3 -H 'Range: bytes=0-9' "$RWS_URL/text.txt")
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = 206 ] &&
    [ "$(header_of content-range)" = "bytes 0-9/$(wc -c <"$WWW/text.txt")" ] &&
    [ "$body" = "$(head -c 10 "$WWW/text.txt")" ]
}
check S1.3 "Range bytes=0-9 → 206 with Content-Range and the first 10 bytes" range_ok
check S1.4 "a range past the end → 416" test "$(status_of S1.4 /text.txt -H 'Range: bytes=999999-')" = 416

head_ok() {
  http S1.5 -I "$RWS_URL/text.txt" >/dev/null
  [ "$(header_of content-length)" = "$(wc -c <"$WWW/text.txt")" ]
}
check S1.5 "HEAD gives the length and no body" head_ok

# One ETag for the file on every replica, so a cache revalidating through the
# load balancer gets a 304 whichever pod answers.
etag_ok() {
  local etag codes
  http S1.6 "$RWS_URL/text.txt" >/dev/null
  etag=$(header_of etag)
  [ -n "$etag" ] || return 1
  # A connection each: the gateway keeps one client connection on one pod.
  codes=$(for _ in $(seq 12); do status_of S1.6 /text.txt -H "If-None-Match: $etag"; echo; done)
  grep -vqx 304 <<<"$codes" || return 0
  echo "    If-None-Match: $(sort <<<"$codes" | uniq -c | tr -s ' ' | tr '\n' ',')" >&2
  return 1
}
check S1.6 "If-None-Match → 304 on every replica" etag_ok

# curl --compressed decodes what rws sent (Content-Encoding), with our own
# Accept-Encoding taking the place of curl's.
encoding_ok() { # <encoding>
  local enc=$1
  [ "$(curl -sS --max-time 20 --compressed -D "$REPORT/S1.7.$enc" -H "Accept-Encoding: $enc" "$RWS_URL/text.txt" | sha)" = "$(sha <"$WWW/text.txt")" ] &&
    grep -qi "^content-encoding: $enc" "$REPORT/S1.7.$enc" &&
    grep -i '^vary:' "$REPORT/S1.7.$enc" | grep -qi accept-encoding
}
check S1.7 "gzip by Accept-Encoding, Vary: Accept-Encoding" encoding_ok gzip
check S1.8 "brotli by Accept-Encoding" encoding_ok br
check S1.9 "zstd by Accept-Encoding" encoding_ok zstd
identity_ok() {
  http S1.10 -H 'Accept-Encoding: identity' "$RWS_URL/text.txt" >/dev/null
  [ -z "$(header_of content-encoding)" ]
}
check S1.10 "identity → no Content-Encoding" identity_ok

respond_ok() {
  [ "$(http S1.11 "$RWS_URL/e2e/respond")" = respond-ok ] && header_of content-type | grep -q '^text/plain'
}
check S1.11 "respond route: body and type" respond_ok
redirect_ok() {
  http S1.12 "$RWS_URL/e2e/old/page" >/dev/null
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = 308 ] && [ "$(header_of location)" = /e2e/respond ]
}
check S1.12 "redirect route: 308 and Location" redirect_ok

rewrite_ok() {
  local body
  body=$(http S1.13 "$RWS_URL/e2e/rw/abc")
  [ "$(header_of x-e2e-rewrite)" = yes ] && grep -q '^GET /rewritten/abc ' <<<"$body" && grep -qi '^X-E2e-Injected: yes' <<<"$body"
}
check S1.13 "rewrite: path prefix swapped, request and response headers set" rewrite_ok

cache_ok() { # the response cache is per replica: a hit within a few requests
  local i
  for i in $(seq 8); do
    http S1.14 "$RWS_URL/e2e/cache/page" >/dev/null
    header_of cache-status | grep -q '; hit' && return 0
  done
  return 1
}
check S1.14 "response cache: a repeat is a hit (Cache-Status)" cache_ok

traversal_ok() {
  local p code body
  for p in '/../../etc/passwd' '/%2e%2e/%2e%2e/etc/passwd' '/..%2f..%2fetc/passwd' '/.htpasswd' '/%2e/etc/passwd'; do
    # -L: the gateway (Envoy) answers some encoded forms itself with a 307
    # to the normalised path; what rws then serves is what counts.
    code=$(curl -sSL --max-time 20 --path-as-is -o "$REPORT/S1.15.body" -w '%{http_code}' "$RWS_URL$p")
    case $code in 400|403|404) ;; *) echo "    $p → $code" >&2; return 1 ;; esac
    if grep -q 'root:' "$REPORT/S1.15.body"; then echo "    $p leaked /etc/passwd" >&2; return 1; fi
  done
}
check S1.15 "path traversal and dotfiles refused" traversal_ok
check S1.16 "an unknown path → 404" test "$(status_of S1.16 /no/such/file)" = 404

cors_static_ok() {
  http S1.17 -X OPTIONS -H 'Origin: https://e2e.rws8.tech' -H 'Access-Control-Request-Method: POST' "$RWS_URL/text.txt" >/dev/null
  [ "$(header_of access-control-allow-origin)" = https://e2e.rws8.tech ] || return 1
  http S1.17 -X OPTIONS -H 'Origin: https://evil.example' -H 'Access-Control-Request-Method: POST' "$RWS_URL/text.txt" >/dev/null
  [ -z "$(header_of access-control-allow-origin)" ]
}
check S1.17 "CORS preflight on a static file: allowed origin yes, others no" cors_static_ok
cors_route_ok() {
  http S1.18 -X OPTIONS -H 'Origin: https://e2e.rws8.tech' -H 'Access-Control-Request-Method: POST' "$RWS_URL/e2e/respond" >/dev/null
  [ "$(header_of access-control-allow-origin)" = https://e2e.rws8.tech ] || return 1
  http S1.18 -H 'Origin: https://e2e.rws8.tech' "$RWS_URL/e2e/proxy/cors" >/dev/null
  [ "$(header_of access-control-allow-origin)" = https://e2e.rws8.tech ]
}
check S1.18 "[cors] on [[route]] responses (respond and proxy)" cors_route_ok

# Host routing: the same pods under a second gateway hostname (rws-alt);
# [route.match] host picks the route, any other host gets the default.
vhost_ok() {
  [ -n "$ALT_URL" ] || { echo "    no rws-alt hostname" >&2; return 1; }
  [ "$(curl -sS --max-time 20 "$ALT_URL/e2e/vhost")" = vhost-alt ] &&
    [ "$(curl -sS --max-time 20 "$RWS_URL/e2e/vhost")" = vhost-default ]
}
check S1.19 "[route.match] host: the second hostname gets its own route" vhost_ok
