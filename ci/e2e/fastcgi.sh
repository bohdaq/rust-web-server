#!/usr/bin/env bash
# FastCGI against real php-fpm: a front-controller app (pretty URLs reach
# index.php), PATH_INFO, query strings, form posts and uploads, cookies,
# redirects and status codes from PHP, static files served by rws, the
# cgi.fix_pathinfo hole closed, a Unix socket, and php-fpm down → 502.
#
#   ci/e2e/fastcgi.sh <path-to-rws> <php-fpm binary | docker>
#
# With `docker`, php:8.3-fpm runs with the work directory mounted at the
# same path and host networking, so SCRIPT_FILENAME paths match.
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
PHP_FPM="$2"
WORK="$(mktemp -d)"
chmod 755 "$WORK"
PIDS=()
CONTAINER=""
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  [ -n "$CONTAINER" ] && docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  wait 2>/dev/null || true
  rm -rf "$WORK" 2>/dev/null || sudo rm -rf "$WORK" 2>/dev/null || true
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -30 "$WORK"/rws*.log "$WORK"/fpm.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

mkdir -p app/public/assets app/public/uploads app/public/admin run
cat > app/public/index.php <<'PHP'
<?php
$path = parse_url($_SERVER['REQUEST_URI'], PHP_URL_PATH);
if ($path === '/login') { header('Location: /dashboard'); exit; }
if ($path === '/missing') { http_response_code(404); echo "no such page"; exit; }
if ($path === '/cookie') { setcookie('a', '1'); setcookie('b', '2'); echo "cookies"; exit; }
if ($path === '/upload') {
  $f = $_FILES['file'] ?? null;
  echo $f ? "uploaded " . $f['name'] . " " . filesize($f['tmp_name']) : "no file";
  exit;
}
header('Content-Type: application/json');
echo json_encode([
  'script' => $_SERVER['SCRIPT_NAME'],
  'uri' => $_SERVER['REQUEST_URI'],
  'path_info' => $_SERVER['PATH_INFO'] ?? '',
  'get' => $_GET,
  'post' => $_POST,
  'host' => $_SERVER['SERVER_NAME'],
  'https' => $_SERVER['HTTPS'] ?? '',
  'remote' => $_SERVER['REMOTE_ADDR'],
  'env' => $_SERVER['APP_ENV'] ?? '',
  'proxy' => getenv('HTTP_PROXY') ?: '',
]);
PHP
echo '<?php echo "info:" . ($_SERVER["PATH_INFO"] ?? "");' > app/public/info.php
echo '<?php echo "admin index";' > app/public/admin/index.php
echo 'body{color:red}' > app/public/assets/site.css
echo '<?php echo "PWNED";' > app/public/uploads/evil.jpg

FPM_ARGS=(--nodaemonize --fpm-config "$WORK/fpm.conf")
cat > fpm.conf <<CONF
[global]
error_log = /proc/self/fd/2
[www]
listen = 127.0.0.1:19001
pm = static
pm.max_children = 4
clear_env = no
catch_workers_output = yes
security.limit_extensions = .php
CONF
cat > fpm-sock.conf <<CONF
[global]
error_log = /proc/self/fd/2
[www]
listen = $WORK/run/fpm.sock
listen.mode = 0666
pm = static
pm.max_children = 2
security.limit_extensions = .php
CONF

start_fpm() { # <config file> <name>
  if [ "$PHP_FPM" = docker ]; then
    CONTAINER=$(docker run -d --rm --network host --user "$(id -u):$(id -g)" -v "$WORK:$WORK" php:8.3-fpm php-fpm --nodaemonize --fpm-config "$1")
  else
    "$PHP_FPM" --nodaemonize --fpm-config "$1" >"fpm-$2.log" 2>&1 & PIDS+=($!)
  fi
}
stop_fpm() {
  if [ "$PHP_FPM" = docker ]; then docker rm -f "$CONTAINER" >/dev/null; CONTAINER=""; else kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true; fi
}

cat > rws.config.toml <<TOML
port = 18491

[[route]]
name = "php"
[route.match]
path = "/*"
[route.action]
type = "fastcgi"
address = "127.0.0.1:19001"
root = "$WORK/app/public"
[route.action.params]
APP_ENV = "e2e"
TOML
"$RWS" --check=rws.config.toml >check.log 2>&1 || fail "rws --check: $(cat check.log)"
sed 's/^address = .*$/address = "nowhere"/' rws.config.toml > bad.toml
! "$RWS" --check=bad.toml >check.log 2>&1 || fail "rws --check accepted a bad address"
pass "rws --check validates the fastcgi action"

start_fpm "$WORK/fpm.conf" tcp
for _ in $(seq 1 100); do (exec 3<>/dev/tcp/127.0.0.1/19001) 2>/dev/null && break; sleep 0.1; done
(cd "$WORK" && exec env RWS_CONFIG_FILE=rws.config.toml "$RWS" >rws.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18491/assets/site.css && break; sleep 0.1; done

j() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
out=$(curl -s "http://127.0.0.1:18491/users/7?tab=posts" -H "Proxy: http://evil:1")
[ "$(j "d['script']" <<<"$out")" = /index.php ] || fail "front controller: $out"
[ "$(j "d['uri']" <<<"$out")" = "/users/7?tab=posts" ] || fail "REQUEST_URI: $out"
[ "$(j "d['get']['tab']" <<<"$out")" = posts ] || fail "query: $out"
[ "$(j "d['env']" <<<"$out")" = e2e ] || fail "params: $out"
[ "$(j "d['proxy']" <<<"$out")" = "" ] || fail "httpoxy: HTTP_PROXY reached PHP: $out"
[ "$(j "d['remote']" <<<"$out")" = 127.0.0.1 ] || fail "REMOTE_ADDR: $out"
pass "front controller: pretty URL, REQUEST_URI, query, params, no HTTP_PROXY"

[ "$(curl -s http://127.0.0.1:18491/info.php/a/b)" = "info:/a/b" ] || fail "PATH_INFO"
[ "$(curl -s http://127.0.0.1:18491/admin/)" = "admin index" ] || fail "directory index"
[ "$(curl -s -o /dev/null -w '%{http_code} %{redirect_url}' http://127.0.0.1:18491/admin)" = "301 http://127.0.0.1:18491/admin/" ] || fail "directory redirect"
pass "PATH_INFO, directory index and its redirect"

out=$(curl -s -d "name=rws&lang=rust" http://127.0.0.1:18491/form)
[ "$(j "d['post']['name']" <<<"$out")" = rws ] || fail "form post: $out"
head -c 300000 /dev/urandom > upload.bin
[ "$(curl -s -F file=@upload.bin http://127.0.0.1:18491/upload)" = "uploaded upload.bin 300000" ] || fail "upload"
pass "form post and a 300 KB multipart upload"

[ "$(curl -s -o /dev/null -w '%{http_code} %{redirect_url}' http://127.0.0.1:18491/login)" = "302 http://127.0.0.1:18491/dashboard" ] || fail "Location → 302"
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18491/missing)" = 404 ] || fail "http_response_code(404)"
[ "$(curl -s -D - -o /dev/null http://127.0.0.1:18491/cookie | grep -ci '^set-cookie:')" = 2 ] || fail "two Set-Cookie headers"
pass "redirects, status codes and several cookies from PHP"

[ "$(curl -s http://127.0.0.1:18491/assets/site.css)" = "body{color:red}" ] || fail "static file"
grep -q "assets/site.css" fpm-tcp.log 2>/dev/null && fail "the static file went to php-fpm"
[ "$(curl -s http://127.0.0.1:18491/uploads/evil.jpg/x.php)" != PWNED ] || fail "cgi.fix_pathinfo: an upload ran as PHP"
[ "$(curl -s http://127.0.0.1:18491/uploads/evil.jpg)" = '<?php echo "PWNED";' ] || fail "the upload should be served as a file"
pass "static files from rws; uploads never run as PHP"

stop_fpm
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18491/)" = 502 ] || fail "php-fpm down should be 502"
pass "php-fpm down → 502"

kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null || true
start_fpm "$WORK/fpm-sock.conf" sock
for _ in $(seq 1 100); do [ -S run/fpm.sock ] && break; sleep 0.1; done
sed -i "s|^address = .*$|address = \"unix:$WORK/run/fpm.sock\"|" rws.config.toml
(cd "$WORK" && exec env RWS_CONFIG_FILE=rws.config.toml "$RWS" >rws-sock.log 2>&1) & PIDS+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null http://127.0.0.1:18491/assets/site.css && break; sleep 0.1; done
[ "$(curl -s http://127.0.0.1:18491/info.php/sock)" = "info:/sock" ] || fail "Unix socket"
pass "Unix socket"

echo "all FastCGI end-to-end checks passed"
