// The admin UI in headless Chromium against a running rws (ci/e2e/admin_ui.sh):
// no CSP violations or script errors, a wrong token refused, then drain a
// backend, switch maintenance mode and purge a cache through the UI and see
// the API agree, PURGE over HTTP, and a request appear in the live log.
import { chromium } from "playwright";

const [admin, token, publicUrl, purgeToken, externalUrl] = process.argv.slice(2);
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

const entries = async () => (await api("/caches")).find((c) => c.name === "route:pages")?.entries;
for (const path of ["cached/a", "cached/b", "cached/c/1"]) await fetch(`${publicUrl}${path}`);
if ((await entries()) !== 3) fail(`expected 3 cached pages, got ${await entries()}`);
await page.reload();
await page.locator("#purge-path input").fill("/cached/c/*");
await page.getByRole("button", { name: "Purge" }).first().click();
await page.getByText("1 entry purged from route:pages").waitFor({ timeout: 5000 }).catch(() => fail("no purge confirmation in the UI"));
if ((await entries()) !== 2) fail(`the prefix purge left ${await entries()} entries`);
const purge = (auth) => fetch(`${publicUrl}cached/a`, { method: "PURGE", headers: auth ? { Authorization: `Bearer ${auth}` } : {} });
if ((await purge("wrong")).status !== 401) fail("PURGE with a wrong token wasn't refused");
const purged = await purge(purgeToken);
if (purged.status !== 200 || (await purged.text()) !== '{"purged":1}') fail("PURGE with the token didn't purge one entry");
if ((await entries()) !== 1) fail(`PURGE left ${await entries()} entries`);
console.log("ok: purge a cache from the UI and with HTTP PURGE");

await page.locator("#authz-tuple input").fill("doc:42#viewer@user:alice");
await page.getByRole("button", { name: "Add" }).last().click();
await page.getByText("doc:42#viewer@user:alice added").waitFor({ timeout: 5000 }).catch(() => fail("no confirmation for the added tuple"));
if (!(await api("/authz/tuples")).tuples.includes("doc:42#viewer@user:alice")) fail("the tuple didn't reach the server");
await page.locator("#authz-subject input").fill("user:alice");
await page.locator("#authz-relation input").fill("viewer");
await page.locator("#authz-object input").fill("doc:42");
await page.getByRole("button", { name: "Check" }).click();
await page.locator("#authz-result").getByText("Allowed").waitFor({ timeout: 5000 }).catch(() => fail("the check didn't show Allowed"));
console.log("ok: authorization tuple added and checked from the UI");

// Visitors from outside: a person (a page and its stylesheet), an AI agent
// claiming GPTBot (not from OpenAI's network: spoofed), and curl.
const browserUA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const visit = (path, ua, headers = {}) => fetch(`${externalUrl}${path}`, { headers: { "User-Agent": ua, ...headers } });
// Arrives from a search engine with campaign tags (18.31 sources/audience).
await visit("?utm_source=newsletter&utm_campaign=launch", browserUA, { Referer: "https://www.google.com/", "Sec-CH-UA-Platform": '"Linux"', "Sec-CH-UA-Mobile": "?0" });
await visit("site.css", browserUA);
await visit("", "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.3; +https://openai.com/gptbot)");
await visit("", "curl/8.5.0");
const traffic = await api("/traffic?hours=24");
if (traffic.totals.people.visitors !== 1 || traffic.totals.people.pages !== 1) fail(`people: ${JSON.stringify(traffic.totals.people)}`);
if (!traffic.bots.some((b) => b.kind === "spoofed" && b.name === "GPTBot")) fail(`spoofed GPTBot not counted: ${JSON.stringify(traffic.bots)}`);
if (!traffic.bots.some((b) => b.kind === "tool")) fail(`curl not counted: ${JSON.stringify(traffic.bots)}`);
const has = (list, key, n) => (list ?? []).some(([k, v]) => k === key && v === n);
if (!has(traffic.browsers, "Chrome", 1) || !has(traffic.os, "Linux", 1) || !has(traffic.devices, "desktop", 1)) fail(`audience: ${JSON.stringify([traffic.browsers, traffic.os, traffic.devices])}`);
if (!has(traffic.channels, "campaign", 1) || !has(traffic.utm_source, "newsletter", 1) || !has(traffic.utm_campaign, "launch", 1)) fail(`sources: ${JSON.stringify([traffic.channels, traffic.utm_source, traffic.utm_campaign])}`);
if ((traffic.countries ?? []).length !== 1) fail(`countries: ${JSON.stringify(traffic.countries)}`);
await page.goto(`${admin}/admin/#/traffic`);
await page.locator('lit-material-stat[label="People: visitors"]').waitFor({ timeout: 5000 }).catch(() => fail("no Traffic page"));
await page.locator("lit-material-line-chart").waitFor({ timeout: 5000 }).catch(() => fail("no traffic chart"));
await page.getByRole("heading", { name: "Sources", exact: true }).waitFor({ timeout: 5000 }).catch(() => fail("no Sources section"));
await page.getByRole("heading", { name: "Audience", exact: true }).waitFor({ timeout: 5000 }).catch(() => fail("no Audience section"));
await page.locator('lit-material-bar-chart[label="Visits by channel"]').waitFor({ timeout: 5000 }).catch(() => fail("no channel chart"));
await page.getByRole("button", { name: "Week" }).click();
await page.getByText("Who visited in the last week").waitFor({ timeout: 5000 }).catch(() => fail("the range didn't switch"));
console.log("ok: Traffic page counts a person, a spoofed GPTBot and curl");

await page.goto(`${admin}/admin/#/logs`);
await page.waitForTimeout(800);
const marker = `ui-marker-${Date.now()}`;
await fetch(`${publicUrl}${marker}`);
await page.getByText(marker).waitFor({ timeout: 5000 }).catch(() => fail("the request didn't appear in the live log"));
console.log("ok: live log tail");

// Signed-in user in the header, and the changes above on the Audit page (18.22.0).
await page.getByText("admin · admin").first().waitFor({ timeout: 5000 }).catch(() => fail("the header doesn't show the signed-in user and role"));
await page.goto(`${admin}/admin/#/audit`);
await page.getByText("PUT /maintenance").first().waitFor({ timeout: 5000 }).catch(() => fail("the Audit page doesn't list the maintenance change"));
console.log("ok: signed-in user shown; Audit page lists the changes made");

await browser.close();
if (problems.length) fail(problems.join("\n"));
console.log("ok: no CSP violations or script errors");
