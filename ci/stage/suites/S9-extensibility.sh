# S9 extensibility: a WebAssembly plugin as route middleware
# (examples/wasm_plugin, ConfigMap rws-e2e-wasm) and FastCGI to php-fpm
# (manifests/services.yaml). Sourced by run.sh with lib.sh.

wasm_ok() {
  local body; body=$(http S9.1 "$RWS_URL/e2e/wasm/x" | tr -d '\r')
  grep -qi '^X-Plugin: rws-example' <<<"$body" && [ "$(header_of x-plugin-config)" = from-stage ]
}
check S9.1 "wasm plugin: request header added, response stamped with its config" wasm_ok
wasm_answer_ok() {
  local body; body=$(http S9.2 -H 'X-Block: yes' "$RWS_URL/e2e/wasm/x")
  [ "$(head -1 "$LAST_HEADERS" | awk '{print $2}')" = 403 ] && grep -q "blocked by the example plugin" <<<"$body"
}
check S9.2 "wasm plugin answers a request itself (403)" wasm_answer_ok
wasm_metrics_ok() { [ "$(metric_sum 'rws_wasm_plugin_calls_total{plugin="tag",hook="request"}')" -gt 0 ]; }
check S9.3 "rws_wasm_plugin_calls_total counts the calls" wasm_metrics_ok

php_ok() {
  local out; out=$(http S9.4 -d 'name=rws' "$RWS_URL/e2e/php/app.php?tab=posts" -H 'Proxy: http://evil:1')
  python3 -I -c '
import json, sys
d = json.loads(sys.argv[1])
assert d["script"] == "/e2e/php/app.php", d
assert d["get"]["tab"] == "posts" and d["post"]["name"] == "rws", d
assert d["env"] == "e2e" and d["proxy"] == "", d
assert d["remote"] == sys.argv[2], (d["remote"], sys.argv[2])
' "$out" "$(client_address)" || { echo "    $out" >&2; return 1; }
}
check S9.4 "FastCGI: PHP runs with query, form, params, no HTTP_PROXY, REMOTE_ADDR = client" php_ok
php_pathinfo_ok() { [ "$(http S9.5 "$RWS_URL/e2e/php/info.php/a/b")" = "info:/a/b" ]; }
check S9.5 "FastCGI: PATH_INFO" php_pathinfo_ok
php_static_ok() {
  [ "$(http S9.6 "$RWS_URL/e2e/php/site.css")" = "body{color:red}" ] &&
    [ "$(http S9.6 "$RWS_URL/e2e/php/uploads/evil.jpg/x.php")" != PWNED ] &&
    [ "$(http S9.6 "$RWS_URL/e2e/php/uploads/evil.jpg")" = '<?php echo "PWNED";' ]
}
check S9.6 "FastCGI: static files from rws, an upload never runs as PHP" php_static_ok
php_down_ok() {
  k scale deploy/php --replicas=0 >/dev/null
  k wait --for=delete pod -l app=php --timeout=60s >/dev/null 2>&1 || true
  local code; code=$(status_of S9.7 /e2e/php/app.php)
  k scale deploy/php --replicas=1 >/dev/null
  echo "    php-fpm down: $code" >&2
  [ "$code" = 502 ]
}
check S9.7 "FastCGI: php-fpm down → 502" php_down_ok
