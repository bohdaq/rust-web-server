#!/usr/bin/env bash
# The LLM proxy from the config file, against the rws binary: Python
# stand-ins for an OpenAI provider that is rate limited, a second
# OpenAI-compatible one (chunked event streams) and Anthropic; virtual keys,
# fallback, usage from JSON and streams, budgets, /metrics and the audit log.
# With a redis-server, two replicas share one budget.
#
#   ci/e2e/llm_proxy.sh <path-to-rws> [redis-server]
set -euo pipefail

RWS="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
REDIS="${2:-}"
WORK="$(mktemp -d)"
PIDS=()
cleanup() {
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; tail -40 "$WORK"/rws*.log 2>/dev/null || true; exit 1; }
pass() { echo "ok: $*"; }
cd "$WORK"

# mode "busy": 429 to everything. "openai": JSON, or a chunked event stream
# when the body asks to stream (usage in the final chunk only if the body
# has stream_options.include_usage). "anthropic": an event stream. Each
# request's Authorization / x-api-key and body go to seen-<port>.log.
cat > provider.py <<'PY'
import http.server, json, sys
port, mode = int(sys.argv[1]), sys.argv[2]
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        with open(f"seen-{port}.log", "a") as f:
            f.write(json.dumps({"path": self.path, "auth": self.headers.get("Authorization"), "x_api_key": self.headers.get("x-api-key"), "version": self.headers.get("anthropic-version"), "encoding": self.headers.get("Accept-Encoding"), "body": body.decode()}) + "\n")
        req = json.loads(body or b"{}")
        if mode == "busy":
            return self.json(429, {"error": {"message": "rate limited"}})
        if mode == "openai" and not req.get("stream"):
            return self.json(200, {"choices": [{"message": {"content": "hi"}}], "usage": {"prompt_tokens": 11, "completion_tokens": 7}})
        if mode == "openai":
            events = [{"choices": [{"delta": {"content": "Hel"}}]}, {"choices": [{"delta": {"content": "lo"}}]}]
            if req.get("stream_options", {}).get("include_usage"):
                events.append({"choices": [], "usage": {"prompt_tokens": 20, "completion_tokens": 2}})
            frames = [f"data: {json.dumps(e)}\n\n" for e in events] + ["data: [DONE]\n\n"]
        else:
            frames = [
                "event: message_start\ndata: " + json.dumps({"type": "message_start", "message": {"usage": {"input_tokens": 30, "cache_read_input_tokens": 10, "output_tokens": 1}}}) + "\n\n",
                "event: content_block_delta\ndata: " + json.dumps({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hi"}}) + "\n\n",
                "event: message_delta\ndata: " + json.dumps({"type": "message_delta", "usage": {"output_tokens": 5}}) + "\n\n",
                "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        for frame in frames:
            data = frame.encode()
            self.wfile.write(b"%x\r\n%s\r\n" % (len(data), data))
            self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
    def json(self, status, obj):
        data = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
PY
python3 provider.py 9441 busy & PIDS+=($!)
python3 provider.py 9442 openai & PIDS+=($!)
python3 provider.py 9443 anthropic & PIDS+=($!)

export BUSY_KEY="sk-busy" OPENAI_KEY="sk-openai" ANTHROPIC_KEY="sk-ant" ALICE_KEY="vk-alice" BOB_KEY="vk-bob"
config() { # <port> [redis line]
  cat <<EOF2
port = $1

[[route]]
  name = "llm"
  [route.match]
  path = "/v1/*"
  [route.action]
  type = "llm"
  [route.action.llm]
  audit_log = true
  ${2:-}

  [[route.action.llm.provider]]
  name = "busy"
  api = "openai"
  url = "http://127.0.0.1:9441"
  key_env = "BUSY_KEY"
  [[route.action.llm.provider]]
  name = "openai"
  api = "openai"
  url = "http://127.0.0.1:9442"
  key_env = "OPENAI_KEY"
  [[route.action.llm.provider]]
  name = "anthropic"
  api = "anthropic"
  url = "http://127.0.0.1:9443"
  key_env = "ANTHROPIC_KEY"

  [[route.action.llm.key]]
  identity = "alice"
  key_env = "ALICE_KEY"
  [[route.action.llm.key]]
  identity = "bob"
  key_env = "BOB_KEY"
  budget_tokens = 50
  window_secs = 3600
EOF2
}
config 18441 > rws.config.toml
"$RWS" --check=rws.config.toml >check.log 2>&1 || { cat check.log; fail "rws --check rejected the llm config"; }
pass "rws --check accepts the llm config"

RWS_CONFIG_FILE=rws.config.toml "$RWS" >rws.log 2>&1 & PIDS+=($!)
for _ in $(seq 1 100); do
  curl -s -o /dev/null http://127.0.0.1:18441/healthz && break
  sleep 0.1
done

chat() { # <port> <key> <json body> → body
  curl -s --max-time 10 -H "Authorization: Bearer $2" -H 'Content-Type: application/json' -H 'Accept-Encoding: gzip' \
    -X POST "http://127.0.0.1:$1/v1/chat/completions" -d "$3"
}

[ "$(curl -s -o /dev/null -w '%{http_code}' -H 'Authorization: Bearer vk-eve' -X POST http://127.0.0.1:18441/v1/chat/completions -d '{}')" = 401 ] || fail "an unknown key was let in"
pass "unknown virtual keys get 401"

out=$(chat 18441 vk-alice '{"model":"gpt-e2e","messages":[]}')
grep -q '"prompt_tokens": 11' <<<"$out" || fail "no JSON reply through the fallback: $out"
grep -q '"auth": "Bearer sk-busy"' seen-9441.log || fail "the first provider wasn't tried with its key"
grep -q '"auth": "Bearer sk-openai"' seen-9442.log || fail "the fallback didn't get its own key"
! grep -q vk-alice seen-944*.log || fail "a virtual key reached a provider"
! grep -q '"encoding": "gzip"' seen-9442.log || fail "Accept-Encoding reached the provider"
pass "429 falls back to the next provider; provider keys swapped in, virtual keys never sent"

out=$(chat 18441 vk-alice '{"model":"gpt-e2e","stream":true}')
grep -q 'data: \[DONE\]' <<<"$out" && grep -q '"completion_tokens": 2' <<<"$out" || fail "the stream didn't come through: $out"
grep -q 'stream_options\\": {\\"include_usage\\": true}\|stream_options\\":{\\"include_usage\\":true}' seen-9442.log || fail "the proxy didn't ask the OpenAI stream for usage"
pass "OpenAI stream (chunked upstream) passed through with usage requested"

out=$(curl -s --max-time 10 -H 'x-api-key: vk-alice' -H 'Content-Type: application/json' -X POST http://127.0.0.1:18441/v1/messages -d '{"model":"claude-e2e","stream":true}')
grep -q 'message_stop' <<<"$out" || fail "the Anthropic stream didn't come through: $out"
grep -q '"x_api_key": "sk-ant", "version": "2023-06-01"' seen-9443.log || fail "Anthropic didn't get its key and version: $(cat seen-9443.log)"
pass "Anthropic Messages stream with x-api-key auth"

sleep 0.3
metrics=$(curl -s http://127.0.0.1:18441/metrics)
grep -q 'rws_llm_tokens_total{provider="openai",model="gpt-e2e",identity="alice",type="input"} 31' <<<"$metrics" || fail "OpenAI tokens not counted: $(grep rws_llm_ <<<"$metrics")"
grep -q 'rws_llm_tokens_total{provider="anthropic",model="claude-e2e",identity="alice",type="input"} 40' <<<"$metrics" || fail "Anthropic tokens not counted (cache reads count as input)"
grep -q 'rws_llm_tokens_total{provider="anthropic",model="claude-e2e",identity="alice",type="output"} 5' <<<"$metrics" || fail "Anthropic output tokens not counted"
grep -q 'rws_llm_fallbacks_total{provider="busy"} 2' <<<"$metrics" || fail "fallbacks not counted"
grep -q 'rws_llm_time_to_first_token_seconds_count{provider="openai",model="gpt-e2e"} 2' <<<"$metrics" || fail "no time to first token"
pass "rws_llm_* metrics: tokens per identity, fallbacks, time to first token"

grep '"event":"llm_request"' rws.log | grep -q '"sub":"alice","provider":"anthropic","model":"claude-e2e","status":200,"input_tokens":40,"output_tokens":5' || fail "no audit line for the Anthropic request"
pass "audit log"

chat 18441 vk-bob '{"model":"m"}' >/dev/null   # 18 tokens
chat 18441 vk-bob '{"model":"m"}' >/dev/null   # 36
chat 18441 vk-bob '{"model":"m"}' >/dev/null   # 54 > 50
[ "$(curl -s -o /dev/null -w '%{http_code}' -H 'Authorization: Bearer vk-bob' -X POST http://127.0.0.1:18441/v1/chat/completions -d '{}')" = 429 ] || fail "bob's budget wasn't enforced"
pass "token budget"

if [ -n "$REDIS" ]; then
  "$REDIS" --port 6391 --save "" --appendonly no >redis.log 2>&1 & PIDS+=($!)
  for _ in $(seq 1 50); do grep -q "Ready to accept connections" redis.log && break; sleep 0.1; done
  config 18442 'redis = "127.0.0.1:6391"' > one.toml
  config 18443 'redis = "127.0.0.1:6391"' > two.toml
  RWS_CONFIG_FILE=one.toml "$RWS" >rws-one.log 2>&1 & PIDS+=($!)
  RWS_CONFIG_FILE=two.toml "$RWS" >rws-two.log 2>&1 & PIDS+=($!)
  for port in 18442 18443; do
    for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$port/healthz" && break; sleep 0.1; done
  done
  chat 18442 vk-bob '{"model":"m"}' >/dev/null   # 18, on replica one
  chat 18443 vk-bob '{"model":"m"}' >/dev/null   # 36, on replica two
  chat 18442 vk-bob '{"model":"m"}' >/dev/null   # 54
  [ "$(curl -s -o /dev/null -w '%{http_code}' -H 'Authorization: Bearer vk-bob' -X POST http://127.0.0.1:18443/v1/chat/completions -d '{}')" = 429 ] || fail "the replicas didn't share bob's budget"
  pass "two replicas share one budget through Redis"
fi

echo "all LLM proxy end-to-end checks passed"
