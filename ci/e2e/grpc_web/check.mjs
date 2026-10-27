// node check.mjs <url>: load the gRPC-web page in headless Chromium and
// report what it found. Exit 1 unless it says "ok".
import { chromium } from "playwright";

const browser = await chromium.launch();
const page = await browser.newPage({ ignoreHTTPSErrors: true });
page.on("console", message => console.log(`console: ${message.text()}`));
page.on("requestfailed", request => console.log(`request failed: ${request.url()} ${request.failure()?.errorText}`));
await page.goto(process.argv[2]);
try {
  await page.waitForFunction(() => window.result !== undefined, null, { timeout: 60000 });
} catch (e) {
  console.log("progress:", JSON.stringify(await page.evaluate(() => window.progress)));
  throw e;
}
const result = await page.evaluate(() => window.result);
console.log("progress:", JSON.stringify(await page.evaluate(() => window.progress)));
const protocol = await page.evaluate(() => performance.getEntriesByType("navigation")[0]?.nextHopProtocol);
await browser.close();
console.log(`${process.argv[2]} (${protocol}): ${result}`);
process.exit(result === "ok" ? 0 : 1);
