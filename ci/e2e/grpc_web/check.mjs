// node check.mjs <url>: load the gRPC-web page in headless Chromium and
// report what it found. Exit 1 unless it says "ok".
import { chromium } from "playwright";

const browser = await chromium.launch();
const page = await browser.newPage({ ignoreHTTPSErrors: true });
await page.goto(process.argv[2]);
await page.waitForFunction(() => window.result !== undefined, null, { timeout: 30000 });
const result = await page.evaluate(() => window.result);
const protocol = await page.evaluate(() => performance.getEntriesByType("navigation")[0]?.nextHopProtocol);
await browser.close();
console.log(`${process.argv[2]} (${protocol}): ${result}`);
process.exit(result === "ok" ? 0 : 1);
