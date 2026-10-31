// Dictionary compression in headless Chromium against rws (ci/e2e/dictionary.sh):
// fetch v1 of a bundle (rws offers it as a dictionary), then v2 — Chromium
// must send Available-Dictionary, get dcz (or dcb), decode it, and the
// script must equal v2 on disk.
import { chromium } from "playwright";
import { readFileSync } from "node:fs";

const [base, www] = process.argv.slice(2);
const fail = (message) => { console.error(`FAIL: ${message}`); process.exit(1); };
const v2 = readFileSync(`${www}/js/app.v2.js`, "utf8");

const browser = await chromium.launch();
const page = await browser.newPage();
const seen = new Map();
page.on("response", async (r) => {
  const path = new URL(r.url()).pathname;
  seen.set(path, { headers: await r.allHeaders(), request: await r.request().allHeaders() });
});
await page.goto(`${base}/index.html`);

const fetched = async (path) => page.evaluate(async (p) => (await fetch(p)).text(), path);
const first = await fetched("/js/app.v1.js");
const v1Info = seen.get("/js/app.v1.js");
if (!v1Info?.headers["use-as-dictionary"]) fail(`v1 wasn't offered as a dictionary: ${JSON.stringify(v1Info?.headers)}`);
console.log(`ok: v1 (${first.length} bytes) offered with use-as-dictionary: ${v1Info.headers["use-as-dictionary"]}`);

// Chromium stores the dictionary asynchronously.
let info;
for (let attempt = 0; attempt < 20; attempt++) {
  await page.waitForTimeout(250);
  const text = await fetched(`/js/app.v2.js?attempt=${attempt}`);
  info = seen.get("/js/app.v2.js");
  if (text !== v2) fail("v2 as decoded by Chromium differs from the file");
  if (info?.request["available-dictionary"]) break;
}
if (!info?.request["available-dictionary"]) fail(`Chromium never sent Available-Dictionary; accept-encoding was ${info?.request["accept-encoding"]}`);
const coding = info.headers["content-encoding"];
if (coding !== "dcz" && coding !== "dcb") fail(`v2 came as ${coding}, not dcz/dcb: ${JSON.stringify(info.headers)}`);
console.log(`ok: Chromium sent available-dictionary, got ${coding} and decoded it to the exact v2 (${v2.length} bytes)`);

await browser.close();
