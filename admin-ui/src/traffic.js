// The Traffic view: a sticky filter bar (period, site, filters), a row of
// key numbers that choose the chart's metric, the chart against the period
// before, and cards of top lists whose "All" opens a tab with the full
// list. Clicking a row filters the page by it: the site always, anything
// else with the traffic database ([traffic] database_url_env), whose rows
// can answer any combination.
import { LitElement, html, nothing } from "lit";
import { api, ApiError } from "./api.js";
import { formatDuration } from "@lit-material/format";
import { live } from "lit/directives/live.js";
import "@lit-material/world-map";

const TABS = [
  ["overview", "Overview"],
  ["visitors", "Visitors"],
  ["pages", "Pages"],
  ["sources", "Sources"],
  ["audience", "Audience"],
  ["behaviour", "Behaviour"],
];

// Who the page is about: people (the analytics), AI agents, other bots, or
// all three side by side.
const GROUPS = [
  ["people", "Humans"],
  ["apps", "Apps"],
  ["ai", "AI agents"],
  ["bots", "Bots"],
  ["all", "All"],
];
const GROUP_NAMES = { people: "Humans", apps: "Apps", ai: "AI agents", bots: "Bots" };

const PRESETS = [
  ["today", "Today", 0],
  ["7d", "7 days", 6],
  ["30d", "30 days", 29],
];

// Filter dimensions: what a click on a row of that list sets.
const DIMS = {
  page: "Page",
  country: "Country",
  channel: "Channel",
  utm_source: "Source",
  utm_campaign: "Campaign",
  browser: "Browser",
  os: "System",
  device: "Device",
};

const CHANNELS = { direct: "Direct", search: "Search", social: "Social", ai: "AI assistants", store: "App stores", referral: "Other sites", campaign: "Campaigns", "(none)": "Direct" };
const PLATFORMS = { ios: "iOS", ipados: "iPadOS", macos: "macOS", apple: "iOS/macOS", android: "Android", windows: "Windows", linux: "Linux", chrome: "Chrome", edge: "Edge", firefox: "Firefox", safari: "Safari", web: "Web", other: "Other" };
const DOWNLOAD_GROUPS = { people: "people", apps: "apps", bots: "bots, tools", ai: "AI", owner: "you", undecided: "undecided" };

/** "a\tb" keys from the server, split. */
const parts = (key) => key.split("\t");

/** How long ago `t` (seconds) was. */
function ago(t) {
  const s = Math.max(0, Math.round(Date.now() / 1000 - t));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86_400) return `${Math.floor(s / 3600)} h ago`;
  return new Date(t * 1000).toLocaleDateString([], { month: "short", day: "numeric" });
}
const DEVICES = { desktop: "Desktop", mobile: "Phone", tablet: "Tablet" };
const SCREENS = { phone: "Phone (< 600 px)", tablet: "Tablet (600–1023 px)", laptop: "Laptop (1024–1439 px)", wide: "Wide (≥ 1440 px)", "(unknown)": "Unknown" };

const day = (offset = 0) => new Date(Date.now() - offset * 86_400_000).toISOString().slice(0, 10);
const num = (n) => (n ?? 0).toLocaleString();
const secs = (s) => (s >= 1 ? formatDuration(Math.round(s) * 1000, { unitDisplay: "narrow" }) : "0s");
const pct = (x) => `${Math.round(x * 100)}%`;

let regions = null;
try {
  regions = new Intl.DisplayNames([navigator.language || "en"], { type: "region" });
} catch {
  /* older browsers: codes */
}
const countryName = (code) => {
  if (code === "(unknown)") return "Unknown";
  if (code === "(other)") return "Other";
  try {
    return regions?.of(code) ?? code;
  } catch {
    return code;
  }
};

/** Change from `before` to `now`, as text and direction. */
function change(now, before, { points = false, lowerIsBetter = false } = {}) {
  if (before === null || before === undefined || (!points && !before)) return null;
  const d = points ? (now - before) * 100 : ((now - before) / before) * 100;
  const rounded = Math.round(d);
  if (rounded === 0) return { text: points ? "±0 pt" : "±0%", tone: "flat" };
  const up = rounded > 0;
  return { text: `${up ? "+" : "−"}${Math.abs(rounded)}${points ? " pt" : "%"}`, tone: up !== lowerIsBetter ? "good" : "bad" };
}

export class RwsTraffic extends LitElement {
  static properties = {
    tab: { state: true },
    group: { state: true },
    preset: { state: true },
    from: { state: true },
    to: { state: true },
    host: { state: true },
    filters: { state: true },
    metric: { state: true },
    compare: { state: true },
    data: { state: true },
    live: { state: true },
    query: { state: true },
    notice: { state: true },
    off: { state: true },
    customOpen: { state: true },
    owner: { state: true },
  };

  constructor() {
    super();
    this.tab = "overview";
    this.group = "people";
    this.preset = "7d";
    this.from = day(6);
    this.to = "";
    this.host = "";
    this.filters = {};
    this.metric = "pages";
    this.compare = true;
    this.data = undefined;
    this.live = null;
    this.query = null;
    this.notice = "";
    this.off = false;
    this.customOpen = false;
    this.owner = null;
    try {
      const saved = JSON.parse(localStorage.getItem("rws-traffic") ?? "{}");
      if (TABS.some(([id]) => id === saved.tab)) this.tab = saved.tab;
      if (typeof saved.compare === "boolean") this.compare = saved.compare;
      if (GROUPS.some(([id]) => id === saved.group)) this.group = saved.group;
    } catch {
      /* storage unavailable: defaults */
    }
  }

  // Global styles (styles.css) apply: no shadow root.
  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.reload();
    this.timer = setInterval(() => {
      // A period ending today keeps moving; a past one doesn't.
      if (!this.to || this.to >= day(0)) this.reload({ quiet: true });
    }, 10_000);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    clearInterval(this.timer);
  }

  remember() {
    try {
      localStorage.setItem("rws-traffic", JSON.stringify({ tab: this.tab, compare: this.compare, group: this.group }));
    } catch {
      /* not kept */
    }
  }

  get filtered() {
    return Object.keys(this.filters).length > 0;
  }

  range() {
    const p = new URLSearchParams({ from: this.from });
    if (this.to) p.set("to", this.to);
    if (this.host) p.set("host", this.host);
    return p;
  }

  async reload({ quiet = false } = {}) {
    if (!quiet) this.data = undefined;
    try {
      const params = this.range();
      const [data, live] = await Promise.all([
        api(`/traffic?${params}`),
        api(`/traffic/live?${this.host ? `host=${encodeURIComponent(this.host)}` : ""}`).catch(() => null),
      ]);
      this.data = data;
      this.live = live;
      this.off = false;
      if (this.filtered) await this.runQuery();
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) this.off = true;
      else this.notice = e.message ?? String(e);
    }
  }

  // The database answers filtered views, and the period before for comparing.
  async runQuery() {
    const days = this.periodDays();
    const params = this.range();
    if (!params.get("to")) params.set("to", day(0));
    for (const [k, v] of Object.entries(this.filters)) {
      if (k === "page") {
        const [h, ...rest] = v.split("/");
        params.set("path", `/${rest.join("/")}`);
        if (!this.host) params.set("host", h);
      } else params.set(k, v);
    }
    const previous = new URLSearchParams(params);
    const fromMs = Date.parse(params.get("from"));
    previous.set("to", new Date(fromMs - 86_400_000).toISOString().slice(0, 10));
    previous.set("from", new Date(fromMs - days * 86_400_000).toISOString().slice(0, 10));
    try {
      const [now, before] = await Promise.all([api(`/traffic/query?${params}`), api(`/traffic/query?${previous}`).catch(() => null)]);
      this.query = { now, before };
    } catch (e) {
      this.query = null;
      this.notice = e.message ?? String(e);
    }
  }

  periodDays() {
    const to = this.to || day(0);
    return Math.round((Date.parse(to) - Date.parse(this.from)) / 86_400_000) + 1;
  }

  setPreset(id) {
    const preset = PRESETS.find(([p]) => p === id);
    this.preset = id;
    this.customOpen = false;
    this.from = day(preset[2]);
    this.to = "";
    this.reload();
  }

  applyCustom(event) {
    event.preventDefault();
    const from = this.querySelector("#traffic-from")?.value;
    const to = this.querySelector("#traffic-to")?.value;
    if (!from) return;
    if (to && to < from) {
      this.notice = "The start date is after the end date.";
      return;
    }
    this.preset = "custom";
    this.from = from;
    this.to = to && to < day(0) ? to : "";
    this.customOpen = false;
    this.notice = "";
    this.reload();
  }

  setHost(host) {
    this.host = host;
    this.reload();
  }

  /** Filter by a row: the site always; anything else needs the database. */
  pick(dim, value) {
    if (!this.data?.store) {
      this.notice = `Filtering by ${DIMS[dim].toLowerCase()} needs the traffic database: set [traffic] database_url_env.`;
      return;
    }
    this.notice = "";
    this.filters = { ...this.filters, [dim]: value };
    this.tab = "overview";
    this.runQuery();
  }

  unpick(dim) {
    const next = { ...this.filters };
    delete next[dim];
    this.filters = next;
    if (this.filtered) this.runQuery();
    else this.query = null;
  }

  setTab(id) {
    this.tab = id;
    this.remember();
  }

  /** The owner link and the sites to open it on (GET /traffic/owner). */
  async showOwner() {
    if (this.owner) {
      this.owner = null;
      return;
    }
    try {
      this.owner = await api("/traffic/owner");
    } catch (e) {
      this.notice = e.message ?? String(e);
    }
  }

  setGroup(id) {
    this.group = id;
    if (id !== "people") {
      this.tab = "overview";
      this.metric = "requests";
    } else if (this.metric === "requests") {
      this.metric = "pages";
    }
    this.remember();
  }

  // ── names ────────────────────────────────────────────────────────────────

  /** A page key `host/path`, without the host when one site is shown. */
  pageName(key) {
    if (key === "(other)") return "Other pages";
    const single = this.host || (this.data?.hosts?.length ?? 0) <= 1;
    if (!single) return key;
    const i = key.indexOf("/");
    return i >= 0 ? key.slice(i) : key;
  }

  label(dim, key) {
    if (key === "(other)") return "Other";
    switch (dim) {
      case "page":
        return this.pageName(key);
      case "country":
        return countryName(key);
      case "channel":
        return CHANNELS[key] ?? key;
      case "device":
        return DEVICES[key] ?? key;
      default:
        return key;
    }
  }

  // ── pieces ───────────────────────────────────────────────────────────────

  /** A ranked list with inline bars; rows filter when `dim` is given. */
  ranked(rows, { dim = null, unit = "", limit = 0, value = (r) => num(r[1]), flag = false, pages = false } = {}) {
    const list = limit ? rows.slice(0, limit) : rows;
    if (!list.length) return html`<p class="t-none">Nothing in this period.</p>`;
    const max = Math.max(...list.map((r) => r[1]), 1);
    const clickable = dim && dim in DIMS && !(dim in this.filters);
    return html`<ol class="t-rank" aria-label=${unit}>
      ${list.map(([key, n, ...rest]) => {
        const name = dim ? this.label(dim, key) : pages ? this.pageName(key) : key;
        const body = html`<span class="t-bar" style=${`width:${(n / max) * 100}%`}></span>
          <span class="t-name" title=${key}>${flag && key.length === 2 ? html`<span class="t-code">${key}</span>` : nothing}${name}</span>
          <span class="t-num">${value([key, n, ...rest])}</span>`;
        return html`<li>${clickable && key !== "(other)" && key !== "(unknown)"
          ? html`<button class="t-row" title=${`Filter by ${name}`} @click=${() => this.pick(dim, key)}>${body}</button>`
          : html`<div class="t-row">${body}</div>`}</li>`;
      })}
    </ol>`;
  }

  card(title, rows, opts = {}, tab = null) {
    return html`<section class="t-card" aria-label=${title}>
      <header><h3>${title}</h3>${opts.hint ? html`<span class="t-hint">${opts.hint}</span>` : nothing}</header>
      ${this.ranked(rows, { limit: tab ? 6 : 0, ...opts })}
      ${tab && rows.length > 6 ? html`<button class="t-more" @click=${() => this.setTab(tab)}>All ${rows.length > 25 ? "25" : rows.length} →</button>` : nothing}
    </section>`;
  }

  /** The numbers of the period (counts or the database's answer). */
  numbers() {
    const t = this.data;
    if (this.query) {
      const { now, before } = this.query;
      return {
        visitors: [now.visitors, before?.visitors],
        pages: [now.views, before?.views],
        sessions: [now.sessions, before?.sessions],
        bounce: [now.sessions ? now.bounces / now.sessions : 0, before?.sessions ? before.bounces / before.sessions : null],
        duration: [null, null],
      };
    }
    const s = t.sessions ?? {};
    const p = t.previous_sessions ?? {};
    const hasBefore = (t.previous?.people?.pages ?? 0) > 0 || (p.sessions ?? 0) > 0;
    return {
      visitors: [t.totals.people.visitors, hasBefore ? t.previous.people.visitors : null],
      pages: [t.totals.people.pages, hasBefore ? t.previous.people.pages : null],
      sessions: [s.sessions ?? 0, hasBefore ? p.sessions ?? 0 : null],
      bounce: [s.sessions ? s.bounces / s.sessions : 0, p.sessions ? p.bounces / p.sessions : null],
      duration: [s.sessions ? s.duration / s.sessions : 0, p.sessions ? p.duration / p.sessions : null],
    };
  }

  /** Which metrics the chart can draw for this view. */
  chartable(metric) {
    if (metric === "pages") return true;
    if (metric === "duration") return false;
    if (this.query) return metric === "visitors";
    return this.data.bucket === "day";
  }

  chartSeries() {
    const t = this.data;
    const m = this.chartable(this.metric) ? this.metric : "pages";
    const names = { pages: "Page views", visitors: "Visitors", sessions: "Sessions", bounce: "Bounce rate" };
    let now;
    let before = null;
    if (this.query) {
      const key = m === "visitors" ? "visitors" : "views";
      now = this.query.now.series.map((p) => ({ x: p.t * 1000, y: p[key] }));
      // The period before, by position (day 1 against day 1).
      const firstMs = Date.parse(this.from);
      if (this.query.before) {
        const prevFrom = firstMs - this.periodDays() * 86_400_000;
        before = this.query.before.series.map((p) => ({ x: p.t * 1000 - prevFrom + firstMs, y: p[key] }));
      }
    } else if (m === "pages") {
      now = t.series.map((p) => ({ x: p.t * 1000, y: p.pages ?? 0 }));
      before = (t.previous_series ?? []).map((p) => ({ x: p.t * 1000, y: p.pages ?? 0 }));
    } else {
      const pick = (d, prev) => {
        const v = prev ? d[`previous_${m === "bounce" ? "sessions" : m}`] : d[m === "bounce" ? "sessions" : m];
        if (m !== "bounce") return v;
        const b = prev ? d.previous_bounces : d.bounces;
        return v ? Math.round((b / v) * 100) : 0;
      };
      now = (t.days ?? []).map((d) => ({ x: d.t * 1000, y: pick(d, false) }));
      before = (t.days ?? []).map((d) => ({ x: d.t * 1000, y: pick(d, true) }));
    }
    const series = [{ name: names[m], points: now }];
    if (this.compare && before && before.some((p) => p.y > 0)) series.push({ name: "Period before", points: before });
    return { series, unit: m === "bounce" ? "%" : names[m].toLowerCase() };
  }

  /** The site options: the configured sites, then other Host headers seen. */
  hostOptions(hosts, configured) {
    const option = (h) => html`<option value=${h} ?selected=${this.host === h}>${h === "other" ? "Other hosts (combined)" : h}</option>`;
    const sites = hosts.filter((h) => configured.includes(h));
    const rest = hosts.filter((h) => !configured.includes(h));
    if (!sites.length || !rest.length) return hosts.map(option);
    return html`<optgroup label="Sites">${sites.map(option)}</optgroup><optgroup label="Other hosts">${rest.map(option)}</optgroup>`;
  }

  // ── sections ─────────────────────────────────────────────────────────────

  renderBar() {
    const t = this.data;
    const hosts = t?.hosts ?? [];
    return html`<div class="t-bar-top">
      <div class="t-seg" role="group" aria-label="Period">
        ${PRESETS.map(([id, label]) => html`<button aria-pressed=${this.preset === id} @click=${() => this.setPreset(id)}>${label}</button>`)}
        <button aria-pressed=${this.preset === "custom"} aria-expanded=${this.customOpen} @click=${() => (this.customOpen = !this.customOpen)}>
          ${this.preset === "custom" ? `${this.from} – ${this.to || "today"}` : "Custom"}
        </button>
      </div>
      <div class="t-seg" role="group" aria-label="Who">
        ${GROUPS.map(([id, label]) => html`<button aria-pressed=${this.group === id} @click=${() => this.setGroup(id)}>${label}</button>`)}
      </div>
      ${hosts.length > 1
        ? html`<label class="t-select"><span class="t-sr">Site</span>
            <select @change=${(e) => this.setHost(e.target.value)}>
              <option value="" ?selected=${!this.host}>All sites</option>
              ${this.hostOptions(hosts, t?.configured ?? [])}
            </select></label>`
        : nothing}
      <label class="t-check"><input type="checkbox" .checked=${this.compare} @change=${(e) => { this.compare = e.target.checked; this.remember(); }}> Compare</label>
      <span class="t-spacer"></span>
      ${this.live
        ? html`<span class="t-live" title=${this.live.pages.map(([p, n]) => `${this.pageName(p)}: ${n}`).join("\n") || "Nobody right now"}>
            <span class="t-dot" data-on=${this.live.visitors > 0}></span>${num(this.live.visitors)} online</span>`
        : nothing}
    </div>
    ${this.customOpen
      ? html`<form class="t-custom" @submit=${(e) => this.applyCustom(e)}>
          <label>From <input id="traffic-from" type="date" .value=${this.from} max=${day(0)} min=${day(89)}></label>
          <label>To <input id="traffic-to" type="date" .value=${this.to || day(0)} max=${day(0)} min=${day(89)}></label>
          <lit-material-button type="submit" variant="tonal">Apply</lit-material-button>
          <span class="t-hint">${t?.store ? "Up to 90 days; any range with the database in the filter results." : "Counts are kept for the configured retention_days (30 by default)."}</span>
        </form>`
      : nothing}
    ${this.filtered || this.host
      ? html`<div class="t-chips" aria-label="Filters">
          ${this.host ? html`<button class="t-chip" @click=${() => this.setHost("")}>Site: ${this.host} <span aria-hidden="true">✕</span></button>` : nothing}
          ${Object.entries(this.filters).map(([k, v]) => html`<button class="t-chip" @click=${() => this.unpick(k)}>${DIMS[k]}: ${this.label(k, v)} <span aria-hidden="true">✕</span></button>`)}
          ${this.filtered ? html`<button class="t-chip t-clear" @click=${() => { this.filters = {}; this.query = null; }}>Clear filters</button>` : nothing}
        </div>`
      : nothing}
    ${this.notice ? html`<p class="t-notice" role="status">${this.notice} <button class="t-more" @click=${() => (this.notice = "")}>Dismiss</button></p>` : nothing}`;
  }

  renderKpis() {
    const n = this.numbers();
    const kpis = [
      ["visitors", "Visitors", num(n.visitors[0]), change(n.visitors[0], n.visitors[1]), "Each person counted once a day"],
      ["pages", "Page views", num(n.pages[0]), change(n.pages[0], n.pages[1]), "Pages people opened"],
      ["sessions", "Sessions", num(n.sessions[0]), change(n.sessions[0], n.sessions[1]), "Page views 30 minutes or less apart"],
      ["bounce", "Bounce rate", pct(n.bounce[0]), change(n.bounce[0], n.bounce[1], { points: true, lowerIsBetter: true }), "Sessions of one page view"],
      ["duration", "Session length", n.duration[0] === null ? "—" : secs(n.duration[0]), n.duration[0] === null ? null : change(n.duration[0], n.duration[1]), "First page view to last, on average"],
    ];
    return html`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${kpis.map(([id, label, value, delta, hint]) => {
        const can = this.chartable(id);
        return html`<button class="t-kpi" aria-pressed=${can && this.metric === id} ?disabled=${!can} title=${hint}
          @click=${() => (this.metric = id)}>
          <span class="t-kpi-label">${label}</span>
          <span class="t-kpi-value">${value}</span>
          <span class="t-kpi-delta" data-tone=${delta?.tone ?? "none"}>${this.compare && delta ? delta.text : " "}</span>
        </button>`;
      })}
    </div>`;
  }

  renderChart() {
    const { series, unit } = this.chartSeries();
    const hourly = !this.query && this.data.bucket === "hour" && this.chartable(this.metric) && this.metric === "pages";
    return html`<section class="t-chart" aria-label="Chart">
      <lit-material-line-chart label=${`${series[0].name} per ${hourly ? "hour" : "day"}`} unit=${unit} height="240" .series=${series}
        .formatX=${(x) => (hourly ? new Date(x).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : new Date(x).toLocaleDateString([], { month: "short", day: "numeric" }))}></lit-material-line-chart>
    </section>`;
  }

  /** The lists a tab or card shows: from the counts, or the database's answer. */
  lists() {
    const t = this.data;
    if (this.query) {
      const q = this.query.now;
      return { pages: q.pages, channels: q.channels, referrers: q.referrers, utm_source: q.utm_source, utm_campaign: q.utm_campaign, utm_medium: [], countries: q.countries, browsers: q.browsers, os: q.os, devices: q.devices, entry: [], exit: [], not_found: [] };
    }
    return {
      pages: t.people_pages,
      channels: t.channels ?? [],
      referrers: t.referrers,
      utm_source: t.utm_source ?? [],
      utm_medium: t.utm_medium ?? [],
      utm_campaign: t.utm_campaign ?? [],
      countries: t.countries ?? [],
      browsers: t.browsers ?? [],
      os: t.os ?? [],
      devices: t.devices ?? [],
      entry: t.entry_pages ?? [],
      exit: t.exit_pages ?? [],
      not_found: t.not_found,
    };
  }

  renderOverview() {
    const l = this.lists();
    const live = this.live;
    return html`
      ${this.renderKpis()}
      ${this.query ? nothing : this.renderTiers()}
      ${this.renderChart()}
      <div class="t-cards">
        ${this.card("Top pages", l.pages, { dim: "page", unit: "views" }, "pages")}
        ${this.card("Sources", l.channels, { dim: "channel", unit: "visits" }, "sources")}
        <section class="t-card t-wide" aria-label="Countries">
          <header><h3>Countries</h3>${l.countries.length > 6 ? html`<button class="t-more" @click=${() => this.setTab("audience")}>All ${l.countries.length} →</button>` : nothing}</header>
          <div class="t-map-row">
            ${this.renderMap(l.countries)}
            ${this.ranked(l.countries, { dim: "country", unit: "visitors", flag: true, limit: 6 })}
          </div>
        </section>
        ${this.card("Devices", l.devices, { dim: "device", unit: "visitors" }, "audience")}
        ${live && !this.filtered
          ? html`<section class="t-card" aria-label="Right now">
              <header><h3>Right now</h3><span class="t-hint">${num(live.visitors)} ${live.visitors === 1 ? "person" : "people"}, last 5 minutes</span></header>
              ${live.series.some((p) => p.views > 0)
                ? html`<lit-material-sparkline label="Page views per minute, last 30 minutes" .values=${live.series.map((p) => p.views)}></lit-material-sparkline>
                    ${this.ranked(live.pages, { dim: "page", limit: 5, unit: "views" })}`
                : html`<p class="t-none">Nobody has opened a page in the last 30 minutes.</p>`}
            </section>`
          : nothing}
      </div>`;
  }

  /** How sure the count of people is, and the owner's own visits apart. */
  renderTiers() {
    const t = this.data;
    const tiers = t.tiers ?? { verified: 0, page_only: 0, hosting: 0 };
    const people = t.totals.people.visitors;
    const likely = Math.max(0, people - tiers.verified);
    const mine = t.totals.owner ?? { pages: 0, visitors: 0 };
    const items = [
      ...(t.script ? [["Verified", tiers.verified, "Ran the page script: a real browser showing the page"]] : []),
      [t.script ? "Likely" : "People", t.script ? likely : people, "Loaded the page's styles or scripts, as a browser does"],
      ["Page only", tiers.page_only, "A browser User-Agent that loaded no asset: counted as a bot"],
      ["Server networks", tiers.hosting, "A browser User-Agent from a cloud or hosting network: counted as a bot"],
    ];
    return html`<div class="t-tiers" role="group" aria-label="How sure">
      ${items.map(([label, n, hint]) => html`<span class="t-tier" title=${hint}><b>${num(n)}</b> ${label}</span>`)}
      <span class="t-spacer"></span>
      <span class="t-tier t-owner" title="Your own visits (admin sign-in, owner networks, or this browser's owner link) aren't in these numbers">
        ${mine.pages ? html`<b>${num(mine.pages)}</b> of your page views not counted` : "Your visits aren't counted"}
        <button class="t-more" @click=${() => this.showOwner()}>${this.owner ? "Hide" : "Exclude a browser…"}</button>
      </span>
    </div>
    ${this.owner ? this.renderOwner() : nothing}`;
  }

  renderOwner() {
    const o = this.owner;
    // The configured sites, else names seen (not addresses, not localhost).
    const sites = o.sites.length ? o.sites : (this.data?.hosts ?? []).filter((h) => /[a-z]/i.test(h) && h.includes(".") && !h.includes(":") && h !== "other");
    return html`<section class="t-card t-wide" aria-label="Exclude a browser">
      <header><h3>Exclude a browser</h3><span class="t-hint">Open the link in each browser and on each phone you use</span></header>
      <p class="t-foot">Signing in here already excludes this browser for the day. A link sets a cookie on that site for a year, so visits from that browser count as yours even without signing in. Keep the links private.${o.networks ? " Your [traffic] owner_networks are excluded too." : ""}</p>
      <ul class="t-links">
        ${sites.map((h) => {
          const url = `https://${h}${o.path}`;
          return html`<li><a href=${url} target="_blank" rel="noopener noreferrer">${h}</a>
            <button class="t-more" @click=${() => navigator.clipboard?.writeText(url)}>Copy link</button></li>`;
        })}
      </ul>
    </section>`;
  }

  /** The recent-visitors feed: one card per visit, newest first. */
  renderVisitors() {
    const visits = this.data.visits ?? [];
    if (!visits.length) {
      return html`<lit-material-empty-state headline="No visits yet">Visits appear here when they end (after 30 quiet minutes), and while they're open.</lit-material-empty-state>`;
    }
    const many = !this.host && (this.data.hosts?.length ?? 0) > 1;
    return html`<ol class="t-visits" aria-label="Recent visits">
      ${visits.map((v) => {
        const how = v.channel === "direct" || !v.channel ? "Direct" : `${CHANNELS[v.channel] ?? v.channel}${v.referrer ? ` · ${v.referrer}` : ""}`;
        const device = [DEVICES[v.device] ?? v.device, v.os, v.browser].filter(Boolean).join(" · ");
        const more = v.views - v.pages.length;
        return html`<li class="t-visit">
          <header>
            <span class="t-visit-who">${v.country ? html`<span class="t-code">${v.country}</span>${countryName(v.country)}` : "Unknown country"}</span>
            ${v.verified ? html`<lit-material-status status="ok">Verified</lit-material-status>` : nothing}
            ${v.number > 1 ? html`<lit-material-status status="info">Visit ${v.number} today</lit-material-status>` : nothing}
            <span class="t-spacer"></span>
            <span class="t-hint">${ago(v.end)} · ${v.views === 1 ? "one page" : `${v.views} pages in ${secs(v.end - v.start)}`}</span>
          </header>
          <p class="t-visit-meta">${many ? html`<b>${v.host}</b> · ` : nothing}${how}${v.campaign ? ` · campaign ${v.campaign}` : ""} · ${device || "Unknown device"}</p>
          <ol class="t-path">${v.pages.map((p) => html`<li>${p}</li>`)}${more > 0 ? html`<li>+${more} more</li>` : nothing}</ol>
          ${v.events.length ? html`<p class="t-visit-meta">Events: ${v.events.join(", ")}</p>` : nothing}
        </li>`;
      })}
    </ol>
    <p class="t-foot">The last ${visits.length} visits kept (up to 300 across days). No addresses or User-Agents are kept; a person who comes back another day is a new visitor.</p>`;
  }

  renderPages() {
    const l = this.lists();
    return html`<div class="t-cards">
      ${this.card("Top pages", l.pages, { dim: "page", unit: "views" })}
      ${l.entry.length ? this.card("Entry pages", l.entry, { dim: "page", unit: "sessions", hint: "Where sessions began" }) : nothing}
      ${l.exit.length ? this.card("Exit pages", l.exit, { dim: "page", unit: "sessions", hint: "Where sessions ended" }) : nothing}
      ${l.not_found.length ? this.card("Not found", l.not_found, { unit: "requests", hint: "Paths answered 404", pages: true }) : nothing}
      ${this.query ? nothing : this.renderLandings()}
      ${this.query ? nothing : this.renderDownloads()}
    </div>`;
  }

  /** Where visits from outside began, and from which source. */
  renderLandings() {
    const rows = (this.data.entries ?? []).map(([k, n]) => {
      const [page, channel] = parts(k);
      return [`${this.pageName(page)} · ${CHANNELS[channel] ?? channel}`, n];
    });
    return rows.length ? this.card("Landing pages by source", rows, { unit: "visits", hint: "First page of visits from outside" }) : nothing;
  }

  renderDownloads(only = null) {
    const rows = (this.data.downloads ?? [])
      .map(([k, n]) => {
        const [page, group] = parts(k);
        return [page, group, n];
      })
      .filter(([, g]) => !only || only.includes(g))
      .map(([page, g, n]) => [`${this.pageName(page)} · ${DOWNLOAD_GROUPS[g] ?? g}`, n]);
    return rows.length ? this.card("Downloads", rows, { unit: "downloads", hint: "Installers, packages, install scripts" }) : nothing;
  }

  renderSources() {
    const l = this.lists();
    return html`<div class="t-cards">
      ${this.card("Channels", l.channels, { dim: "channel", unit: "visits", hint: "How sessions arrived" })}
      ${this.card("Referring sites", l.referrers, { unit: "visits" })}
      ${l.utm_source.length || l.utm_campaign.length
        ? html`${this.card("Campaign sources", l.utm_source, { dim: "utm_source", unit: "visits", hint: "utm_source" })}
            ${l.utm_medium.length ? this.card("Campaign media", l.utm_medium, { unit: "visits", hint: "utm_medium" }) : nothing}
            ${this.card("Campaigns", l.utm_campaign, { dim: "utm_campaign", unit: "visits", hint: "utm_campaign" })}`
        : nothing}
    </div>`;
  }

  renderAudience() {
    const l = this.lists();
    const t = this.data;
    const share = (rows) => {
      const all = rows.reduce((s, [, n]) => s + n, 0) || 1;
      return (r) => `${num(r[1])} · ${pct(r[1] / all)}`;
    };
    return html`<section class="t-card t-wide t-map-big" aria-label="Map">${this.renderMap(l.countries)}</section>
    <div class="t-cards">
      ${this.card("Countries", l.countries, { dim: "country", unit: "visitors", flag: true, value: share(l.countries) })}
      ${this.card("Devices", l.devices, { dim: "device", unit: "visitors", value: share(l.devices) })}
      ${this.card("Browsers", l.browsers, { dim: "browser", unit: "visitors", value: share(l.browsers) })}
      ${this.card("Operating systems", l.os, { dim: "os", unit: "visitors", value: share(l.os) })}
    </div>
    <p class="t-foot">${t.geo_source
      ? html`Countries: <a href="https://db-ip.com" target="_blank" rel="noopener">IP Geolocation by DB-IP</a> (${t.geo_source}, CC BY 4.0), looked up on this server. No addresses are kept.`
      : "No country table is loaded (built without the geo feature)."}</p>`;
  }

  renderBehaviour() {
    const t = this.data;
    const any = ["time_on_page", "scroll", "screens", "events", "goals"].some((k) => (t[k] ?? []).length);
    if (!any) {
      return html`<lit-material-empty-state headline=${t.script ? "No reports yet" : "Turn on the page script"}>
        ${t.script
          ? html`Add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages; time on page, scroll depth, screens, events and goals appear here.`
          : html`Set <code>[traffic] script = true</code> and add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages. It reports to this server only, with no cookies.`}
      </lit-material-empty-state>`;
    }
    const sessions = t.sessions?.sessions ?? 0;
    const scroll = ["25", "50", "75", "100"].map((b) => [`up to ${b}%`, (t.scroll.find(([k]) => k === b) ?? [b, 0])[1]]);
    const props = (t.event_props ?? []).map(([k, n]) => [k.replace("\t", " · "), n]);
    return html`<div class="t-cards">
      ${this.card("Time on page", (t.time_on_page ?? []).map(([k, avg, n]) => [k, avg, n]), { dim: "page", unit: "seconds", value: (r) => `${secs(r[1])} · ${num(r[2])}×`, hint: "Average, views reported" })}
      ${this.card("Goals", t.goals ?? [], { unit: "conversions", value: (r) => (sessions ? `${num(r[1])} · ${((r[1] / sessions) * 100).toFixed(1)}%` : num(r[1])), hint: "Sessions that reached it" })}
      ${this.card("Events", t.events ?? [], { unit: "events", hint: "rws.track()" })}
      ${props.length ? this.card("Event properties", props, { unit: "events" }) : nothing}
      ${this.card("Scroll depth", scroll, { unit: "views" })}
      ${this.card("Screen sizes", (t.screens ?? []).map(([k, n]) => [SCREENS[k] ?? k, n]), { unit: "views" })}
    </div>`;
  }

  /** A group's own numbers (AI agents or bots): visitors, requests, pages. */
  groupKpis(g) {
    const t = this.data;
    const now = t.totals[g];
    const before = t.previous?.[g];
    const has = before && (before.requests > 0 || before.visitors > 0);
    const tiles = [
      ["requests", "Requests", now.requests, has ? before.requests : null, "Everything they fetched"],
      ["visitors", g === "ai" ? "Agents" : g === "apps" ? "Users" : "Clients", now.visitors, has ? before.visitors : null, "Distinct clients, counted once a day"],
      ["fetches", "Pages", now.pages, has ? before.pages : null, "HTML pages they fetched"],
    ];
    return html`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${tiles.map(([id, label, value, prev, hint]) => {
        const delta = change(value, prev);
        return html`<button class="t-kpi" aria-pressed=${id === "requests"} ?disabled=${id !== "requests"} title=${hint}>
          <span class="t-kpi-label">${label}</span><span class="t-kpi-value">${num(value)}</span>
          <span class="t-kpi-delta" data-tone=${delta ? "flat" : "none"}>${this.compare && delta ? delta.text : " "}</span>
        </button>`;
      })}
    </div>`;
  }

  /** Requests per bucket for the groups given, and the period before for one. */
  groupChart(groups) {
    const t = this.data;
    const hourly = t.bucket === "hour";
    const series = groups.map((g) => ({ name: GROUP_NAMES[g], points: t.series.map((p) => ({ x: p.t * 1000, y: p[g] })) }));
    if (groups.length === 1 && this.compare) {
      const before = (t.previous_series ?? []).map((p) => ({ x: p.t * 1000, y: p[groups[0]] }));
      if (before.some((p) => p.y > 0)) series.push({ name: "Period before", points: before });
    }
    return html`<section class="t-chart${groups.length > 1 ? " t-chart-multi" : ""}" aria-label="Chart">
      <lit-material-line-chart label=${`Requests per ${hourly ? "hour" : "day"}`} unit="requests" height="240" .series=${series}
        .formatX=${(x) => (hourly ? new Date(x).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : new Date(x).toLocaleDateString([], { month: "short", day: "numeric" }))}></lit-material-line-chart>
    </section>`;
  }

  renderApps() {
    const t = this.data;
    const a = t.apps ?? { users: [], versions: [], events: [] };
    const named = (rows, map = (x) => x) => rows.map(([k, n]) => {
      const [app, rest] = parts(k);
      return [`${app} · ${map(rest)}`, n];
    });
    if (!t.totals.apps?.requests && !a.users.length) {
      return html`${this.groupKpis("apps")}
        <lit-material-empty-state headline="No app traffic yet">
          Native apps and browser extensions that call this server are counted here, not as bots. Name yours with
          <code>[traffic] apps = ["MyApp=MyApp/", "MyApp=chrome-extension://&lt;id&gt;"]</code>, and have them send events to
          <code>POST /_rws/e</code> with <code>{"app":"MyApp","v":"1.2","p":"ios","n":"opened"}</code>.
        </lit-material-empty-state>
        <div class="t-cards">${this.renderDownloads()}</div>`;
    }
    return html`
      ${this.groupKpis("apps")}
      ${this.groupChart(["apps"])}
      <div class="t-cards">
        ${this.card("Users by app and platform", named(a.users, (p) => PLATFORMS[p] ?? p), { unit: "users", hint: "Clients counted once a day" })}
        ${this.card("Versions", named(a.versions), { unit: "users" })}
        ${this.card("App events", named(a.events), { unit: "events", hint: "POST /_rws/e" })}
        ${this.renderDownloads()}
      </div>
      <p class="t-foot">Apps are recognised by <code>[traffic] apps</code> (a User-Agent token or an extension origin), or by platform: Apple's and Android's networking stacks, Electron, extension origins. Unnamed ones are grouped by platform.</p>`;
  }

  renderRobots(g) {
    const t = this.data;
    const byKind = (rows) => {
      const sum = new Map();
      for (const [k, n] of rows) sum.set(k, (sum.get(k) ?? 0) + n);
      return [...sum.entries()].sort((a, b) => b[1] - a[1]);
    };
    if (g === "ai") {
      const agents = t.ai_agents.map((a) => [`${a.name}${a.operator ? ` (${a.operator})` : ""}`, a.requests, a]);
      const kinds = byKind(t.ai_agents.map((a) => [{ "ai-training": "Training crawlers", "ai-search": "AI search", "ai-assistant": "Assistants (for a person)", "ai-agent": "Agents" }[a.kind] ?? a.kind, a.requests]));
      return html`
        ${this.groupKpis("ai")}
        ${this.groupChart(["ai"])}
        <div class="t-cards">
          ${this.card("AI agents", agents, { unit: "requests", value: (r) => html`${num(r[1])}${r[2].verified ? html` <lit-material-status status="ok">Verified</lit-material-status>` : nothing}`, hint: "Verified: from the operator's published network" })}
          ${this.card("Pages they read", t.ai_pages, { unit: "requests", pages: true })}
          ${this.card("What for", kinds, { unit: "requests" })}
        </div>`;
    }
    const label = (b) => {
      const kind = b.kind.replaceAll("-", " ");
      return b.name && b.name !== kind.replace("browser no assets", "browser, no assets") ? `${kind} · ${b.name}` : kind;
    };
    const kinds = byKind(t.bots.map((b) => [b.kind.replaceAll("-", " "), b.requests]));
    return html`
      ${this.groupKpis("bots")}
      ${this.groupChart(["bots"])}
      <div class="t-cards">
        ${this.card("By kind", kinds, { unit: "requests", hint: "Search, SEO, monitors, scanners…" })}
        ${this.card("Bots", t.bots.map((b) => [label(b), b.requests]), { unit: "requests" })}
        ${t.not_found.length ? this.card("Not found", t.not_found, { unit: "requests", hint: "404s, everyone (scanners mostly)", pages: true }) : nothing}
        ${(t.crawl ?? []).length
          ? this.card("Crawlers", t.crawl.map((c) => [c.name, c.pages + c.not_found, c]), { unit: "requests", value: (r) => `${num(r[2].pages)} pages${r[2].not_found ? ` · ${num(r[2].not_found)} 404` : ""}`, hint: "Named search, AI and other crawlers" })
          : nothing}
        ${(t.crawl_404 ?? []).length ? this.card("404s crawlers hit", t.crawl_404.map(([k, n]) => {
            const [page, name] = parts(k);
            return [`${this.pageName(page)} · ${name}`, n];
          }), { unit: "requests", hint: "Fix or redirect these" }) : nothing}
        ${(t.probes?.paths ?? []).length ? this.card("Probes", t.probes.paths, { unit: "requests", hint: "Scanners looking for secrets and admin pages" }) : nothing}
        ${(t.probes?.sources ?? []).length ? this.card("Probe sources", t.probes.sources.map(([k, n]) => {
            const [country, network, address] = parts(k);
            return [[countryName(country), network === "-" ? "" : network, address ?? ""].filter(Boolean).join(" · "), n];
          }), { unit: "requests", hint: "Country and network" }) : nothing}
        ${this.renderDownloads(["bots"])}
      </div>
      <p class="t-foot">People are browsers that load a page's styles or scripts; a browser that never does is counted here as "browser, no assets". Spoofed: claims to be a known crawler from outside its network.</p>`;
  }

  renderAll() {
    const t = this.data;
    const tiles = ["people", "apps", "ai", "bots"].map((g) => {
      const now = t.totals[g];
      const before = t.previous?.[g];
      const delta = change(now.requests, before && before.requests ? before.requests : null);
      return html`<button class="t-kpi" @click=${() => this.setGroup(g)} title=${`Open ${GROUP_NAMES[g]}`}>
        <span class="t-kpi-label">${GROUP_NAMES[g]}</span><span class="t-kpi-value">${num(now.visitors)}</span>
        <span class="t-kpi-delta">${num(now.requests)} requests${this.compare && delta ? ` · ${delta.text}` : ""}</span>
      </button>`;
    });
    const total = ["people", "apps", "ai", "bots"].reduce((s, g) => s + (t.totals[g]?.requests ?? 0), 0) || 1;
    const share = ["people", "apps", "ai", "bots"].map((g) => [GROUP_NAMES[g], t.totals[g]?.requests ?? 0]);
    return html`
      <div class="t-kpis" role="group" aria-label="Groups">${tiles}</div>
      ${this.groupChart(["people", "apps", "ai", "bots"])}
      <div class="t-cards">
        ${this.card("Share of requests", share, { unit: "requests", value: (r) => `${num(r[1])} · ${pct(r[1] / total)}` })}
        ${this.card("Top pages (humans)", t.people_pages, { dim: "page", unit: "views", limit: 6 })}
        ${this.card("AI agents", t.ai_agents.map((a) => [a.name, a.requests]), { unit: "requests", limit: 6 })}
      </div>`;
  }

  /** People by country on a world map; picking a country filters like its row, picking it again clears. */
  renderMap(rows) {
    const data = Object.fromEntries(rows.filter(([k]) => k.length === 2));
    return html`<lit-material-world-map
      class="t-map"
      label="Visitors by country"
      unit="visitors"
      selectable
      .data=${data}
      .selected=${live(this.filters.country ?? "")}
      .format=${num}
      @change=${(e) => (e.detail.code ? this.pick("country", e.detail.code) : this.unpick("country"))}
    ></lit-material-world-map>`;
  }

  render() {
    if (this.off) {
      return html`<h1>Traffic</h1><lit-material-empty-state headline="Traffic counting is off">It's on whenever the admin API is, unless <code>[traffic] enabled = false</code>.</lit-material-empty-state>`;
    }
    const t = this.data;
    const body = !t
      ? html`<div class="t-loading">${[0, 1, 2].map(() => html`<lit-material-skeleton variant="rect" style="height: 96px"></lit-material-skeleton>`)}</div>`
      : this.query === null && this.filtered && !this.notice
        ? html`<p class="t-none">Loading…</p>`
        : this.group !== "people"
        ? this.group === "all" ? this.renderAll() : this.group === "apps" ? this.renderApps() : this.renderRobots(this.group)
        : ({
            overview: () => this.renderOverview(),
            visitors: () => this.renderVisitors(),
            pages: () => this.renderPages(),
            sources: () => this.renderSources(),
            audience: () => this.renderAudience(),
            behaviour: () => this.renderBehaviour(),
          }[this.tab] ?? (() => this.renderOverview()))();
    return html`
      <div class="t-head">
        <h1>Traffic</h1>
        ${this.renderBar()}
        ${this.group === "people"
          ? html`<div class="t-tabs" role="tablist" aria-label="Traffic sections">
              ${TABS.map(([id, label]) => html`<button role="tab" aria-selected=${this.tab === id} @click=${() => this.setTab(id)}>${label}</button>`)}
            </div>`
          : html`<div class="t-tabs t-tabs-empty"></div>`}
      </div>
      <div role="tabpanel" aria-label=${this.group === "people" ? (TABS.find(([id]) => id === this.tab) ?? TABS[0])[1] : GROUP_NAMES[this.group] ?? "All"}>${body}</div>`;
  }
}

customElements.define("rws-traffic", RwsTraffic);
