#!/usr/bin/env bash
# WebSocket upgrades through a config-file `type = "proxy"` route, against
# the rws binary: a Python WebSocket echo backend, a Python client doing the
# RFC 6455 handshake through rws over plain HTTP and over TLS (tls =
# "internal"), text frames echoed both ways, several concurrent sockets,
# ordinary requests on the same route still proxied, and a dead backend → 502.
#
#   ci/e2e/ws_upgrade.sh <path-to-rws>
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -20 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

cat > backend.py <<'PY'
# A minimal WebSocket echo server (text frames), plus a plain GET answer.
import base64, hashlib, socket, struct, threading
GUID = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
def recv_frame(c):
    h = c.recv(2)
    if len(h) < 2: return None
    n = h[1] & 0x7f
    if n == 126: n = struct.unpack(">H", c.recv(2))[0]
    elif n == 127: n = struct.unpack(">Q", c.recv(8))[0]
    mask = c.recv(4) if h[1] & 0x80 else b"\0\0\0\0"
    data = b""
    while len(data) < n: data += c.recv(n - len(data))
    return h[0] & 0x0f, bytes(b ^ mask[i % 4] for i, b in enumerate(data))
def serve(c):
    head = b""
    while b"\r\n\r\n" not in head:
        chunk = c.recv(1024)
        if not chunk: return
        head += chunk
    lines = head.decode().split("\r\n")
    headers = {l.split(":", 1)[0].lower(): l.split(":", 1)[1].strip() for l in lines[1:] if ":" in l}
    if headers.get("upgrade", "").lower() != "websocket":
        body = b"plain " + lines[0].split()[1].encode()
        c.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s" % (len(body), body)); c.close(); return
    accept = base64.b64encode(hashlib.sha1(headers["sec-websocket-key"].encode() + GUID).digest())
    c.sendall(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n")
    while True:
        f = recv_frame(c)
        if f is None or f[0] == 8: break
        reply = b"echo:" + f[1] + b" path=" + lines[0].split()[1].encode()
        c.sendall(bytes([0x81, len(reply)]) + reply)
    c.close()
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(("127.0.0.1", 18522)); s.listen(64)
while True:
    conn, _ = s.accept(); threading.Thread(target=serve, args=(conn,), daemon=True).start()
PY
cat > client.py <<'PY'
# Handshake through rws, send N text frames, check the echoes. argv: host port tls(0/1) ca n
import base64, os, socket, ssl, sys
host, port, tls, ca, n = sys.argv[1], int(sys.argv[2]), sys.argv[3] == "1", sys.argv[4], int(sys.argv[5])
c = socket.create_connection((host, port), timeout=10)
if tls:
    ctx = ssl.create_default_context(cafile=ca); ctx.set_alpn_protocols(["http/1.1"])
    c = ctx.wrap_socket(c, server_hostname=host)
key = base64.b64encode(os.urandom(16)).decode()
c.sendall(f"GET /ws/chat HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode())
head = b""
while b"\r\n\r\n" not in head: head += c.recv(1)
assert head.startswith(b"HTTP/1.1 101"), head
rest = b""
for i in range(n):
    msg = f"m{i}".encode(); mask = os.urandom(4)
    c.sendall(bytes([0x81, 0x80 | len(msg)]) + mask + bytes(b ^ mask[j % 4] for j, b in enumerate(msg)))
    while len(rest) < 2: rest += c.recv(64)
    ln = rest[1]
    while len(rest) < 2 + ln: rest += c.recv(64)
    got, rest = rest[2:2 + ln], rest[2 + ln:]
    assert got == b"echo:" + msg + b" path=/chat", got
c.sendall(bytes([0x88, 0x80]) + os.urandom(4))
print("ok")
PY
python3 -I backend.py & PIDS+=($!)

export RWS_CONFIG_TLS_INTERNAL_DIR="$WORK/pki"
write_config() { # <port> <extra top-level>
  cat > rws.config.toml <<TOML
port = $1
$2
[[upstream]]
name = "app"
backends = ["127.0.0.1:18522"]

[[route]]
[route.match]
path = "/ws/*"
[route.action]
type = "proxy"
upstream = "app"
strip_path_prefix = "/ws"
TOML
}
start() { # <name> <probe url>
  (exec env RWS_CONFIG_FILE=rws.config.toml "$RWS" >"rws-$1.log" 2>&1) & PIDS+=($!)
  for _ in $(seq 1 100); do curl -s -o /dev/null --cacert pki/ca.pem "$2" && return 0; sleep 0.1; done
  fail "rws didn't start ($1)"
}

write_config 18521 ""
start plain http://127.0.0.1:18521/ws/x
[ "$(python3 -I client.py 127.0.0.1 18521 0 - 5)" = ok ] || fail "WebSocket through a plain listener"
[ "$(curl -s http://127.0.0.1:18521/ws/hello)" = "plain /hello" ] || fail "ordinary requests on the route"
for i in 1 2 3 4 5 6 7 8; do python3 -I client.py 127.0.0.1 18521 0 - 20 >"c$i.out" & done
wait $(jobs -p | tail -8) 2>/dev/null || true
[ "$(cat c*.out | grep -c '^ok$')" = 8 ] || fail "concurrent WebSockets: $(cat c*.out)"
pass "plain: handshake, echoes, 8 concurrent sockets, ordinary requests still proxied"

kill "${PIDS[-1]}"; wait "${PIDS[-1]}" 2>/dev/null || true
write_config 18523 'tls = "internal"'
start tls https://localhost:18523/ws/x
[ "$(python3 -I client.py localhost 18523 1 pki/ca.pem 5)" = ok ] || fail "WebSocket through TLS"
pass "TLS (tls = \"internal\"): handshake and echoes"

kill "${PIDS[0]}"; wait "${PIDS[0]}" 2>/dev/null || true
status=$(curl -s -o /dev/null -w '%{http_code}' --cacert pki/ca.pem -H "Connection: Upgrade" -H "Upgrade: websocket" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" -H "Sec-WebSocket-Version: 13" https://localhost:18523/ws/x)
[ "$status" = 502 ] || fail "dead backend: $status"
pass "a dead backend is a 502"

echo "all WebSocket upgrade end-to-end checks passed"
