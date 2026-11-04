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
await page.goto(`${base}/index.html`);

// Fetch `path` in the page and return its text with the response's and
// request's headers. The response is awaited with waitForResponse, registered
// before the fetch: an async page.on("response") handler could still be
// awaiting allHeaders() when the fetch resolved, leaving nothing recorded.
const fetched = async (path) => {
  const response = page.waitForResponse((r) => {
    const url = new URL(r.url());
    return url.pathname + url.search === path;
  });
  const text = await page.evaluate(async (p) => (await fetch(p)).text(), path);
  const r = await response;
  return { text, headers: await r.allHeaders(), request: await r.request().allHeaders() };
};
const v1Info = await fetched("/js/app.v1.js");
if (!v1Info.headers["use-as-dictionary"]) fail(`v1 wasn't offered as a dictionary: ${JSON.stringify(v1Info.headers)}`);
console.log(`ok: v1 (${v1Info.text.length} bytes) offered with use-as-dictionary: ${v1Info.headers["use-as-dictionary"]}`);

// Chromium stores the dictionary asynchronously.
let info;
for (let attempt = 0; attempt < 20; attempt++) {
  await page.waitForTimeout(250);
  info = await fetched(`/js/app.v2.js?attempt=${attempt}`);
  if (info.text !== v2) fail("v2 as decoded by Chromium differs from the file");
  if (info.request["available-dictionary"]) break;
}
if (!info?.request["available-dictionary"]) fail(`Chromium never sent Available-Dictionary; accept-encoding was ${info?.request["accept-encoding"]}`);
const coding = info.headers["content-encoding"];
if (coding !== "dcz" && coding !== "dcb") fail(`v2 came as ${coding}, not dcz/dcb: ${JSON.stringify(info.headers)}`);
console.log(`ok: Chromium sent available-dictionary, got ${coding} and decoded it to the exact v2 (${v2.length} bytes)`);

await browser.close();
