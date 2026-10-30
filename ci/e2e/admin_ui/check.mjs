// The admin UI in headless Chromium against a running rws (ci/e2e/admin_ui.sh):
// no CSP violations or script errors, a wrong token refused, then drain a
// backend and switch maintenance mode through the UI and see the API agree,
// and a request appear in the live log.
import { chromium } from "playwright";

const [admin, token, publicUrl] = process.argv.slice(2);
const api = (path) => fetch(`${admin}/admin/api/v1${path}`, { headers: { Authorization: `Bearer ${token}` } }).then((r) => r.json());
const fail = (message) => { console.error(`FAIL: ${message}`); process.exit(1); };

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const problems = [];
page.on("pageerror", (e) => problems.push(`script error: ${e.message}`));
page.on("console", (m) => {
  const text = m.text();
  if (m.type() === "error" && !/status of (401|404)/.test(text)) problems.push(`console: ${text}`);
  if (/Content Security Policy/i.test(text)) problems.push(`CSP: ${text}`);
});

const signIn = async (value) => {
  const input = page.locator("#token input");
  await input.fill(value);
  await input.press("Enter");
};

await page.goto(`${admin}/admin/`);
await signIn("not-the-token");
await page.waitForFunction(() => document.querySelector("#token")?.error === true, null, { timeout: 5000 }).catch(() => fail("a wrong token wasn't refused"));
console.log("ok: a wrong token is refused");

await signIn(token);
await page.getByText("Requests served").waitFor({ timeout: 5000 }).catch(() => fail("no overview after signing in"));
console.log("ok: signed in, overview shown");

await page.goto(`${admin}/admin/#/upstreams`);
const first = page.locator("lit-material-switch").first();
await first.waitFor({ timeout: 5000 });
await first.click();
await page.getByText("Drained").first().waitFor({ timeout: 5000 }).catch(() => fail("the UI didn't show the backend drained"));
const [web] = await api("/upstreams");
if (!web.backends[0].drained) fail(`the drain didn't reach the server: ${JSON.stringify(web)}`);
await first.click();
await page.waitForTimeout(500);
if ((await api("/upstreams"))[0].backends[0].drained) fail("undrain didn't reach the server");
console.log("ok: drain and undrain a backend from the UI");

await page.goto(`${admin}/admin/#/controls`);
const maintenance = page.locator('lit-material-switch[aria-label="Maintenance mode"]');
await maintenance.waitFor({ timeout: 5000 });
await maintenance.click();
await page.getByText("On: 503 for visitors").waitFor({ timeout: 5000 }).catch(() => fail("maintenance didn't switch on in the UI"));
if ((await fetch(publicUrl)).status !== 503) fail("maintenance mode didn't reach the public port");
await maintenance.click();
await page.waitForTimeout(500);
if ((await api("/maintenance")).active) fail("maintenance didn't switch off");
console.log("ok: maintenance mode from the UI");

await page.goto(`${admin}/admin/#/logs`);
await page.waitForTimeout(800);
const marker = `ui-marker-${Date.now()}`;
await fetch(`${publicUrl}${marker}`);
await page.getByText(marker).waitFor({ timeout: 5000 }).catch(() => fail("the request didn't appear in the live log"));
console.log("ok: live log tail");

await browser.close();
if (problems.length) fail(problems.join("\n"));
console.log("ok: no CSP violations or script errors");
