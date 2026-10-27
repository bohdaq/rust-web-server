// WebSocket and SSE checks against examples/realtime.rs, with Node's own
// WebSocket and fetch (undici) as the clients.
//
//   node ci/e2e/realtime_client.mjs <base URL, http:// or https://>
//
// For https, trust the test CA with NODE_EXTRA_CA_CERTS.
const base = process.argv[2];
const wsBase = base.replace(/^http/, "ws");
const fail = (message) => {
  console.log(`FAIL: ${message}`);
  process.exit(1);
};
const withTimeout = (promise, ms, what) =>
  Promise.race([promise, new Promise((_, reject) => setTimeout(() => reject(new Error(`${what}: no answer in ${ms} ms`)), ms))]);

function open(path) {
  const ws = new WebSocket(`${wsBase}${path}`);
  ws.binaryType = "arraybuffer";
  const received = [];
  let waiting = null;
  ws.onmessage = (event) => {
    received.push(event.data);
    if (waiting) waiting();
  };
  const closed = new Promise((resolve) => (ws.onclose = resolve));
  const next = async () => {
    while (received.length === 0) await new Promise((resolve) => (waiting = resolve));
    return received.shift();
  };
  return withTimeout(new Promise((resolve, reject) => {
    ws.onopen = () => resolve({ ws, next, closed });
    ws.onerror = () => reject(new Error(`could not open ${path}`));
  }), 5000, `open ${path}`);
}

// ── chat: echo with the room's name, text and binary, a big message ─────────
{
  const { ws, next, closed } = await open("/chat/lobby");
  ws.send("hi");
  const first = await withTimeout(next(), 5000, "chat reply");
  if (!/^lobby #\d+: hi$/.test(first)) fail(`chat reply ${JSON.stringify(first)}`);
  const big = "é".repeat(200_000); // 400 KB of two-byte characters
  ws.send(big);
  const echoed = await withTimeout(next(), 5000, "big reply");
  if (!echoed.endsWith(big) || !echoed.startsWith("lobby #")) fail(`big message came back as ${echoed.length} chars`);
  ws.send(new Uint8Array([0, 1, 2, 255]));
  const binary = new Uint8Array(await withTimeout(next(), 5000, "binary reply"));
  if (binary.join(",") !== "0,1,2,255") fail(`binary echo ${binary.join(",")}`);
  ws.close(1000, "bye");
  const event = await withTimeout(closed, 5000, "chat close");
  if (!event.wasClean || event.code !== 1000) fail(`chat close: clean=${event.wasClean} code=${event.code}`);
  console.log("ok: chat echo (text, 400 KB text, binary) and a clean close");
}

// ── ticker: the server sends first, from another thread, then closes ────────
{
  const { next, closed } = await open("/ticker");
  const ticks = [];
  for (let i = 0; i < 3; i++) ticks.push(await withTimeout(next(), 5000, "tick"));
  if (ticks.join("|") !== "tick 1|tick 2|tick 3") fail(`ticks ${ticks.join("|")}`);
  const event = await withTimeout(closed, 5000, "ticker close");
  if (!event.wasClean || event.code !== 1000 || event.reason !== "done") fail(`ticker close: clean=${event.wasClean} code=${event.code} reason=${event.reason}`);
  console.log("ok: server-pushed ticks from another thread, server-initiated clean close");
}

// ── a plain GET on a WebSocket route ─────────────────────────────────────────
{
  const response = await fetch(`${base}/chat/lobby`);
  if (response.status !== 426) fail(`plain GET on a WebSocket route: ${response.status}`);
  console.log("ok: a plain GET on a WebSocket route gets 426");
}

// ── SSE: events arrive as they are sent ──────────────────────────────────────
{
  const started = Date.now();
  const response = await fetch(`${base}/events`);
  if (response.headers.get("content-type") !== "text/event-stream") fail(`SSE content type ${response.headers.get("content-type")}`);
  const decoder = new TextDecoder();
  let text = "";
  const arrivals = [];
  for await (const chunk of response.body) {
    text += decoder.decode(chunk, { stream: true });
    while (arrivals.length < (text.match(/\n\n/g) || []).length) arrivals.push(Date.now() - started);
  }
  const ids = [...text.matchAll(/^id: (\d+)$/gm)].map((m) => m[1]).join(",");
  if (ids !== "1,2,3" || !text.includes("event: count\ndata: event 2\n")) fail(`SSE body ${JSON.stringify(text)}`);
  // REALTIME_EVENT_MS = 400: buffered, all three would arrive together at the end.
  if (!(arrivals[0] < 300 && arrivals[1] >= 300 && arrivals[2] - arrivals[0] >= 600)) fail(`SSE events weren't streamed: arrived at ${arrivals} ms`);
  console.log(`ok: SSE events streamed as sent (at ${arrivals.join(", ")} ms), the stream ended with its senders`);
}
