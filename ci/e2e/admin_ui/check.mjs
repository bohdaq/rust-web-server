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
if (traffic.sessions?.sessions !== 1 || traffic.sessions.bounces !== 1) fail(`sessions: ${JSON.stringify(traffic.sessions)}`);
if (!has(traffic.entry_pages, (traffic.entry_pages ?? [])[0]?.[0], 1)) fail(`entry pages: ${JSON.stringify(traffic.entry_pages)}`);
// The first-party script in a real browser (as a person: a desktop
// Firefox UA, since HeadlessChrome is a known bot): an event, a route
// change, then leaving the page reports time and scroll.
{
  const ctx = await browser.newContext({ userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:144.0) Gecko/20100101 Firefox/144.0", viewport: { width: 1280, height: 800 } });
  await ctx.addInitScript(() => { window.rwsTrackAutomation = true; });
  const visitor = await ctx.newPage();
  await visitor.goto(`${externalUrl}app.html`);
  await visitor.waitForFunction(() => window.rws && typeof window.rws.track === "function", null, { timeout: 5000 }).catch(() => fail("the script didn't load"));
  await visitor.evaluate(() => { window.scrollTo(0, 2000); window.rws.track("signup", { plan: "pro" }); history.pushState({}, "", "/thanks/"); });
  await visitor.waitForTimeout(300);
  // A second route change reports leaving /thanks/. (Not a navigation away:
  // headless Chromium sends unload-time beacons with its own HeadlessChrome
  // User-Agent, ignoring the context's — another visitor, a bot.)
  await visitor.evaluate(() => history.pushState({}, "", "/done/"));
  await visitor.waitForTimeout(300);
  await ctx.close();
  let t;
  for (let i = 0; i < 30; i++) {
    t = await api("/traffic?hours=24");
    if ((t.events ?? []).length && (t.time_on_page ?? []).length >= 2) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  const pair = (list, key) => (list ?? []).find(([k]) => k === key)?.[1];
  if (pair(t.events, "signup") !== 1) fail(`script event: ${JSON.stringify(t.events)}`);
  if (pair(t.event_props, "signup\tplan=pro") !== 1) fail(`event props: ${JSON.stringify(t.event_props)}`);
  if (pair(t.goals, "event:signup") !== 1 || pair(t.goals, "/thanks/") !== 1) fail(`goals: ${JSON.stringify(t.goals)}`);
  if ((t.time_on_page ?? []).length < 2) fail(`time on page: ${JSON.stringify(t.time_on_page)}`);
  if (pair(t.screens, "laptop") < 1) fail(`screens: ${JSON.stringify(t.screens)}`);
  if (t.totals.people.visitors !== 2) fail(`people after the script visit: ${JSON.stringify(t.totals.people)}`);
  console.log("ok: first-party script reports an event, goals, a route change and time on page");
}
await page.goto(`${admin}/admin/#/traffic`);
const live = await api("/traffic/live");
if (live.visitors !== 2 || live.views_5m !== 4) fail(`live: ${JSON.stringify(live)}`);
// 18.35: the script visitor is verified and in the visit feed; an app's
// requests and events are the app's, not a bot's; a scanner's probe is
// listed without its address; the owner link takes a browser out.
{
  const site = externalUrl.replace(/\/$/, "");
  const ua = "HITOP/2.3.1 CFNetwork/3826.500.111 Darwin/25.0.0";
  await fetch(`${site}/api/ping`, { headers: { "user-agent": ua } });
  const event = await fetch(`${site}/_rws/e`, { method: "POST", headers: { "user-agent": ua }, body: JSON.stringify({ app: "HITOP", v: "2.3.1", p: "ios", n: "opened" }) });
  if (event.status !== 204) fail(`app event: ${event.status}`);
  const unknown = await fetch(`${site}/_rws/e`, { method: "POST", body: JSON.stringify({ app: "Other", n: "x" }) });
  if (unknown.status !== 403) fail(`unknown app event: ${unknown.status}`);
  await fetch(`${site}/.git/config`, { headers: { "user-agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Safari/537.36" } });
  const t = await api("/traffic?hours=24");
  if (t.tiers?.verified !== 1) fail(`tiers: ${JSON.stringify(t.tiers)}`);
  if (!(t.visits ?? []).some((v) => v.verified && v.pages.includes("/thanks/"))) fail(`visits: ${JSON.stringify(t.visits)}`);
  if (t.totals.apps?.visitors !== 1) fail(`apps: ${JSON.stringify(t.totals.apps)}`);
  if (!(t.apps?.events ?? []).some(([k, n]) => k === "HITOP\topened" && n === 1)) fail(`app events: ${JSON.stringify(t.apps)}`);
  if (!(t.apps?.versions ?? []).some(([k]) => k === "HITOP\t2.3.1")) fail(`app versions: ${JSON.stringify(t.apps)}`);
  if (!(t.probes?.paths ?? []).some(([k]) => k === "/.git/config")) fail(`probes: ${JSON.stringify(t.probes)}`);
  const owner = await api("/traffic/owner");
  if (!owner.path?.startsWith("/_rws/me?k=")) fail(`owner: ${JSON.stringify(owner)}`);
  const me = await browser.newContext({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15" });
  const mine = await me.newPage();
  const res = await mine.goto(`${site}${owner.path}`);
  if (res.status() !== 200 || !(await me.cookies()).some((c) => c.name === "rws_me")) fail("the owner link set no cookie");
  await mine.goto(`${site}/app.html`);
  await mine.waitForTimeout(300);
  await me.close();
  const after = await api("/traffic?hours=24");
  if (after.totals.people.visitors !== 2) fail(`the owner counted as a person: ${JSON.stringify(after.totals)}`);
  if ((after.totals.owner?.pages ?? 0) < 1) fail(`owner pages: ${JSON.stringify(after.totals.owner)}`);
  console.log("ok: verified visitor, visit feed, app user and events, probe, owner link");
  // The page loaded before these: show them now.
  await page.reload();
}
const ranged = await api(`/traffic?from=${new Date().toISOString().slice(0, 10)}`);
if (!Array.isArray(ranged.previous_series) || !Array.isArray(ranged.days)) fail(`range: ${JSON.stringify(Object.keys(ranged))}`);
// The Traffic view (18.32 layout): key numbers, chart, cards, tabs.
const t2 = page.locator("rws-traffic");
await t2.locator(".t-kpi", { hasText: "Visitors" }).waitFor({ timeout: 8000 }).catch(() => fail("no Traffic key numbers"));
await t2.locator(".t-kpi", { hasText: "Bounce rate" }).waitFor({ timeout: 5000 }).catch(() => fail("no bounce rate"));
await t2.locator("lit-material-line-chart").waitFor({ timeout: 5000 }).catch(() => fail("no traffic chart"));
await t2.getByRole("heading", { name: "Top pages", exact: true }).waitFor({ timeout: 5000 }).catch(() => fail("no Top pages card"));
await t2.getByRole("heading", { name: "Right now", exact: true }).waitFor({ timeout: 5000 }).catch(() => fail("no Right now card"));
const visitorsShown = (await t2.locator(".t-kpi", { hasText: "Visitors" }).locator(".t-kpi-value").textContent()).trim();
if (visitorsShown !== String(live.visitors)) console.log(`(visitors tile ${visitorsShown}, live ${live.visitors})`);
await t2.locator(".t-tiers", { hasText: "Verified" }).waitFor({ timeout: 5000 }).catch(() => fail("no person tiers"));
await t2.getByRole("tab", { name: "Visitors" }).click();
await t2.locator(".t-visit").first().waitFor({ timeout: 5000 }).catch(() => fail("no visit in the feed"));
for (const [tab, heading] of [["Pages", "Entry pages"], ["Sources", "Channels"], ["Audience", "Browsers"], ["Behaviour", "Goals"]]) {
  await t2.getByRole("tab", { name: tab }).click();
  await t2.getByRole("heading", { name: heading, exact: true }).waitFor({ timeout: 5000 }).catch(() => fail(`no ${heading} on the ${tab} tab`));
}
await t2.getByRole("tab", { name: "Overview" }).click();
if ((await t2.locator(".t-map svg path").count()) < 150) fail("no world map");
// A country is shaded only when the visitors' address has one (a CI runner
// reaches itself over a private address: no country).
const known = (ranged.countries ?? []).some(([code]) => /^[A-Z]{2}$/.test(code));
if (known && (await t2.locator(".t-map path.on").count()) < 1) fail("the map shows no country");
// Humans / AI agents / Bots / All.
for (const [group, heading] of [["Apps", "App events"], ["AI agents", "AI agents"], ["Bots", "Probes"], ["All", "Share of requests"]]) {
  await t2.locator(".t-seg button", { hasText: group }).click();
  await t2.getByRole("heading", { name: heading, exact: true }).waitFor({ timeout: 5000 }).catch(() => fail(`no ${heading} for ${group}`));
}
await t2.locator(".t-seg button", { hasText: "Humans" }).click();
// Without a database, clicking a row explains what filtering needs.
await t2.locator("button.t-row").first().click();
await t2.locator(".t-notice").waitFor({ timeout: 5000 }).catch(() => fail("no notice for a filter without the database"));
await t2.getByRole("button", { name: "30 days" }).click();
await t2.locator('.t-seg button[aria-pressed="true"]', { hasText: "30 days" }).waitFor({ timeout: 5000 }).catch(() => fail("the period didn't switch"));
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
