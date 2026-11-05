// The Traffic view: a sticky filter bar (period, site, filters), a row of
// key numbers that choose the chart's metric, the chart against the period
// before, and cards of top lists whose "All" opens a tab with the full
// list. Clicking a row filters the page by it: the site always, anything
// else with the traffic database ([traffic] database_url_env), whose rows
// can answer any combination.
import { LitElement, html, nothing } from "lit";
import { api, ApiError } from "./api.js";
import { formatDuration } from "@lit-material/format";

const TABS = [
  ["overview", "Overview"],
  ["pages", "Pages"],
  ["sources", "Sources"],
  ["audience", "Audience"],
  ["behaviour", "Behaviour"],
  ["robots", "AI & bots"],
];

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

const CHANNELS = { direct: "Direct", search: "Search", social: "Social", ai: "AI assistants", referral: "Other sites", campaign: "Campaigns", "(none)": "Direct" };
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
  };

  constructor() {
    super();
    this.tab = "overview";
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
    try {
      const saved = JSON.parse(localStorage.getItem("rws-traffic") ?? "{}");
      if (TABS.some(([id]) => id === saved.tab)) this.tab = saved.tab;
      if (typeof saved.compare === "boolean") this.compare = saved.compare;
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
      localStorage.setItem("rws-traffic", JSON.stringify({ tab: this.tab, compare: this.compare }));
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
      ${hosts.length > 1
        ? html`<label class="t-select"><span class="t-sr">Site</span>
            <select @change=${(e) => this.setHost(e.target.value)}>
              <option value="" ?selected=${!this.host}>All sites</option>
              ${hosts.map((h) => html`<option value=${h} ?selected=${this.host === h}>${h}</option>`)}
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
      ${this.renderChart()}
      <div class="t-cards">
        ${this.card("Top pages", l.pages, { dim: "page", unit: "views" }, "pages")}
        ${this.card("Sources", l.channels, { dim: "channel", unit: "visits" }, "sources")}
        ${this.card("Countries", l.countries, { dim: "country", unit: "visitors", flag: true }, "audience")}
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

  renderPages() {
    const l = this.lists();
    return html`<div class="t-cards">
      ${this.card("Top pages", l.pages, { dim: "page", unit: "views" })}
      ${l.entry.length ? this.card("Entry pages", l.entry, { dim: "page", unit: "sessions", hint: "Where sessions began" }) : nothing}
      ${l.exit.length ? this.card("Exit pages", l.exit, { dim: "page", unit: "sessions", hint: "Where sessions ended" }) : nothing}
      ${l.not_found.length ? this.card("Not found", l.not_found, { unit: "requests", hint: "Paths answered 404", pages: true }) : nothing}
    </div>`;
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
    return html`<div class="t-cards">
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

  renderRobots() {
    const t = this.data;
    const g = (k) => t.totals[k];
    const agents = t.ai_agents.map((a) => [`${a.name}${a.operator ? ` (${a.operator})` : ""}`, a.requests, a]);
    const bots = t.bots.map((b) => {
      const kind = b.kind.replaceAll("-", " ");
      return [b.name && b.name !== kind.replace("browser no assets", "browser, no assets") ? `${kind} · ${b.name}` : kind, b.requests];
    });
    return html`
      <div class="t-kpis">
        ${[["ai", "AI agents"], ["bots", "Bots"]].map(([k, label]) => html`<div class="t-kpi" aria-disabled="true">
          <span class="t-kpi-label">${label}</span><span class="t-kpi-value">${num(g(k).visitors)}</span>
          <span class="t-kpi-delta">${num(g(k).requests)} requests</span></div>`)}
      </div>
      <p class="t-foot">People are browsers that load a page's styles or scripts. AI agents and bots are known by name, published network or behaviour; "Verified" means the request came from the operator's published network.</p>
      <div class="t-cards">
        ${this.card("AI agents", agents, { unit: "requests", value: (r) => html`${num(r[1])}${r[2].verified ? html` <lit-material-status status="ok">Verified</lit-material-status>` : nothing}` })}
        ${this.card("Pages AI agents read", t.ai_pages, { unit: "requests", pages: true })}
        ${this.card("Bots", bots, { unit: "requests" })}
      </div>`;
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
        : {
            overview: () => this.renderOverview(),
            pages: () => this.renderPages(),
            sources: () => this.renderSources(),
            audience: () => this.renderAudience(),
            behaviour: () => this.renderBehaviour(),
            robots: () => this.renderRobots(),
          }[this.tab]();
    return html`
      <div class="t-head">
        <h1>Traffic</h1>
        ${this.renderBar()}
        <div class="t-tabs" role="tablist" aria-label="Traffic sections">
          ${TABS.map(([id, label]) => html`<button role="tab" aria-selected=${this.tab === id} @click=${() => this.setTab(id)}>${label}</button>`)}
        </div>
      </div>
      <div role="tabpanel" aria-label=${TABS.find(([id]) => id === this.tab)[1]}>${body}</div>`;
  }
}

customElements.define("rws-traffic", RwsTraffic);
