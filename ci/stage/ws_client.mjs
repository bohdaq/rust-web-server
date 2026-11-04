// A WebSocket round trip for S6 (Node 24's global WebSocket):
//   node ws_client.mjs <wss url> <message> [idle seconds]
// Sends <message>, waits for its echo, optionally stays idle, then sends and
// waits again. Prints "ok <n>" with the echoes seen, exits 1 otherwise.
const [url, message, idle = "0"] = process.argv.slice(2);
const ws = new WebSocket(url);
let seen = 0;
const fail = (why) => { console.log(`fail: ${why}`); process.exit(1); };
const timer = setTimeout(() => fail("timed out"), (Number(idle) + 20) * 1000);
const send = (n) => ws.send(`${message}-${n}`);
ws.onopen = () => send(1);
ws.onerror = (e) => fail(`error ${e.message ?? e}`);
ws.onclose = (e) => { if (seen < 2) fail(`closed ${e.code} after ${seen}`); };
ws.onmessage = (e) => {
  const text = String(e.data);
  if (!text.startsWith(message)) return;   // the echo server's greeting
  seen++;
  if (seen === 1) setTimeout(() => send(2), Number(idle) * 1000);
  else { clearTimeout(timer); console.log(`ok ${seen}`); ws.close(); process.exit(0); }
};
