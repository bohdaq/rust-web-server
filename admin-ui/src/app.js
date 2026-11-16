import { LitElement, html, svg, nothing } from "lit";
import { live } from "lit/directives/live.js";
import { api, ApiError, followLogs, setToken, token } from "./api.js";
import { formatBytes as bytes, formatDuration } from "@lit-material/format";
import "@lit-material/truncate";
import { icon } from "./icons.js";
import { between, latency, quantile, under } from "./breakdown.js";
import { counts, diffLines, hunks } from "./diff.js";

// Sections, grouped as the sidebar shows them. `key`: press G, then the key.
const VIEWS = [
  { id: "overview", label: "Overview", icon: "gauge", key: "o", group: "Observe" },
  { id: "traffic", label: "Traffic", icon: "chart-line", key: "t", group: "Observe" },
  { id: "logs", label: "Logs", icon: "terminal-window", key: "l", group: "Observe" },
  { id: "upstreams", label: "Upstreams", icon: "tree-structure", key: "u", group: "Route" },
  { id: "certificates", label: "Certificates", icon: "certificate", key: "c", group: "Route" },
  { id: "controls", label: "Controls", icon: "sliders-horizontal", key: "x", group: "Operate" },
  { id: "config", label: "Config", icon: "list-bullets", key: "f", group: "Operate" },
  { id: "audit", label: "Audit", icon: "clipboard-text", key: "a", group: "Operate" },
];
const GROUPS = ["Observe", "Route", "Operate"];

// The phone's bottom bar: these four, then "More" for the rest.
const PRIMARY = ["overview", "traffic", "upstreams", "logs"];

// Requests-per-second samples kept for the Overview chart (one every 2 s).
const SAMPLES = 150;
// Access-log lines kept in the Logs page, and how many of them are drawn.
// Response times and "where requests go" cover the last five minutes: an
// earlier /breakdown answer is kept every ten seconds to subtract from the newest.
const WINDOW_MS = 5 * 60 * 1000;
const BASE_EVERY_MS = 10 * 1000;
// A window shorter than this says too little: the numbers since start are shown.
const WINDOW_MIN_MS = 30 * 1000;
// Hosts named in "Where requests go"; the rest are added up.
const HOSTS_SHOWN = 5;
const LOG_KEPT = 1000;
const LOG_DRAWN = 300;
// Lines of "What will change" drawn on the Config page.
const DIFF_DRAWN = 400;

const DOCS = [
  ["Admin console guide", "https://rws8.tech/docs/deployment/admin/"],
  ["Traffic analytics guide", "https://rws8.tech/docs/deployment/traffic/"],
  ["Config file reference", "https://rws8.tech/docs/configuration/config-file/"],
];

// Stable across renders, so the palette doesn't see a new value each time.
const PALETTE_HINTS = [{ keys: [], label: "Changes are written to Audit" }];

const MOD = /Mac|iPhone|iPad/.test(navigator.platform ?? "") ? "⌘" : "Ctrl";

/** An IPv4 or IPv6 address (the block form checks before asking the server). */
const looksLikeIp = (v) => /^(\d{1,3}\.){3}\d{1,3}$/.test(v) && v.split(".").every((n) => Number(n) <= 255) || (/^[0-9a-f:.]+$/i.test(v) && v.includes(":"));

/** An API error message for people: no raw JSON shapes. */
const plain = (message) => (/expected \{/.test(message ?? "") ? "The server didn't accept that value." : message);

function viewFromHash() {
  const id = location.hash.replace(/^#\/?/, "");
  return VIEWS.some((v) => v.id === id) ? id : "overview";
}

// "3d 4h", "2h 5m", "4m 12s".
const duration = (secs) => formatDuration(secs * 1000, { unitDisplay: "narrow" });

// The rws app icon (site/dist/brand/icon-orange.svg), inline: the admin
// page's CSP allows no images from elsewhere.
const brandMark = html`<svg class="brand-mark" viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="7" fill="#D2551E"/><g transform="translate(4 3.5)"><rect x="2" y="5" width="20" height="4" rx="1.5" fill="#16120F"/><rect x="2" y="10.5" width="20" height="4" rx="1.5" fill="#16120F"/><path d="M3.5 16H15L14 17L15 18L14 19L15 20H3.5A1.5 1.5 0 0 1 2 18.5V17.5A1.5 1.5 0 0 1 3.5 16Z" fill="#F2EBE3"/><rect x="16.5" y="16.5" width="2" height="2" fill="#F2EBE3"/><rect x="19.5" y="15" width="1.4" height="1.4" fill="#F2EBE3"/></g></svg>`;

const status = (kind, label) => html`<lit-material-status status=${kind}>${label}</lit-material-status>`;
const time = (ms) => new Date(ms).toLocaleTimeString([], { hour12: false });
const plural = (n, one, many) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

/** A rate for people: one decimal under 10, whole numbers above. */
const rate = (v) => (v < 10 ? v.toFixed(1) : Math.round(v).toLocaleString());

/** "just now", "40 s ago", "3 min ago", "2 h ago", then the date. */
function ago(ms, now = Date.now()) {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 2) return "just now";
  if (s < 60) return `${s} s ago`;
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return new Date(ms).toLocaleDateString();
}

/** "15:02" today, "Tue 15:02" on another day. */
function hourMinute(ms) {
  const at = new Date(ms);
  const clock = at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  return at.toDateString() === new Date().toDateString() ? clock : `${at.toLocaleDateString([], { weekday: "short" })} ${clock}`;
}

/** "in 40 s", "in 12 min", "in 2 h 5 min". */
function left(ms, now = Date.now()) {
  const s = Math.max(0, Math.round((ms - now) / 1000));
  if (s < 60) return `in ${s} s`;
  const minutes = Math.round(s / 60);
  if (minutes < 60) return `in ${minutes} min`;
  const h = Math.floor(minutes / 60), m = minutes % 60;
  return m ? `in ${h} h ${m} min` : `in ${h} h`;
}

/** Seconds for a choice: "5 minutes", "1 hour", "90 seconds". */
function span(secs) {
  if (secs % 3600 === 0) return plural(secs / 3600, "hour", "hours");
  if (secs % 60 === 0) return plural(secs / 60, "minute", "minutes");
  return plural(secs, "second", "seconds");
}

// What the maintenance dialog offers (seconds).
const RETRY_AFTER = [300, 900, 3600];
const TURN_OFF_AFTER = [3600, 14400];

/** The next round number at or above `v`, for the chart's top line. */
function niceMax(v) {
  if (v <= 1) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  const n = v / p;
  return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * p;
}

/** An access-log line: `j` is the parsed JSON line, or null for other formats. */
function parseLog(raw) {
  try {
    const j = JSON.parse(raw);
    if (j && typeof j === "object" && j.status !== undefined) return { raw, j };
  } catch {
    // Not JSON (the combined format): shown as written.
  }
  return { raw, j: null };
}

/** The status class of a log line: "2", "4", "5"… or "" when unknown. */
const statusClass = (r) => (r.j ? String(r.j.status)[0] : /"\s([1-5])\d\d\s/.exec(r.raw)?.[1] ?? "");
const statusTone = (code) => (code >= 500 ? "bad" : code >= 400 ? "warn" : code >= 300 ? "info" : "ok");

function clock(when) {
  const at = new Date(when);
  return Number.isNaN(at.getTime()) ? String(when ?? "") : time(at.getTime());
}

/** One request as a row: time, method, URL, status, size, client. */
function requestRow(r) {
  if (!r.j) return html`<div class="req" role="row"><span class="c-raw" role="cell" title=${r.raw}>${r.raw}</span></div>`;
  const j = r.j;
  const path = j.path ?? j.uri ?? "";
  return html`<div class="req" role="row">
    <span class="c-time" role="cell">${clock(j.time)}</span>
    <span class="c-method" role="cell">${j.method ?? ""}</span>
    <span class="c-url" role="cell" title="${j.host ?? ""}${path}">${j.host ? html`<span class="host">${j.host}</span>` : nothing}${path}</span>
    <span class="c-status" role="cell" data-tone=${statusTone(Number(j.status))}>${j.status}</span>
    <span class="c-size" role="cell">${typeof j.bytes === "number" ? bytes(j.bytes) : ""}</span>
    <span class="c-from" role="cell">${j.remote_addr ?? ""}</span>
  </div>`;
}

const requestHead = html`<div class="req req-head" role="row">
  <span role="columnheader">Time</span><span role="columnheader">Method</span><span role="columnheader">URL</span>
  <span role="columnheader">Status</span><span class="c-size" role="columnheader">Size</span><span class="c-from" role="columnheader">From</span>
</div>`;

/** An audit entry in words where the UI knows the change, else "PUT /path". */
function changeText(e) {
  const path = e.path.replace("/admin/api/v1", "");
  let detail = null;
  try {
    detail = JSON.parse(e.detail);
  } catch {
    // No body, or not JSON.
  }
  if (/\/drain$/.test(path) && detail?.backend) return `${detail.drained ? "Drained" : "Put back"} ${detail.backend}`;
  return `${e.method === "TOOL" ? "tool" : e.method} ${path}`;
}

/** "updated 4 s ago", ticking by itself so the app doesn't re-render each second. */
class RwsAgo extends LitElement {
  static properties = { at: { type: Number }, now: { state: true } };

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.tick = setInterval(() => (this.now = Date.now()), 1000);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    clearInterval(this.tick);
  }

  render() {
    return this.at ? html`updated ${ago(this.at, Math.max(this.now ?? 0, Date.now()))}` : nothing;
  }
}
customElements.define("rws-ago", RwsAgo);

/** "in 12 min" until `at`, ticking by itself. */
class RwsLeft extends LitElement {
  static properties = { at: { type: Number }, now: { state: true } };

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.tick = setInterval(() => (this.now = Date.now()), 1000);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    clearInterval(this.tick);
  }

  render() {
    return this.at ? left(this.at) : nothing;
  }
}
customElements.define("rws-left", RwsLeft);

/** The whole app, rendered into the light DOM so the theme's classes apply. */
export class RwsAdmin extends LitElement {
  static properties = {
    authed: { state: true },
    view: { state: true },
    loginError: { state: true },
    data: { state: true },
    error: { state: true },
    samples: { state: true },
    recent: { state: true },
    hover: { state: true },
    paused: { state: true },
    logTick: { state: true },
    logFilter: { state: true },
    filter: { state: true },
    busy: { state: true },
    me: { state: true },
    sso: { state: true },
    moreOpen: { state: true },
    conn: { state: true },
    notice: { state: true },
    ask: { state: true },
    typed: { state: true },
    picked: { state: true },
    query: { state: true },
    fieldError: { state: true },
    file: { state: true },
    draft: { state: true },
    fileCheck: { state: true },
    fileNewer: { state: true },
    fileError: { state: true },
    applied: { state: true },
  };

  constructor() {
    super();
    this.authed = false;
    this.view = viewFromHash();
    this.data = {};
    this.samples = [];
    this.recent = null;
    this.hover = null;
    this.paused = false;
    this.logTick = 0;
    this.logAll = [];
    this.logFrozen = [];
    this.logFilter = { text: "", status: "all" };
    this.filter = "";
    this.busy = "";
    this.lastTotal = null;
    this.bases = [];
    this.me = null;
    this.sso = false;
    this.moreOpen = false;
    // The admin API's reachability: lost = the last poll failed; at = the last answer.
    this.conn = { lost: false, at: 0 };
    this.notice = null;
    this.ask = null;
    this.typed = "";
    this.picked = {};
    this.query = "";
    this.fieldError = {};
    // The configuration file as read (file), the edited text (draft; null = no
    // edits) and what the server says about it. Kept in memory only: it may
    // hold secrets.
    this.file = null;
    this.draft = null;
    this.fileCheck = null;
    this.fileNewer = null;
    this.fileError = "";
    this.applied = null;
  }

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.onHash = () => this.show(viewFromHash(), { push: false });
    addEventListener("hashchange", this.onHash);
    this.onKey = (e) => this.key(e);
    document.addEventListener("keydown", this.onKey);
    // Edits to the configuration file live in this page only: ask before leaving them.
    this.onLeave = (e) => {
      if (this.draft === null) return;
      e.preventDefault();
      e.returnValue = "";
    };
    addEventListener("beforeunload", this.onLeave);
    // A token in this tab, or an OIDC sign-in cookie: try either.
    this.check({ quiet: !token() });
    api("/auth").then((m) => (this.sso = Boolean(m?.oidc))).catch(() => {});
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    removeEventListener("hashchange", this.onHash);
    document.removeEventListener("keydown", this.onKey);
    removeEventListener("beforeunload", this.onLeave);
    this.stop();
    clearInterval(this.pulse);
    clearTimeout(this.checkTimer);
  }

  // ── session ───────────────────────────────────────────────────────────
  async check({ quiet = false } = {}) {
    try {
      const [me, info] = await Promise.all([api("/whoami"), api("/status")]);
      this.me = me;
      this.data = { ...this.data, status: info };
      this.authed = true;
      this.loginError = "";
      this.conn = { lost: false, at: Date.now() };
      this.show(this.view, { push: false });
      this.heartbeat();
    } catch (e) {
      this.authed = false;
      if (quiet && e instanceof ApiError && e.status === 401) {
        this.focusToken();
        return;
      }
      this.loginError =
        e instanceof ApiError && e.status === 401 ? "That token isn't an admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server."
        : e instanceof ApiError && e.status === 403 ? e.message
        : `The admin API didn't answer: ${e.message}`;
      if (e instanceof ApiError && e.status === 401) setToken("");
      this.focusToken();
    }
  }

  focusToken() {
    this.updateComplete.then(() => this.querySelector("#token")?.focus());
  }

  /**
   * Every 5 s on every page: /status (ready, maintenance) and /upstreams
   * (healthy backends), and once a minute /certificates, so the header and
   * "Needs attention" tell the truth wherever you are, and a lost
   * connection shows with the time of the last answer.
   */
  heartbeat() {
    clearInterval(this.pulse);
    let beats = 0;
    const beat = async () => {
      if (!this.authed) return;
      try {
        const [info, ups] = await Promise.all([api("/status"), api("/upstreams").catch(() => this.data.upstreams ?? [])]);
        const before = this.data.status;
        // Its time ran out since the last look (nobody here turned it off).
        if (before?.maintenance && before.maintenance_until && !info.maintenance && before.maintenance_until * 1000 <= Date.now() + 1000) {
          this.toast("Maintenance mode turned itself off: visitors reach your sites again");
        }
        this.data = { ...this.data, status: info, upstreams: ups };
        // Controls shows /maintenance: keep it in step when it changed elsewhere.
        const shown = this.data.maintenance;
        if (shown && (shown.active !== info.maintenance || (shown.until ?? null) !== (info.maintenance_until ?? null))) {
          api("/maintenance").then((maintenance) => (this.data = { ...this.data, maintenance })).catch(() => {});
        }
        this.conn = { lost: false, at: Date.now() };
        if (beats++ % 12 === 0) {
          const certificates = await api("/certificates").catch(() => null);
          if (certificates) this.data = { ...this.data, certificates };
        }
      } catch (e) {
        if (e instanceof ApiError && e.status === 401) {
          this.logout();
          this.loginError = "The server no longer accepts this token. Sign in again.";
          return;
        }
        this.conn = { ...this.conn, lost: true };
      }
    };
    this.pulse = setInterval(beat, 5000);
    beat();
  }

  login(event) {
    event.preventDefault();
    const field = this.querySelector("#token");
    const value = (field?.value ?? "").trim();
    if (!value) {
      this.loginError = "Paste the admin token.";
      return;
    }
    setToken(value);
    this.check();
  }

  logout() {
    const viaSso = this.me?.via === "oidc";
    setToken("");
    this.stop();
    this.authed = false;
    this.data = {};
    this.me = null;
    this.samples = [];
    this.lastTotal = null;
    this.bases = [];
    this.recent = null;
    this.file = null;
    this.draft = null;
    this.fileCheck = null;
    this.fileNewer = null;
    this.fileError = "";
    this.applied = null;
    this.diffOf = null;
    clearTimeout(this.checkTimer);
    clearInterval(this.pulse);
    if (viaSso) location.assign("/admin/auth/logout");
  }

  /** An API call from a view: errors land in the page, 401 signs out. */
  async call(path, options) {
    try {
      const result = await api(path, options);
      this.error = "";
      this.conn = { lost: false, at: Date.now() };
      return result;
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) {
        this.logout();
        this.loginError = "The server no longer accepts this token. Sign in again.";
      } else if (!(e instanceof ApiError)) {
        // fetch itself failed: the header shows the lost connection.
        this.conn = { ...this.conn, lost: true };
      } else {
        this.error = plain(e.message);
      }
      throw e;
    }
  }

  /** A message at the bottom of the screen; `undo` adds an Undo button. */
  willUpdate(changed) {
    // A drained backend finished its last request since the last look.
    const before = changed.get("data")?.upstreams;
    const now = this.data?.upstreams;
    if (!before || !now || before === now) return;
    for (const u of now) {
      for (const b of u.backends) {
        const was = before.find((x) => x.name === u.name)?.backends.find((x) => x.url === b.url);
        if (was?.drained && b.drained && was.open_requests > 0 && b.open_requests === 0) this.toast(`${b.url} finished its requests: safe to deploy`);
      }
    }
  }

  toast(message, undo = null) {
    clearTimeout(this.noticeTimer);
    this.notice = { message, undo };
    this.noticeTimer = setTimeout(() => (this.notice = null), undo ? 8000 : 4000);
  }

  /**
   * A confirmation dialog; resolves true when confirmed. `effects` lists
   * what will happen ([tone, text] pairs, or a function of the picks);
   * `type` is a word the person has to type before the action button works.
   */
  confirm({ title, body, action, tone = "danger", effects = null, type = "", choices = [] }) {
    return new Promise((resolve) => {
      this.typed = "";
      // `choices`: [{ id, label, options: [[value, label]], value }]; the picks are in this.picked.
      this.picked = Object.fromEntries(choices.map((c) => [c.id, c.value]));
      this.ask = { title, body, action, tone, effects, type, choices, resolve };
      this.updateComplete.then(() => {
        this.querySelector("#confirm")?.showModal();
        (this.querySelector("#confirm-type") ?? this.querySelector("#confirm .cancel"))?.focus();
      });
    });
  }

  confirmReady() {
    return !this.ask?.type || this.typed.trim().toLowerCase() === this.ask.type;
  }

  answer(yes) {
    const ask = this.ask;
    this.querySelector("#confirm")?.close();
    this.ask = null;
    ask?.resolve(yes);
  }

  // ── keyboard and the command palette ──────────────────────────────────
  /** Ctrl/⌘ K or "/" opens the palette; G then a letter goes to a section. */
  key(e) {
    if (!this.authed) return;
    const palette = this.querySelector("lit-material-command-palette");
    const k = (e.key ?? "").toLowerCase();
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && k === "k") {
      e.preventDefault();
      if (palette?.open) palette.hide();
      else this.openPalette();
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (palette?.open || this.ask || this.moreOpen && k !== "escape") return;
    if (e.composedPath().some((n) => n instanceof Element && n.matches("input, textarea, select, [contenteditable]"))) return;
    if (k === "escape" && this.moreOpen) {
      this.moreOpen = false;
      return;
    }
    if (this.chord && Date.now() - this.chord < 1000) {
      this.chord = 0;
      const view = VIEWS.find((v) => v.key === k);
      if (view) {
        e.preventDefault();
        this.show(view.id);
      }
      return;
    }
    if (k === "g") this.chord = Date.now();
    else if (k === "/") {
      e.preventDefault();
      this.openPalette();
    }
  }

  openPalette() {
    if (this.ask) return;
    this.query = "";
    this.moreOpen = false;
    // What the palette's actions are built from, when no page loaded it yet.
    for (const [key, path] of [["caches", "/caches"], ["config", "/config"], ["certificates", "/certificates"], ["blocklist", "/blocklist"]]) {
      if (!this.data[key]) api(path).then((value) => (this.data = { ...this.data, [key]: value })).catch(() => {});
    }
    this.querySelector("lit-material-command-palette")?.show();
  }

  /** Everything the palette can do right now: actions, sections, settings, docs. */
  paletteItems() {
    const d = this.data;
    const items = [];
    const add = (section, label, run, more = {}) => items.push({ id: `${section}:${label}:${more.hint ?? ""}`, section, label, run, ...more });
    const asks = { hint: "asks you to confirm" };
    const query = this.query.trim();

    if (looksLikeIp(query)) {
      if ((d.blocklist ?? []).includes(query)) add("Actions", `Unblock ${query}`, () => this.unblock(query));
      else add("Actions", `Block ${query}`, () => this.blockIp(query), { hint: "refuses every request from it" });
    }
    add("Actions", "Reload configuration", () => this.reload(), { hint: "re-reads rws.config.toml", keywords: ["config", "restart", "apply"] });
    if (d.status?.maintenance) add("Actions", "Turn off maintenance mode", () => this.setMaintenance(false), { keywords: ["503"] });
    if (d.status?.maintenance && d.status.maintenance_until) add("Actions", "Keep maintenance on 1 hour longer", () => this.extendMaintenance(), { keywords: ["503", "extend", "add"] });
    else add("Actions", "Turn on maintenance mode", () => this.setMaintenance(true), { ...asks, keywords: ["503", "down"] });
    if (d.certificates?.acme?.running) add("Actions", "Renew certificates now", () => this.renew(), { keywords: ["acme", "tls", "https"] });
    for (const u of d.upstreams ?? []) {
      for (const b of u.backends) {
        if (b.drained) add("Actions", `Put back ${b.url}`, () => this.drain(u.name, b.url, false), { hint: u.name, keywords: ["undrain", "rotation", u.name] });
        else add("Actions", `Drain ${b.url}`, () => this.drain(u.name, b.url, true), { hint: u.name, keywords: ["rotation", "deploy", u.name] });
      }
    }
    for (const c of d.caches ?? []) {
      if (c.entries) add("Actions", `Purge cache ${c.name}`, () => this.purgeAll(c.name, c.entries), { ...asks, keywords: ["clear", "empty"] });
    }
    add("Actions", "Sign out", () => this.logout(), { keywords: ["log out", "logout"] });

    for (const v of VIEWS) add("Go to", v.label, () => this.show(v.id), { shortcut: ["G", v.key.toUpperCase()] });

    if (query.length >= 2 && d.config) {
      const needle = query.toLowerCase();
      const hits = Object.entries(d.config).filter(([k, v]) => k.toLowerCase().includes(needle) || v.toLowerCase().includes(needle)).slice(0, 6);
      for (const [setting, value] of hits) {
        add("Settings", setting, () => { this.filter = setting; this.show("config"); },
          { hint: value === "" ? "empty" : value.length > 36 ? `${value.slice(0, 35)}…` : value, keywords: [value] });
      }
    }

    for (const [label, url] of DOCS) add("Docs", label, () => window.open(url, "_blank", "noopener"), { hint: "opens in a new tab", keywords: ["help", "documentation"] });
    return items;
  }

  // ── views and their polling ───────────────────────────────────────────
  show(view, { push = true } = {}) {
    this.stop();
    this.view = view;
    this.moreOpen = false;
    this.hover = null;
    // A new history entry per section, so Back returns to the last one.
    if (location.hash !== `#/${view}`) {
      if (push && location.hash) history.pushState(null, "", `#/${view}`);
      else history.replaceState(null, "", `#/${view}`);
    }
    document.title = `${VIEWS.find((v) => v.id === view)?.label ?? "Overview"} · rws admin`;
    this.error = "";
    this.fieldError = {};
    const every = { overview: 2000, upstreams: 3000, certificates: 30000, controls: 5000, audit: 10000 }[view];
    this.load(view);
    if (every) this.timer = setInterval(() => this.load(view), every);
    if (view === "logs") this.follow();
    queueMicrotask(() => this.querySelector("main")?.focus({ preventScroll: true }));
  }

  stop() {
    clearInterval(this.timer);
    this.timer = undefined;
    this.abort?.abort();
    this.abort = undefined;
    clearTimeout(this.logTimer);
    this.logTimer = undefined;
  }

  async load(view) {
    const get = async (key, path) => {
      try {
        const value = await this.call(path);
        this.data = { ...this.data, [key]: value };
      } catch {
        // Shown through this.error.
      }
    };
    // Extras a page is fine without (a role may not read them).
    const quiet = (path) => api(path).catch(() => null);
    if (view === "overview") {
      const [, , lines, breakdown] = await Promise.all([get("status", "/status"), get("metrics", "/metrics"), quiet("/logs?n=8"), quiet("/breakdown")]);
      if (Array.isArray(lines)) this.recent = lines.map(parseLog).reverse();
      if (breakdown?.duration) {
        const at = Date.now();
        // Fewer requests than before: the server restarted, earlier answers don't apply.
        if (this.bases.length && breakdown.duration.count < this.bases.at(-1).b.duration.count) this.bases = [];
        if (!this.bases.length || at - this.bases.at(-1).at >= BASE_EVERY_MS) {
          this.bases = [...this.bases, { at, b: breakdown }].filter((e) => at - e.at <= WINDOW_MS);
        }
        this.data = { ...this.data, breakdown: { ...breakdown, at } };
      }
      const m = this.data.metrics;
      if (typeof m?.requests_total === "number") {
        const now = Date.now();
        const secs = this.lastTotal ? (now - this.lastTotal.at) / 1000 : 0;
        if (secs > 0.5) {
          const perSecond = Math.max(0, (m.requests_total - this.lastTotal.total) / secs);
          const errors = Math.max(0, (m.errors_total ?? 0) - this.lastTotal.errors);
          this.samples = [...this.samples, { x: now, y: Math.round(perSecond * 100) / 100, e: errors }].slice(-SAMPLES);
        }
        if (!this.lastTotal || secs > 0.5) this.lastTotal = { total: m.requests_total, errors: m.errors_total ?? 0, at: now };
      }
    } else if (view === "upstreams") {
      const [, audit] = await Promise.all([get("upstreams", "/upstreams"), quiet("/audit?n=200")]);
      if (Array.isArray(audit)) this.data = { ...this.data, audit };
    } else if (view === "certificates") {
      await get("certificates", "/certificates");
    } else if (view === "controls") {
      await Promise.all([get("maintenance", "/maintenance"), get("blocklist", "/blocklist"), get("flags", "/flags"), get("rateLimit", "/rate-limit"), get("caches", "/caches"), this.loadAuthz()]);
    } else if (view === "config") {
      await Promise.all([get("config", "/config"), this.loadFile()]);
    } else if (view === "audit") {
      await get("audit", "/audit?n=200");
    }
  }

  /** The Logs page: the last 200 lines, then every new one as it's written. */
  async follow() {
    const abort = (this.abort = new AbortController());
    this.logAll = [];
    this.logFrozen = [];
    this.paused = false;
    const keep = (lines) => {
      if (abort.signal.aborted) return;
      for (const raw of Array.isArray(lines) ? lines : [lines]) this.logAll.push(parseLog(raw));
      if (this.logAll.length > LOG_KEPT) this.logAll.splice(0, this.logAll.length - LOG_KEPT);
      // Draw at most four times a second, however fast lines arrive.
      if (!this.paused && !this.logTimer) {
        this.logTimer = setTimeout(() => {
          this.logTimer = undefined;
          this.logTick++;
        }, 250);
      }
    };
    try {
      keep(await this.call("/logs?n=200"));
      this.logTick++;
      await followLogs(keep, abort.signal);
    } catch (e) {
      if (e.name !== "AbortError" && !abort.signal.aborted && !this.error) this.error = `The live log stopped: ${e.message}`;
    }
  }

  /** Whether a log line passes the filter box and status chips. */
  logShown(r) {
    const f = this.logFilter;
    if (f.status !== "all" && statusClass(r) !== f.status[0]) return false;
    return !f.text || r.raw.toLowerCase().includes(f.text.toLowerCase());
  }

  setLogFilter(change) {
    this.logFilter = { ...this.logFilter, ...change };
  }

  setPaused(paused) {
    if (paused) this.logFrozen = [...this.logAll];
    this.paused = paused;
  }

  /** Saves the kept lines (those passing the filter) as a text file. */
  downloadLog() {
    const lines = this.logAll.filter((r) => this.logShown(r)).map((r) => r.raw);
    const url = URL.createObjectURL(new Blob([`${lines.join("\n")}\n`], { type: "text/plain" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `rws-access-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.log`;
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    this.toast(`${plural(lines.length, "line", "lines")} saved`);
  }

  // ── the configuration file: edit, see what changes, apply ─────────────
  /** Admins only (the server refuses the rest): the file as it is on disk. */
  async loadFile() {
    if (this.me?.role !== "admin") return;
    const fresh = await api("/config/file").catch(() => null);
    if (!fresh) return;
    if (this.draft !== null && this.file?.editable && fresh.sha256 !== this.file.sha256) {
      // Somebody else wrote it while there are edits here: say so, keep both.
      this.fileNewer = fresh;
      return;
    }
    this.file = fresh;
    this.fileNewer = null;
  }

  /** What applying the edits would change: `{ lines, added, removed }`. */
  get fileDiff() {
    const before = this.file?.content ?? "";
    const after = this.draft ?? before;
    if (this.diffOf?.before !== before || this.diffOf?.after !== after) {
      const lines = diffLines(before, after);
      this.diffOf = { before, after, lines, ...counts(lines) };
    }
    return this.diffOf;
  }

  editFile(text) {
    this.draft = text === this.file?.content ? null : text;
    this.fileError = "";
    this.applied = null;
    clearTimeout(this.checkTimer);
    if (this.draft === null) {
      this.fileCheck = null;
      return;
    }
    this.fileCheck = { state: "checking", of: text };
    this.checkTimer = setTimeout(() => this.checkFile(text), 500);
  }

  /** Ask the server what `rws --check` says about `text`, and what needs a restart. */
  async checkFile(text) {
    try {
      const result = await api("/config/file/check", { method: "POST", body: { content: text } });
      if (this.draft === text) this.fileCheck = { state: "done", of: text, ...result };
    } catch (e) {
      if (this.draft === text) this.fileCheck = { state: "failed", of: text, message: e.message };
    }
  }

  discardFile() {
    const draft = this.draft;
    if (draft === null) return;
    this.editFile(this.file.content);
    this.toast("Edits discarded", () => this.editFile(draft));
  }

  /** The file changed on disk under the edits: compare with the new one, keep the edits. */
  useNewerFile() {
    if (!this.fileNewer) return;
    this.file = this.fileNewer;
    this.fileNewer = null;
    this.editFile(this.draft ?? this.file.content);
  }

  async applyFile() {
    if (this.draft === null || this.busy === "config-file" || this.ask) return;
    const text = this.draft;
    const { added, removed } = this.fileDiff;
    const check = this.fileCheck?.state === "done" && this.fileCheck.of === text ? this.fileCheck : null;
    if (check && !check.ok) return;
    const name = this.file.path.split("/").pop();
    const ok = await this.confirm({
      title: `Apply your edits to ${name}?`,
      body: `rws checks the new text, keeps the file as it is now in ${name}.bak, writes the new one and reloads. Open connections are kept.`,
      action: "Apply",
      tone: "warn",
      effects: [
        ["info", `${plural(added, "line", "lines")} added, ${removed.toLocaleString()} removed`],
        ...(check?.restart ?? []).map((what) => ["warn", `${what}: needs a restart to take effect`]),
        ...(check?.warnings ?? []).map((w) => ["warn", w]),
      ],
    });
    if (!ok || this.draft !== text) return;
    this.busy = "config-file";
    this.fileError = "";
    try {
      const done = await api("/config/file", { method: "PUT", body: { content: text, sha256: this.file.sha256 } });
      this.draft = null;
      this.fileCheck = null;
      this.fileNewer = null;
      this.applied = { ...done, at: Date.now() };
      this.toast(done.changed ? `Applied and reloaded. The previous file is ${done.backup ?? "kept beside it"}` : "Nothing to apply: the file already says this");
      await this.load("config");
    } catch (e) {
      if (e instanceof ApiError && e.status === 409 && this.file?.editable) {
        const fresh = await api("/config/file").catch(() => null);
        if (fresh?.editable && fresh.sha256 !== this.file.sha256) this.fileNewer = fresh;
        else this.fileError = e.message;
      } else if (e instanceof ApiError && e.status === 422 && Array.isArray(e.data?.errors)) {
        this.fileCheck = { state: "done", of: text, ok: false, errors: e.data.errors, warnings: e.data.warnings ?? [], restart: [] };
      } else {
        this.fileError = e.message;
      }
    } finally {
      this.busy = "";
    }
  }

  // ── actions ───────────────────────────────────────────────────────────
  /** An action: a message (with Undo when `undo` is given) on success. */
  async act(label, path, options, after, undo = null) {
    this.busy = label;
    try {
      const result = await this.call(path, options);
      if (after) this.toast(after(result), undo);
      await this.load(this.view);
      return true;
    } catch (e) {
      // Errors of an action show where the action is, not at the page top.
      if (this.error) {
        this.toast(`Didn't work: ${this.error}`);
        this.error = "";
      }
      return false;
    } finally {
      this.busy = "";
    }
  }

  async drain(upstream, backend, drained) {
    const backends = (this.data.upstreams ?? []).find((u) => u.name === upstream)?.backends ?? [];
    const serving = backends.filter((b) => !b.drained && b.healthy && b.url !== backend);
    if (drained && serving.length === 0) {
      const ok = await this.confirm({
        title: `Take ${backend} out of rotation?`,
        body: `It is the last healthy backend of ${upstream}: every request to ${upstream} will get 503 until one is back.`,
        action: "Drain anyway",
      });
      if (!ok) return false;
    }
    const done = await this.act(`drain:${backend}`, `/upstreams/${encodeURIComponent(upstream)}/drain`, { method: "PUT", body: { backend, drained } },
      () => (drained ? `${backend} drained: it gets no new requests` : `${backend} is back in rotation`),
      () => this.drain(upstream, backend, !drained));
    if (done) {
      // Every page shows backends (the header, Needs attention): don't wait for the next poll.
      this.data = { ...this.data, upstreams: (this.data.upstreams ?? []).map((u) => (u.name !== upstream ? u : { ...u, backends: u.backends.map((b) => (b.url === backend ? { ...b, drained, drained_since: drained ? b.drained_since ?? Math.floor(Date.now() / 1000) : null } : b)) })) };
    }
    return done;
  }

  /** What /maintenance answered, shown at once on every page. */
  maintenanceIs(m) {
    this.data = { ...this.data, status: { ...this.data.status, maintenance: m.active, maintenance_until: m.until ?? null }, maintenance: m };
  }

  async setMaintenance(active) {
    const body = { active };
    if (active) {
      // What clients are told to wait when nothing is picked: the config's value.
      const configured = (await api("/maintenance").catch(() => null))?.retry_after_secs;
      const ok = await this.confirm({
        title: "Turn on maintenance mode?",
        body: "Visitors stop reaching your sites until it is off again.",
        effects: (picked) => [
          ["warn", "Every visitor gets 503 Service Unavailable; browsers see a maintenance page."],
          ["ok", "Health checks keep passing."],
          ["ok", "This console and the admin API keep working."],
          ["info", picked.off
            ? `It turns off by itself at ${hourMinute(Date.now() + picked.off * 1000)}, or when the server restarts.`
            : "It stays on until someone turns it off or the server restarts."],
        ],
        choices: [
          { id: "retry", label: "Tell clients to retry after", value: "",
            options: [["", typeof configured === "number" ? (configured ? `${span(configured)} (config)` : "No hint (config)") : "As configured"], ...RETRY_AFTER.map((s) => [s, span(s)])] },
          { id: "off", label: "Turn off by itself after", value: 0, options: [...TURN_OFF_AFTER.map((s) => [s, span(s)]), [0, "Never"]] },
        ],
        type: "maintenance",
        action: "Turn on maintenance",
        tone: "warn",
      });
      if (!ok) return;
      if (this.picked.off) body.for_secs = this.picked.off;
      if (this.picked.retry !== "") body.retry_after_secs = this.picked.retry;
    }
    await this.act("maintenance", "/maintenance", { method: "PUT", body }, (r) => {
      this.maintenanceIs(r);
      if (!r.active) return "Maintenance off";
      return r.until ? `Maintenance on until ${hourMinute(r.until * 1000)}: visitors get 503` : "Maintenance on: visitors get 503";
    });
  }

  /** "Add 1 hour": a timed maintenance stays on that much longer. */
  extendMaintenance(secs = 3600) {
    return this.act("maintenance-extend", "/maintenance", { method: "PUT", body: { add_secs: secs } }, (r) => {
      this.maintenanceIs(r);
      return `Maintenance stays on until ${hourMinute(r.until * 1000)}`;
    });
  }

  reload() {
    return this.act("reload", "/reload", { method: "POST" }, (r) =>
      r.applied ? "Configuration reloaded" : `Reload kept the running configuration: ${r.errors.join("; ")}`);
  }

  renew() {
    return this.act("renew", "/certificates/renew", { method: "POST" }, () => "Renewal requested: the new certificate is swapped in when it's issued");
  }

  async blockIp(ip) {
    const ok = await this.act("block", "/blocklist", { method: "POST", body: { ip } }, (r) => `${r.blocked} blocked`,
      () => this.act(`unblock:${ip}`, `/blocklist/${encodeURIComponent(ip)}`, { method: "DELETE" }, () => `${ip} unblocked`));
    if (ok && this.data.blocklist && !this.data.blocklist.includes(ip)) this.data = { ...this.data, blocklist: [...this.data.blocklist, ip] };
    return ok;
  }

  block(event) {
    event.preventDefault();
    const field = this.querySelector("#block-ip");
    const ip = (field?.value ?? "").trim();
    if (!ip) {
      this.fieldError = { ...this.fieldError, block: "Enter an IP address." };
      return;
    }
    if (!looksLikeIp(ip)) {
      this.fieldError = { ...this.fieldError, block: `${ip} isn't an IPv4 or IPv6 address.` };
      return;
    }
    this.fieldError = { ...this.fieldError, block: "" };
    this.blockIp(ip).then((ok) => {
      if (ok && field) field.value = "";
    });
  }

  async unblock(ip) {
    const ok = await this.act(`unblock:${ip}`, `/blocklist/${encodeURIComponent(ip)}`, { method: "DELETE" }, () => `${ip} unblocked`,
      () => this.act("block", "/blocklist", { method: "POST", body: { ip } }, () => `${ip} blocked again`));
    if (ok && this.data.blocklist) this.data = { ...this.data, blocklist: this.data.blocklist.filter((x) => x !== ip) };
    return ok;
  }

  async loadAuthz() {
    try {
      const [schema, list] = await Promise.all([api("/authz"), api("/authz/tuples?limit=200")]);
      this.data = { ...this.data, authz: { schema, tuples: list.tuples } };
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) this.data = { ...this.data, authz: null };
    }
  }

  addTuple(event) {
    event.preventDefault();
    const field = this.querySelector("#authz-tuple");
    const tuple = (field?.value ?? "").trim();
    if (!tuple) return;
    this.act("authz-add", "/authz/tuples", { method: "POST", body: { tuple } }, (r) => (r.written ? `${r.tuple} added` : `${r.tuple} was already there`)).then((ok) => {
      if (ok && field) field.value = "";
    });
  }

  async checkAccess(event) {
    event.preventDefault();
    const value = (id) => (this.querySelector(id)?.value ?? "").trim();
    const body = { subject: value("#authz-subject"), relation: value("#authz-relation"), object: value("#authz-object") };
    if (!body.subject || !body.relation || !body.object) return;
    try {
      const result = await this.call("/authz/check", { method: "POST", body });
      this.data = { ...this.data, authzCheck: { ...body, ...result } };
    } catch {
      // Shown through this.error.
    }
  }

  /** Purge one path (or a prefix ending in *) from cache `name`. */
  async purgePath(event, name) {
    event.preventDefault();
    const field = event.target.querySelector("lit-material-text-field");
    const path = (field?.value ?? "").trim();
    if (!path) {
      this.fieldError = { ...this.fieldError, [`purge:${name}`]: "Enter a path, e.g. /products/42 or /products/*" };
      return;
    }
    this.fieldError = { ...this.fieldError, [`purge:${name}`]: "" };
    const query = path.endsWith("*") ? `?prefix=${encodeURIComponent(path.slice(0, -1))}` : `?path=${encodeURIComponent(path)}`;
    const ok = await this.act(`purge:${name}`, `/caches/${encodeURIComponent(name)}${query}`, { method: "DELETE" },
      (r) => `${plural(r.purged, "entry", "entries")} purged from ${r.name}`);
    if (ok && field) field.value = "";
  }

  /** Empty cache `name`, after asking. */
  async purgeAll(name, entries) {
    const ok = await this.confirm({
      title: `Purge everything from ${name}?`,
      body: `All ${entries.toLocaleString()} cached ${entries === 1 ? "response is" : "responses are"} dropped; the next requests go to the backends until it fills again.`,
      action: "Purge everything",
    });
    if (!ok) return;
    await this.act(`purge:${name}`, `/caches/${encodeURIComponent(name)}`, { method: "DELETE" },
      (r) => `${plural(r.purged, "entry", "entries")} purged from ${r.name}`);
  }

  /** Empty every cache, after asking. */
  async purgeEvery() {
    const caches = (this.data.caches ?? []).filter((c) => c.entries);
    const total = caches.reduce((n, c) => n + c.entries, 0);
    const ok = await this.confirm({
      title: "Purge every cache?",
      body: `${plural(total, "cached response", "cached responses")} in ${plural(caches.length, "cache", "caches")} are dropped; the next requests go to the backends until the caches fill again.`,
      action: "Purge every cache",
    });
    if (!ok) return;
    this.busy = "purge-every";
    let purged = 0;
    try {
      for (const c of caches) purged += (await this.call(`/caches/${encodeURIComponent(c.name)}`, { method: "DELETE" })).purged;
      this.toast(`${plural(purged, "entry", "entries")} purged from ${plural(caches.length, "cache", "caches")}`);
    } catch {
      this.toast(`Didn't work: ${this.error || "the server didn't answer"}`);
      this.error = "";
    } finally {
      this.busy = "";
      this.load(this.view);
    }
  }

  // ── what the page says about the server ───────────────────────────────
  /** Backends of every upstream: [healthy and in rotation, all]. */
  backendHealth() {
    const all = (this.data.upstreams ?? []).flatMap((u) => u.backends);
    return [all.filter((b) => b.healthy && !b.drained).length, all.length];
  }

  /** The one header chip: disconnected, maintenance, degraded, not ready or ready. */
  serverState() {
    const info = this.data.status;
    if (this.conn.lost) return { tone: "bad", label: "Disconnected", detail: this.conn.at ? `last answer ${time(this.conn.at)}` : "retrying", view: "overview", title: "The admin API doesn't answer; retrying every 5 seconds" };
    if (!info) return null;
    if (info.maintenance) return { tone: "warn", label: "Maintenance", detail: "visitors get 503", view: "controls" };
    const [up, all] = this.backendHealth();
    if (all > 0 && up < all) return { tone: up === 0 ? "bad" : "warn", label: "Degraded", detail: `${up} of ${all} backends serving`, view: "upstreams" };
    if (!info.ready) return { tone: "bad", label: "Not ready", detail: "readiness checks fail", view: "overview" };
    return { tone: "ok", label: "Ready", detail: all ? `${all} of ${all} backends serving` : `up ${duration(info.uptime_secs)}`, view: "overview" };
  }

  /** Things a person should look at, worst first; each may carry one action. */
  attention() {
    const d = this.data;
    const list = [];
    if (d.status && !d.status.ready && !d.status.maintenance) {
      list.push({ tone: "bad", title: "The server reports not ready", detail: "Load balancers that probe readiness stop sending it traffic." });
    }
    for (const u of d.upstreams ?? []) {
      for (const b of u.backends) {
        const open = { label: "Open Upstreams", run: () => this.show("upstreams") };
        if (!b.healthy) list.push({ tone: "bad", title: `${b.url} is failing health checks`, detail: `Upstream ${u.name}: it gets no requests until the checks pass.`, action: open });
        else if (b.breaker === "open") list.push({ tone: "bad", title: `${b.url}: circuit breaker open`, detail: `Upstream ${u.name}: too many failures, requests are held back until a probe succeeds.`, action: open });
        else if (b.breaker === "half_open") list.push({ tone: "warn", title: `${b.url}: circuit breaker probing`, detail: `Upstream ${u.name}: a few requests are let through to see if it recovered.`, action: open });
        if (b.drained) {
          list.push({
            tone: "warn", title: b.open_requests > 0 ? `${b.url} is draining` : `${b.url} is drained`,
            detail: b.open_requests > 0
              ? `Upstream ${u.name}: ${plural(b.open_requests, "request is", "requests are")} still running on it.`
              : `Upstream ${u.name}: no open requests, safe to deploy. Out of rotation until you put it back.`,
            action: { label: "Put back", busy: `drain:${b.url}`, run: () => this.drain(u.name, b.url, false) },
          });
        }
      }
    }
    const acme = d.certificates?.acme?.running;
    for (const c of d.certificates?.certificates ?? []) {
      if (c.days_left >= 21) continue;
      list.push({
        tone: c.days_left < 7 ? "bad" : "warn",
        title: c.days_left < 0 ? `The certificate for ${c.name} has expired` : `The certificate for ${c.name} expires in ${plural(c.days_left, "day", "days")}`,
        detail: acme ? "ACME renews it by itself; you can also renew now." : "Replace the certificate files, then reload the configuration.",
        action: acme ? { label: "Renew now", busy: "renew", run: () => this.renew() } : { label: "Open Certificates", run: () => this.show("certificates") },
      });
    }
    for (const disk of d.status?.disk ?? []) {
      if (disk.low) list.push({ tone: "warn", title: `Disk almost full: ${disk.dir}`, detail: `${bytes(disk.free_bytes)} free of ${bytes(disk.total_bytes)}. rws has stopped writing the access log and disk cache there.` });
    }
    const rank = { bad: 0, warn: 1 };
    return list.sort((a, b) => rank[a.tone] - rank[b.tone]);
  }

  // ── rendering: the shell ──────────────────────────────────────────────
  renderSide() {
    const info = this.data.status;
    return html`<aside class="side">
      <a class="brand" href="#/overview" aria-label="rws admin, Overview">${brandMark}<span>rws</span>${info ? html`<span class="ver">${info.version}</span>` : nothing}</a>
      <nav aria-label="Sections">
        ${GROUPS.map((group) => html`<div class="nav-group" role="group" aria-label=${group}>
          <div class="nav-label" aria-hidden="true">${group}</div>
          ${VIEWS.filter((v) => v.group === group).map((v) => html`<a class="nav-link" href="#/${v.id}" aria-current=${this.view === v.id ? "page" : nothing} aria-keyshortcuts="G ${v.key.toUpperCase()}">
            <span class="icon">${icon(v.icon)}</span>${v.label}${v.id === "config" && this.draft !== null ? html`<span class="nav-dot" title="Edits not applied"><span class="sr">, edits not applied</span></span>` : nothing}<kbd aria-hidden="true">G ${v.key.toUpperCase()}</kbd></a>`)}
        </div>`)}
      </nav>
      ${this.me ? html`<div class="side-foot">
        <span class="who" title="Signed in ${this.me.via === "oidc" ? "with single sign-on" : "with a token"}">${this.me.user} · ${this.me.role}</span>
        <button class="btn btn-quiet" @click=${() => this.logout()}><span class="icon">${icon("sign-out")}</span>Sign out</button>
      </div>` : nothing}
    </aside>`;
  }

  renderTop() {
    const state = this.serverState();
    const label = VIEWS.find((v) => v.id === this.view)?.label ?? "Overview";
    return html`<header class="top">
      <span class="top-brand">${brandMark}<span class="top-title">${label}</span></span>
      ${state ? html`<button class="state" data-tone=${state.tone} title=${state.title ?? "The server's state. Click to see why."} @click=${() => this.show(state.view)}>
        <span class="dot" aria-hidden="true"></span><span>${state.label}</span>${state.detail ? html`<span class="detail">${state.detail}</span>` : nothing}</button>` : nothing}
      <button class="cmd" aria-label="Search or run a command" aria-keyshortcuts="Control+K Meta+K" @click=${() => this.openPalette()}>
        <span class="icon">${icon("magnifying-glass")}</span><span class="cmd-text">Search or run a command</span><kbd aria-hidden="true">${MOD} K</kbd></button>
      <rws-ago class="updated" .at=${this.conn.at}></rws-ago>
    </header>`;
  }

  renderTabbar() {
    const more = !PRIMARY.includes(this.view);
    return html`<nav class="tabbar" aria-label="Sections">
        ${PRIMARY.map((id) => VIEWS.find((v) => v.id === id)).map((v) => html`<a href="#/${v.id}" aria-current=${this.view === v.id ? "page" : nothing}><span class="icon">${icon(v.icon)}</span>${v.label}</a>`)}
        <button class=${more ? "is-current" : ""} aria-expanded=${this.moreOpen} aria-controls="more-sheet" @click=${() => this.toggleMore()}><span class="icon">${icon("dots-three")}</span>More</button>
      </nav>
      ${this.moreOpen
        ? html`<div class="more-scrim" @click=${() => (this.moreOpen = false)}></div>
          <nav id="more-sheet" class="more-sheet" aria-label="More sections">
            ${VIEWS.filter((v) => !PRIMARY.includes(v.id)).map((v) => html`<a href="#/${v.id}" aria-current=${this.view === v.id ? "page" : nothing}
              @click=${() => (this.moreOpen = false)}><span class="icon">${icon(v.icon)}</span>${v.label}${v.id === "config" && this.draft !== null ? html`<span class="nav-dot" title="Edits not applied"><span class="sr">, edits not applied</span></span>` : nothing}</a>`)}
            ${this.me ? html`<div class="more-foot"><span class="who">${this.me.user} · ${this.me.role}</span>
              <button class="btn btn-quiet" @click=${() => this.logout()}><span class="icon">${icon("sign-out")}</span>Sign out</button></div>` : nothing}
          </nav>`
        : nothing}`;
  }

  toggleMore() {
    this.moreOpen = !this.moreOpen;
    if (this.moreOpen) this.updateComplete.then(() => this.querySelector(".more-sheet a")?.focus());
  }

  render() {
    if (!this.authed) return this.renderLogin();
    const info = this.data.status;
    const ask = this.ask;
    return html`
      <div class="shell">
        ${this.renderSide()}
        <div class="body">
          ${this.renderTop()}
          <main id="main" tabindex="-1">
            ${info?.maintenance
              ? html`<div class="banner" role="status">
                  <span class="banner-title">Maintenance mode is on</span>
                  ${info.maintenance_until
                    ? html`<span class="banner-text">Visitors get 503 Service Unavailable. It turns off by itself at ${hourMinute(info.maintenance_until * 1000)}, <rws-left .at=${info.maintenance_until * 1000}></rws-left>.</span>
                        <button class="btn btn-warn" ?disabled=${this.busy === "maintenance-extend"} @click=${() => this.extendMaintenance()}>Add 1 hour</button>
                        <button class="btn btn-warn-fill" ?disabled=${this.busy === "maintenance"} @click=${() => this.setMaintenance(false)}>Turn off now</button>`
                    : html`<span class="banner-text">Visitors get 503 Service Unavailable until you turn it off.</span>
                        <button class="btn btn-warn-fill" ?disabled=${this.busy === "maintenance"} @click=${() => this.setMaintenance(false)}>Turn off</button>`}
                </div>`
              : nothing}
            ${this.error ? html`<lit-material-alert variant="error"><span slot="title">Couldn't load this page</span>${this.error}
                <lit-material-button variant="text" @click=${() => this.load(this.view)}>Retry</lit-material-button></lit-material-alert>` : nothing}
            ${this.renderView()}
          </main>
        </div>
      </div>
      ${this.renderTabbar()}
      <div class="toast" role="status" aria-live="polite">
        ${this.notice
          ? html`<div class="toast-body"><span>${this.notice.message}</span>
              ${this.notice.undo ? html`<button class="toast-action" @click=${() => { const undo = this.notice.undo; this.notice = null; undo(); }}>Undo</button>` : nothing}
              <button class="toast-close" aria-label="Dismiss" @click=${() => (this.notice = null)}>✕</button></div>`
          : nothing}
      </div>
      <lit-material-command-palette label="Search or run a command" placeholder="Search or run a command" .items=${this.paletteItems()}
        .hints=${PALETTE_HINTS}
        @input=${(e) => (this.query = e.composedPath()[0]?.value ?? "")}
        @close=${() => (this.query = "")}
        @select=${(e) => { this.query = ""; e.detail.item.run?.(); }}></lit-material-command-palette>
      <dialog id="confirm" class="confirm" role="alertdialog" data-tone=${ask?.tone ?? "danger"} aria-labelledby="confirm-title" aria-describedby="confirm-body"
        @cancel=${(e) => { e.preventDefault(); this.answer(false); }}>
        ${ask
          ? html`<form novalidate @submit=${(e) => { e.preventDefault(); if (this.confirmReady()) this.answer(true); }}>
            <h2 id="confirm-title">${ask.title}</h2>
            <p id="confirm-body">${ask.body}</p>
            ${ask.choices.map((c) => html`<fieldset class="choice"><legend>${c.label}</legend>
              <div class="choice-options">${c.options.map(([value, label]) => html`<label><input type="radio" name="confirm-${c.id}" .checked=${this.picked[c.id] === value}
                @change=${() => (this.picked = { ...this.picked, [c.id]: value })}><span>${label}</span></label>`)}</div>
            </fieldset>`)}
            ${ask.effects ? html`<ul class="effects">${(typeof ask.effects === "function" ? ask.effects(this.picked) : ask.effects).map(([tone, text]) => html`<li><span class="sq" data-tone=${tone}></span><span>${text}</span></li>`)}</ul>` : nothing}
            ${ask.type
              ? html`<lit-material-text-field id="confirm-type" variant="outlined" label="Type ${ask.type} to confirm" autocomplete="off" autocapitalize="none" spellcheck="false"
                  @input=${(e) => (this.typed = e.target.value ?? "")}></lit-material-text-field>`
              : nothing}
            <div class="confirm-actions">
              <lit-material-button class="cancel" type="button" variant="text" @click=${() => this.answer(false)}>Cancel</lit-material-button>
              <lit-material-button class="go" type="submit" ?disabled=${!this.confirmReady()}>${ask.action}</lit-material-button>
            </div>
          </form>`
          : nothing}
      </dialog>
    `;
  }

  renderLogin() {
    return html`
      <div class="login">
        <section class="login-card card" aria-labelledby="login-title">
          <div class="login-brand">${brandMark}<span>rws admin</span></div>
          <h1 id="login-title">Sign in</h1>
          ${this.sso ? html`<lit-material-button href="/admin/auth/login" style="width: 100%">Sign in with single sign-on</lit-material-button>
            <p class="login-or">or with a token</p>` : nothing}
          <p>Paste an admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names), or your own from <code>[admin] users</code>. It stays in this tab only.</p>
          <form @submit=${(e) => this.login(e)} novalidate>
            <lit-material-text-field id="token" class="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required autofocus
              ?error=${Boolean(this.loginError)} error-text=${this.loginError ?? ""}></lit-material-text-field>
            <lit-material-button type="submit">Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `;
  }

  renderView() {
    switch (this.view) {
      case "upstreams": return this.renderUpstreams();
      case "certificates": return this.renderCertificates();
      case "controls": return this.renderControls();
      case "traffic": return html`<rws-traffic></rws-traffic>`;
      case "config": return this.renderConfig();
      case "logs": return this.renderLogs();
      case "audit": return this.renderAudit();
      default: return this.renderOverview();
    }
  }

  head(title, sub = nothing) {
    return html`<div class="page-head"><h1>${title}</h1>${sub === nothing ? nothing : html`<span class="sub">${sub}</span>`}</div>`;
  }

  skeleton(rows = 3) {
    return html`<div class="skeleton" aria-busy="true">${Array.from({ length: rows }, () => html`<div class="card"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`;
  }

  // ── rendering: Overview ───────────────────────────────────────────────
  renderOverview() {
    const s = this.data.status, m = this.data.metrics;
    if (!s || !m) return html`${this.head("Overview")}${this.skeleton(4)}`;
    const errorRate = m.requests_total ? (m.errors_total / m.requests_total) * 100 : 0;
    const pts = this.samples;
    const now = pts.at(-1);
    const peak = pts.length ? Math.max(...pts.map((p) => p.y)) : 0;
    const mean = pts.length ? pts.reduce((n, p) => n + p.y, 0) / pts.length : 0;
    const span = pts.length > 1 ? (now.x - pts[0].x) / 1000 : 0;
    const over = span >= 90 ? `${Math.round(span / 60)} min` : `${Math.round(span)} s`;
    const needs = this.attention();
    return html`
      ${this.head("Overview", html`Up ${duration(s.uptime_secs)}. Numbers refresh every 2 seconds; this console's own requests aren't counted.`)}
      <section class="card pulse" aria-labelledby="pulse-title">
        <div class="pulse-top">
          <div class="pulse-main">
            <span class="pulse-label" id="pulse-title">Requests per second</span>
            <span class="pulse-big">${now ? rate(now.y) : "–"}</span>
            <span class="pulse-cmp">${pts.length > 1 ? html`Peak ${rate(peak)}, average ${rate(mean)} over the last ${over}.` : "Measuring: the first number arrives in a few seconds."}</span>
          </div>
          <dl class="vitals">
            <div class="vital"><dt>Errors</dt><dd>${errorRate.toFixed(2)}%</dd>
              <span class="note" data-tone=${errorRate >= 5 ? "bad" : errorRate >= 1 ? "warn" : "none"}>${m.errors_total.toLocaleString()} since start</span></div>
            <div class="vital"><dt>Open connections</dt><dd>${m.active_connections.toLocaleString()}</dd><span class="note">right now</span></div>
            <div class="vital"><dt>Queued for a worker</dt><dd>${m.thread_pool_queued.toLocaleString()}</dd>
              <span class="note" data-tone=${m.thread_pool_queued > 0 ? "warn" : "none"}>${m.thread_pool_queued > 0 ? "waiting for a free thread" : "nothing waits"}</span></div>
            <div class="vital"><dt>Requests served</dt><dd>${m.requests_total.toLocaleString()}</dd><span class="note">since start</span></div>
          </dl>
        </div>
        ${this.renderChart()}
      </section>
      <div class="cols ov">
        <div class="stack">
          <section class="card ov-attn" aria-labelledby="attn-title">
            <div class="card-head"><h2 id="attn-title">Needs attention</h2>${needs.length ? html`<span class="count">${needs.length}</span>` : nothing}</div>
            ${needs.length === 0
              ? html`<div class="all-clear"><span class="sq" data-tone="ok"></span>Nothing needs you: backends, certificates and disk are fine.</div>`
              : needs.map((n) => html`<div class="attn">
                  <span class="sq" data-tone=${n.tone}></span>
                  <div class="attn-text"><span class="attn-title">${n.title}</span><span class="attn-detail">${n.detail}</span></div>
                  ${n.action ? html`<button class="btn" ?disabled=${Boolean(n.action.busy) && this.busy === n.action.busy} @click=${n.action.run}>${n.action.label}</button>` : nothing}
                </div>`)}
          </section>
          <section class="card ov-live" aria-labelledby="live-title">
            <div class="card-head"><h2 id="live-title">Live requests</h2>
              <span class="meta">The newest eight. This console's own aren't logged.</span><span class="spacer"></span>
              <a class="link" href="#/logs">Open Logs</a></div>
            <div class="reqs" role="table" aria-labelledby="live-title">
              ${requestHead}
              ${!this.recent ? html`<div class="req-none">Loading…</div>`
                : this.recent.length === 0 ? html`<div class="req-none">No requests yet. They appear here as they arrive.</div>`
                : this.recent.map(requestRow)}
            </div>
          </section>
          <section class="card ov-server" aria-labelledby="server-title">
            <div class="card-head"><h2 id="server-title">Server</h2></div>
            <dl class="facts">
              <dt>Main listener</dt><dd>${s.listeners.main.address} <span class="plain">${s.listeners.main.tls ? "HTTPS, HTTP/2, HTTP/3" : "plain HTTP"}</span></dd>
              <dt>HTTP redirect</dt><dd>${s.listeners.redirect ?? html`<span class="plain">off</span>`}</dd>
              <dt>Admin</dt><dd>${s.listeners.admin}</dd>
              <dt>Uptime</dt><dd>${duration(s.uptime_secs)}</dd>
              <dt>Process</dt><dd>${s.pid}</dd>
              <dt>Version</dt><dd>${s.version}</dd>
              <dt>Built with</dt><dd><span class="plain">${s.features.join(", ")}</span></dd>
            </dl>
          </section>
        </div>
        <div class="stack">${this.renderBreakdown()}</div>
      </div>
    `;
  }

  /**
   * The last five minutes of /breakdown (since start until this page has
   * been open for a while, or when nothing arrived in that time).
   */
  breakdownWindow() {
    const latest = this.data.breakdown;
    if (!latest) return null;
    const base = this.bases[0];
    const span = base ? latest.at - base.at : 0;
    const recent = span >= WINDOW_MIN_MS ? between(latest, base.b) : null;
    if (recent?.count) {
      const secs = span / 1000;
      return { ...recent, bounds: latest.upstream_bounds, over: `over the last ${secs >= 90 ? `${Math.round(secs / 60)} min` : `${Math.round(secs)} s`}` };
    }
    return { ...between(latest, null), bounds: latest.upstream_bounds, over: "since start" };
  }

  /** Response time (p50, p95, p99) and where requests go: by host and by upstream. */
  renderBreakdown() {
    const w = this.breakdownWindow();
    if (!w) return nothing; // a server before 18.38, or a role that can't read it
    const p = (q) => { const v = quantile(w, q); return v === null ? "–" : latency(v); };
    const pct = (n, of) => (of ? Math.round((n / of) * 100) : 0);
    const row = ({ name, mono = true, requests, of, note }) => html`<li>
      <span class="where-name ${mono ? "mono" : ""}" title=${name}>${name}</span>
      <span class="where-pct">${pct(requests, of)}%</span>
      <span class="share-track"><span class="share-fill" style="width: ${Math.min(100, (requests / (of || 1)) * 100)}%"></span></span>
      <span class="where-note">${plural(requests, "request", "requests")}${note ?? nothing}</span>
    </li>`;
    const failed = (n) => (n ? html` · <span class="bad">${plural(n, "error", "errors")}</span>` : nothing);

    const named = w.hosts.slice(0, HOSTS_SHOWN), rest = w.hosts.slice(HOSTS_SHOWN);
    const other = rest.reduce((sum, h) => ({ requests: sum.requests + h.requests, errors: sum.errors + h.errors }), { ...w.otherHosts });
    const byHost = named.reduce((n, h) => n + h.requests, 0) + other.requests;
    // An upstream counts each try, so retries can push the sum past the requests.
    const proxied = w.upstreams.reduce((n, u) => n + u.requests, 0);
    const all = Math.max(w.count, proxied);
    return html`
      <section class="card ov-rt" aria-labelledby="rt-title">
        <div class="card-head"><h2 id="rt-title">Response time</h2>
          <span class="meta">${w.count ? html`${plural(w.count, "request", "requests")} ${w.over}` : "No requests yet"}</span></div>
        <dl class="vitals vitals-3">
          <div class="vital"><dt>p50</dt><dd>${p(0.5)}</dd><span class="note">half are faster</span></div>
          <div class="vital"><dt>p95</dt><dd>${p(0.95)}</dd><span class="note">1 in 20 is slower</span></div>
          <div class="vital"><dt>p99</dt><dd>${p(0.99)}</dd><span class="note">1 in 100 is slower</span></div>
        </dl>
      </section>
      <section class="card ov-where" aria-labelledby="where-title">
        <div class="card-head"><h2 id="where-title">Where requests go</h2><span class="meta">${w.count ? w.over : nothing}</span></div>
        ${!w.count ? html`<div class="all-clear">No requests yet. Hosts and upstreams appear here as requests arrive.</div>` : html`
          <h3 class="where-head" id="where-hosts">By host</h3>
          <ul class="where" aria-labelledby="where-hosts">
            ${named.map((h) => row({ name: h.host || "No host named", mono: Boolean(h.host), requests: h.requests, of: byHost, note: failed(h.errors) }))}
            ${other.requests ? row({ name: rest.length ? plural(rest.length, "other host", "other hosts") : "Other hosts", mono: false, requests: other.requests, of: byHost, note: failed(other.errors) }) : nothing}
          </ul>
          ${w.upstreams.length ? html`
            <h3 class="where-head" id="where-upstreams">By upstream</h3>
            <ul class="where" aria-labelledby="where-upstreams">
              ${w.upstreams.map((u) => {
                const slow = under(u, w.bounds, 0.95);
                const speed = slow === undefined ? nothing : slow === null ? ` · 1 in 20 took over ${latency(w.bounds.at(-1))}` : ` · 95% under ${latency(slow)}`;
                return row({ name: u.name, requests: u.requests, of: all, note: html`${speed}${failed(u.errors)}` });
              })}
              ${all > proxied ? row({ name: "rws itself", mono: false, requests: all - proxied, of: all, note: " · files, cached and configured responses" }) : nothing}
            </ul>` : nothing}`}
      </section>
    `;
  }

  /** Requests per second as an area chart, with a mark where errors happened. */
  renderChart() {
    const pts = this.samples;
    if (pts.length < 2) return html`<div class="chart-empty">The chart fills in as this page stays open.</div>`;
    const W = 1000, H = 200, top = 16, bottom = 8;
    const x0 = pts[0].x, span = Math.max(1, pts.at(-1).x - x0);
    const max = niceMax(Math.max(...pts.map((p) => p.y)));
    const px = (p) => ((p.x - x0) / span) * W;
    const py = (v) => top + (1 - v / max) * (H - top - bottom);
    const line = pts.map((p, i) => `${i ? "L" : "M"}${px(p).toFixed(1)} ${py(p.y).toFixed(1)}`).join("");
    const errors = pts.filter((p) => p.e > 0);
    const at = this.hover !== null && pts[this.hover] ? pts[this.hover] : null;
    const peak = Math.max(...pts.map((p) => p.y));
    const move = (e) => {
      const box = e.currentTarget.getBoundingClientRect();
      const target = x0 + Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) * span;
      let best = 0;
      for (let i = 1; i < pts.length; i++) if (Math.abs(pts[i].x - target) < Math.abs(pts[best].x - target)) best = i;
      if (best !== this.hover) this.hover = best;
    };
    const pct = (v, of) => `${((v / of) * 100).toFixed(2)}%`;
    return html`
      <div class="chart" @pointermove=${move} @pointerdown=${move} @pointerleave=${() => (this.hover = null)}>
        <svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" role="img"
          aria-label="Requests per second over the last ${Math.round(span / 1000)} seconds: now ${rate(pts.at(-1).y)}, peak ${rate(peak)}${errors.length ? `, errors in ${plural(errors.length, "sample", "samples")}` : ""}">
          ${[0, 0.5, 1].map((f) => svg`<line class="grid-line" x1="0" x2=${W} y1=${py(max * f)} y2=${py(max * f)} vector-effect="non-scaling-stroke"></line>`)}
          <path class="area" d="${line}L${W} ${py(0)}L0 ${py(0)}Z"></path>
          <path class="line" d=${line} vector-effect="non-scaling-stroke"></path>
          ${errors.map((p) => svg`<rect class="err" x=${px(p) - 2} y=${H - 6} width="4" height="6"></rect>`)}
          ${at ? svg`<line class="cursor" x1=${px(at)} x2=${px(at)} y1="0" y2=${H} vector-effect="non-scaling-stroke"></line>` : nothing}
        </svg>
        <span class="chart-y" style="top: ${pct(py(max), H)}">${rate(max)}</span>
        <span class="chart-y" style="top: ${pct(py(max / 2), H)}">${rate(max / 2)}</span>
        ${at ? html`<span class="chart-dot" style="left: ${pct(px(at), W)}; top: ${pct(py(at.y), H)}"></span>
          <span class="chart-tip" style="left: clamp(90px, ${pct(px(at), W)}, calc(100% - 90px))">${time(at.x)} · ${rate(at.y)} req/s${at.e ? ` · ${plural(at.e, "error", "errors")}` : ""}</span>` : nothing}
      </div>
      <div class="chart-x" aria-hidden="true"><span>${time(x0)}</span>
        ${errors.length ? html`<span class="err-key"><span class="sq" data-tone="bad"></span>errors</span>` : nothing}<span>now</span></div>
    `;
  }

  // ── rendering: Upstreams ──────────────────────────────────────────────
  renderUpstreams() {
    const ups = this.data.upstreams;
    if (!ups) return html`${this.head("Upstreams")}${this.skeleton(2)}`;
    const backends = ups.reduce((n, u) => n + u.backends.length, 0);
    const changes = (this.data.audit ?? []).filter((e) => e.path.includes("/upstreams/") && e.status < 300).slice(0, 6);
    const drained = ups.flatMap((u) => u.backends.filter((b) => b.drained).map((b) => [u, b]));
    return html`
      ${this.head("Upstreams", ups.length ? html`${plural(ups.length, "upstream", "upstreams")}, ${plural(backends, "backend", "backends")}. Drain a backend before deploying it: new requests go elsewhere, requests already on it finish, and this page says when none is left.` : nothing)}
      ${ups.length === 0
        ? html`<div class="card"><lit-material-empty-state size="compact" headline="No upstreams configured">Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</lit-material-empty-state></div>`
        : html`${drained.map(([u, b]) => this.renderDrain(u, b))}<div class="cols cols-side">
          <div class="stack">${ups.map((u) => this.renderUpstream(u))}</div>
          <section class="card" aria-labelledby="changes-title">
            <div class="card-head"><h2 id="changes-title">Recent changes</h2><span class="spacer"></span><a class="link" href="#/audit">Open Audit</a></div>
            ${changes.length === 0
              ? html`<div class="all-clear">Nobody has drained or put back a backend since the server started.</div>`
              : changes.map((e) => html`<div class="change"><span class="change-what">${changeText(e)}</span>
                  <span class="change-who">${e.user} · ${ago(e.at * 1000)}</span></div>`)}
          </section>
        </div>`}
    `;
  }

  /** A drained backend: what it still has to finish, or that it is safe to stop. */
  renderDrain(u, b) {
    const open = b.open_requests ?? 0;
    const since = b.drained_since ? ` Drained ${ago(b.drained_since * 1000)}.` : "";
    return html`<div class="banner" data-tone=${open ? "warn" : "ok"} role="status">
      <span class="banner-title"><span class="mono">${b.url}</span> ${open ? "is draining" : "is idle: safe to deploy"}</span>
      <span class="banner-text">${open
        ? `${plural(open, "request is", "requests are")} still running on it. New requests to ${u.name} go to its other backends.`
        : `No requests are open on it. Put it back when it is ready to serve ${u.name} again.`}${since}</span>
      <button class="btn" ?disabled=${this.busy === `drain:${b.url}`} @click=${() => this.drain(u.name, b.url, false)}>Put back</button>
    </div>`;
  }

  renderUpstream(u) {
    const serving = u.backends.filter((b) => b.healthy && !b.drained);
    const weight = serving.reduce((n, b) => n + b.weight, 0);
    return html`<section class="card" aria-labelledby="up-${u.name}">
      <div class="card-head"><h2 id="up-${u.name}" class="mono">${u.name}</h2>
        <span class="meta">${serving.length} of ${u.backends.length} serving</span></div>
      <div class="table-wrap"><table class="tbl tbl-stack" role="table">
        <thead role="rowgroup"><tr role="row"><th role="columnheader">Backend</th><th role="columnheader">Share of traffic</th><th role="columnheader">Health</th><th role="columnheader">Breaker</th><th role="columnheader" class="num two-line">Open<br>requests</th><th role="columnheader"><span class="sr">Rotation</span></th></tr></thead>
        <tbody role="rowgroup">${u.backends.map((b) => {
          const share = b.healthy && !b.drained && weight ? Math.round((b.weight / weight) * 100) : 0;
          return html`<tr role="row" class=${b.drained ? "is-warn" : ""}>
            <td role="cell" class="mono nowrap">${b.url}</td>
            <td role="cell" class="wide"><div class="share"><span class="share-track"><span class="share-fill" style="width: ${share}%"></span></span>
              <span class="share-text">${share}%</span>${b.drained ? status("warning", b.open_requests > 0 ? "Draining" : "Drained") : html`<span class="share-note">weight ${b.weight}</span>`}</div></td>
            <td role="cell">${b.healthy ? status("ok", "Healthy") : status("error", "Failing checks")}</td>
            <td role="cell" data-label="Breaker">${b.breaker === null ? status("neutral", "None")
              : b.breaker === "closed" ? status("ok", "Closed")
              : b.breaker === "half_open" ? status("warning", "Probing")
              : status("error", "Open")}</td>
            <td role="cell" class="num" data-label="Open requests">${(b.open_requests ?? 0).toLocaleString()}</td>
            <td role="cell" class="act"><button class="btn" aria-label="${b.drained ? "Put back" : "Drain"} ${b.url}" ?disabled=${this.busy === `drain:${b.url}`}
                @click=${() => this.drain(u.name, b.url, !b.drained)}>${b.drained ? "Put back" : "Drain"}</button></td>
          </tr>`;
        })}</tbody>
      </table></div>
    </section>`;
  }

  // ── rendering: Certificates ───────────────────────────────────────────
  renderCertificates() {
    const c = this.data.certificates;
    if (!c) return html`${this.head("Certificates")}${this.skeleton(2)}`;
    const acme = c.acme;
    const days = (x) => (x.days_left < 7 ? status("error", String(x.days_left)) : x.days_left < 21 ? status("warning", String(x.days_left)) : x.days_left);
    return html`
      ${this.head("Certificates", "Every certificate this server presents, soonest expiry first.")}
      <section class="card" aria-labelledby="certs-title">
        <div class="card-head"><h2 id="certs-title">Presented</h2>${c.certificates.length ? html`<span class="count">${c.certificates.length}</span>` : nothing}</div>
        ${c.certificates.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`
          : html`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>Name</th><th>Expires</th><th class="num">Days left</th><th>Source</th></tr></thead>
              <tbody>${c.certificates.map((x) => html`<tr><td class="mono">${x.name}</td><td>${x.expires}</td><td class="num">${days(x)}</td>
                <td class="dim">${x.source === "on_demand" ? "On demand" : "Configured"}</td></tr>`)}</tbody>
            </table></div>`}
      </section>
      <section class="card" aria-labelledby="acme-title">
        <div class="card-head"><h2 id="acme-title">ACME</h2>${acme.running ? status("ok", "Renews by itself") : status("neutral", "Off")}</div>
        <div class="card-body">
          ${acme.running
            ? html`<p>${acme.last_check ? html`Last check ${new Date(acme.last_check.at * 1000).toLocaleString()}: ${acme.last_check.outcome}.` : "No check has finished yet."}</p>
                <div class="row"><button class="btn" ?disabled=${this.busy === "renew"} @click=${() => this.renew()}><span class="icon">${icon("arrow-clockwise")}</span>Renew now</button></div>`
            : html`<p>Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</p>`}
        </div>
      </section>
    `;
  }

  // ── rendering: Controls ───────────────────────────────────────────────
  renderControls() {
    const d = this.data;
    if (!d.maintenance || !d.blocklist || !d.flags) return html`${this.head("Controls")}${this.skeleton(3)}`;
    const on = d.maintenance.active;
    const caches = d.caches ?? [];
    return html`
      ${this.head("Controls", "Things that change what visitors get, right now. They last until the server restarts.")}
      <div class="trio">
        <section class="card" aria-labelledby="maint-title">
          <div class="card-head"><h2 id="maint-title">Maintenance mode</h2></div>
          <div class="card-body">
            <p>${on ? status("warning", "On: 503 for visitors") : "Off. Visitors reach every host as usual."}</p>
            ${on ? html`<p class="hint">${d.maintenance.until
                ? html`Turns off by itself at ${hourMinute(d.maintenance.until * 1000)}, <rws-left .at=${d.maintenance.until * 1000}></rws-left>.`
                : "Stays on until someone turns it off."}
              ${d.maintenance.retry_after_secs ? ` Clients are told to retry after ${span(d.maintenance.retry_after_secs)}.` : " Clients get no hint when to retry."}</p>` : nothing}
            <div class="row"><button id="maintenance-toggle" class="btn ${on ? "" : "btn-warn"}" ?disabled=${this.busy === "maintenance"} @click=${() => this.setMaintenance(!on)}>${on ? "Turn off" : "Turn on"}</button>
              ${on && d.maintenance.until ? html`<button class="btn" ?disabled=${this.busy === "maintenance-extend"} @click=${() => this.extendMaintenance()}>Add 1 hour</button>` : nothing}</div>
          </div>
        </section>
        <section class="card" aria-labelledby="reload-title">
          <div class="card-head"><h2 id="reload-title">Configuration</h2></div>
          <div class="card-body">
            <p>Re-read <code>rws.config.toml</code> and the certificates. A file with errors is refused and the running configuration stays.</p>
            <div class="row"><button class="btn" ?disabled=${this.busy === "reload"} @click=${() => this.reload()}><span class="icon">${icon("arrow-clockwise")}</span>Reload</button></div>
          </div>
        </section>
        <section class="card" aria-labelledby="limit-title">
          <div class="card-head"><h2 id="limit-title">Global rate limit</h2></div>
          <div class="card-body">
            ${d.rateLimit?.configured
              ? html`<span class="big-value">${d.rateLimit.max_requests.toLocaleString()} per ${d.rateLimit.window_secs} s</span><p>Requests from one address above that get 429.</p>`
              : html`<span class="big-value">Off</span><p>Set <code>rate_limit</code> in the config to cap requests per address.</p>`}
          </div>
        </section>
      </div>

      <section class="card" aria-labelledby="caches-title">
        <div class="card-head"><h2 id="caches-title">Response caches</h2><span class="spacer"></span>
          ${caches.length > 1 ? html`<button class="btn btn-danger" ?disabled=${this.busy === "purge-every" || !caches.some((c) => c.entries)} @click=${() => this.purgeEvery()}>Purge every cache</button>` : nothing}</div>
        ${caches.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`
          : html`<div class="table-wrap"><table class="tbl tbl-stack" role="table">
              <thead role="rowgroup"><tr role="row"><th role="columnheader">Cache</th><th role="columnheader">Kept in</th><th role="columnheader" class="num">Entries</th><th role="columnheader" class="num">On disk</th><th role="columnheader">Served from cache</th><th role="columnheader"><span class="sr">Purge</span></th></tr></thead>
              <tbody role="rowgroup">${caches.map((c) => {
                const asked = c.hits + c.misses;
                const hit = asked ? Math.round((c.hits / asked) * 100) : 0;
                return html`<tr role="row">
                  <td role="cell" class="mono">${c.name}</td>
                  <td role="cell" class="dim" data-label="Kept in">${c.kind}</td>
                  <td role="cell" class="num" data-label="Entries">${c.entries.toLocaleString()}</td>
                  <td role="cell" class="num" data-label="On disk">${c.bytes === null ? "–" : bytes(c.bytes)}</td>
                  <td role="cell" class="wide">${asked
                    ? html`<div class="share" title="${c.hits.toLocaleString()} hits, ${c.misses.toLocaleString()} misses"><span class="share-track"><span class="share-fill" data-tone="ok" style="width: ${hit}%"></span></span><span class="share-text">${hit}%</span></div>`
                    : html`<span class="dim">no requests yet</span>`}</td>
                  <td role="cell" class="act"><button class="btn btn-danger" aria-label="Purge all of ${c.name}" ?disabled=${this.busy === `purge:${c.name}` || !c.entries} @click=${() => this.purgeAll(c.name, c.entries)}>Purge all</button></td>
                </tr>`;
              })}</tbody>
            </table></div>
            <div class="card-body">
              ${caches.map((c) => html`<form class="field-row purge" @submit=${(e) => this.purgePath(e, c.name)} novalidate>
                <lit-material-text-field variant="outlined" label="Path to purge from ${c.name}" supporting-text="One path, or a prefix ending in *: /products/*"
                  ?error=${Boolean(this.fieldError[`purge:${c.name}`])} error-text=${this.fieldError[`purge:${c.name}`] ?? ""}></lit-material-text-field>
                <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy === `purge:${c.name}`}>Purge path</lit-material-button>
              </form>`)}
            </div>`}
      </section>

      <div class="cols">
        <section class="card" aria-labelledby="block-title">
          <div class="card-head"><h2 id="block-title">Blocked addresses</h2>${d.blocklist.length ? html`<span class="count">${d.blocklist.length}</span>` : nothing}</div>
          <div class="card-body">
            <form class="field-row" @submit=${(e) => this.block(e)} novalidate>
              <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"
                ?error=${Boolean(this.fieldError.block)} error-text=${this.fieldError.block ?? ""} @input=${() => this.fieldError.block && (this.fieldError = { ...this.fieldError, block: "" })}></lit-material-text-field>
              <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy === "block"}><span slot="icon" class="icon">${icon("plus")}</span>Block</lit-material-button>
            </form>
            ${d.blocklist.length === 0 ? html`<p>Nobody is blocked. A blocked address gets 403 on every request.</p>` : nothing}
          </div>
          ${d.blocklist.length === 0 ? nothing : html`<div class="table-wrap"><table class="tbl">
            <thead><tr><th>Address</th><th><span class="sr">Unblock</span></th></tr></thead>
            <tbody>${d.blocklist.map((ip) => html`<tr><td class="mono">${ip}</td>
              <td class="act"><button class="btn" aria-label="Unblock ${ip}" ?disabled=${this.busy === `unblock:${ip}`} @click=${() => this.unblock(ip)}>Unblock</button></td></tr>`)}</tbody>
          </table></div>`}
        </section>
        <section class="card" aria-labelledby="flags-title">
          <div class="card-head"><h2 id="flags-title">Feature flags</h2></div>
          ${d.flags.length === 0
            ? html`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`
            : html`<div class="table-wrap"><table class="tbl">
                <thead><tr><th>Flag</th><th><span class="sr">Enabled</span></th></tr></thead>
                <tbody>${d.flags.map((f) => html`<tr><td class="mono">${f.name}</td>
                  <td class="act"><lit-material-switch aria-label="${f.name}" .checked=${f.enabled}
                    @change=${(e) => this.act(`flag:${f.name}`, `/flags/${encodeURIComponent(f.name)}`, { method: "PUT", body: { enabled: e.target.checked } }, (r) => `${r.name} ${r.enabled ? "on" : "off"}`)}></lit-material-switch></td></tr>`)}</tbody>
              </table></div>`}
        </section>
      </div>

      ${d.authz ? this.renderAuthz(d.authz, d.authzCheck) : nothing}
    `;
  }

  renderAuthz(authz, check) {
    const types = Object.entries(authz.schema.types).filter(([, relations]) => relations.length).map(([type, relations]) => `${type}: ${relations.join(", ")}`);
    return html`
      <section class="card" aria-labelledby="authz-title">
        <div class="card-head"><h2 id="authz-title">Authorization</h2><span class="meta">Relationship tuples, object#relation@subject. Schema: ${types.join(" · ")}.</span></div>
        <div class="card-body">
          <form class="field-row" @submit=${(e) => this.addTuple(e)} novalidate>
            <lit-material-text-field id="authz-tuple" variant="outlined" label="Tuple" supporting-text="e.g. doc:42#viewer@user:alice"></lit-material-text-field>
            <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy === "authz-add"}><span slot="icon" class="icon">${icon("plus")}</span>Add</lit-material-button>
          </form>
          ${authz.tuples.length === 0 ? html`<p>No tuples yet.</p>` : nothing}
        </div>
        ${authz.tuples.length === 0 ? nothing : html`<div class="table-wrap"><table class="tbl">
          <thead><tr><th>Tuple</th><th><span class="sr">Remove</span></th></tr></thead>
          <tbody>${authz.tuples.map((tuple) => html`<tr><td class="mono">${tuple}</td>
            <td class="act"><button class="btn" aria-label="Remove ${tuple}" ?disabled=${this.busy === `authz-del:${tuple}`}
              @click=${() => this.act(`authz-del:${tuple}`, `/authz/tuples?tuple=${encodeURIComponent(tuple)}`, { method: "DELETE" }, () => `${tuple} removed`)}>Remove</button></td></tr>`)}</tbody>
        </table></div>`}
        <div class="card-body">
          <form class="field-row" @submit=${(e) => this.checkAccess(e)} novalidate>
            <lit-material-text-field id="authz-subject" variant="outlined" label="Subject" supporting-text="user:alice"></lit-material-text-field>
            <lit-material-text-field id="authz-relation" variant="outlined" label="Relation" supporting-text="viewer"></lit-material-text-field>
            <lit-material-text-field id="authz-object" variant="outlined" label="Object" supporting-text="doc:42"></lit-material-text-field>
            <lit-material-button type="submit" variant="tonal">Check</lit-material-button>
          </form>
          ${check
            ? html`<p class="result" id="authz-result">
                ${check.allowed ? status("ok", "Allowed") : status("error", "Denied")}
                <span>${check.subject} ${check.relation} ${check.object}${check.allowed ? html`, through <code>${check.path.join(" → ")}</code>` : nothing}</span></p>`
            : nothing}
        </div>
      </section>
    `;
  }

  // ── rendering: Config, Audit, Logs ────────────────────────────────────
  renderConfig() {
    const config = this.data.config;
    if (!config) return html`${this.head("Config")}${this.skeleton(2)}`;
    const needle = this.filter.toLowerCase();
    const all = Object.entries(config);
    const rows = all.filter(([k, v]) => !needle || k.toLowerCase().includes(needle) || v.toLowerCase().includes(needle));
    return html`
      ${this.head("Config", this.file?.editable
        ? html`Edit the file, see what changes, then apply: rws checks it, keeps the previous version and reloads. Below it, every setting in effect.`
        : html`Every setting in effect. Secrets read (hidden). Change them in <code>rws.config.toml</code> or the environment, then reload from Controls.`)}
      ${this.renderFile()}
      <section class="card" aria-labelledby="config-title">
        <div class="card-head"><h2 id="config-title">Settings in effect</h2><span class="count">${needle ? `${rows.length} of ${all.length}` : all.length}</span></div>
        <div class="card-body">
          <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${(e) => (this.filter = e.target.value)}></lit-material-text-field>
        </div>
        ${rows.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${all.length}.</lit-material-empty-state>`
          : html`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>Setting</th><th>Value</th></tr></thead>
              <tbody>${rows.map(([setting, value]) => html`<tr><td class="mono">${setting}</td>
                <td class="mono">${value === "" ? status("neutral", "empty") : html`<lit-material-truncate position="middle" style="max-width: min(60ch, 50vw)" .value=${value}></lit-material-truncate>`}</td></tr>`)}</tbody>
            </table></div>`}
      </section>
    `;
  }

  /** The Config page's file card: for admins, an editor when `[admin] edit_config` is on. */
  renderFile() {
    const file = this.file;
    if (!file || this.me?.role !== "admin") return nothing;
    if (!file.enabled) {
      return html`<section class="card cfg" aria-labelledby="file-title">
        <div class="card-head"><h2 id="file-title">Config file</h2>${file.path ? html`<span class="cfg-path">${file.path}</span>` : nothing}</div>
        <div class="card-body"><p>Editing it from here is off. Add <code>edit_config = true</code> under <code>[admin]</code> and reload: then you can edit the file on this page,
          see what changes and apply it. Whoever holds an admin token can then change every setting, so it is yours to turn on.</p></div>
      </section>`;
    }
    if (!file.editable) {
      return html`<section class="card cfg" aria-labelledby="file-title">
        <div class="card-head"><h2 id="file-title">Config file</h2>${file.path ? html`<span class="cfg-path">${file.path}</span>` : nothing}</div>
        <div class="card-body"><p>It can't be edited from here: ${file.reason}.</p></div>
      </section>`;
    }
    const text = this.draft ?? file.content;
    const staged = this.draft !== null;
    const diff = staged ? this.fileDiff : null;
    const check = this.fileCheck;
    const checked = check?.state === "done" ? check : null;
    const applying = this.busy === "config-file";
    const count = text.split("\n").length;
    const parts = diff ? hunks(diff.lines, 3) : [];
    let drawn = 0;
    return html`<section class="card cfg" aria-labelledby="file-title">
      <div class="card-head"><h2 id="file-title">Config file</h2><span class="cfg-path">${file.path}</span><span class="spacer"></span>
        <span class="meta">${staged ? "Edited, not applied" : file.modified ? `Saved ${ago(file.modified * 1000)}` : "As on disk"}</span></div>
      ${this.fileNewer
        ? html`<div class="banner" role="alert">
            <span class="banner-title">The file changed on disk</span>
            <span class="banner-text">Somebody wrote it after you started. Compare your edits with the new file before applying: what it gained will show as removed lines.</span>
            <button class="btn btn-warn" @click=${() => this.useNewerFile()}>Compare with the new file</button>
          </div>`
        : nothing}
      ${this.applied?.restart?.length && !staged
        ? html`<div class="banner" role="status">
            <span class="banner-title">Applied. Restart rws for the rest</span>
            <span class="banner-text">A reload doesn't apply ${this.applied.restart.join(", ")}: ${this.applied.restart.length === 1 ? "it takes" : "they take"} effect when rws starts again.</span>
          </div>`
        : nothing}
      <div class="editor">
        <pre class="editor-gutter" aria-hidden="true">${Array.from({ length: count }, (_, i) => i + 1).join("\n")}</pre>
        <textarea id="file-text" wrap="off" spellcheck="false" autocomplete="off" autocapitalize="off" autocorrect="off"
          aria-label="Contents of ${file.path}" aria-describedby="file-check" ?readonly=${applying}
          .value=${live(text)}
          @input=${(e) => this.editFile(e.target.value)}
          @scroll=${(e) => { const gutter = e.target.previousElementSibling; if (gutter) gutter.scrollTop = e.target.scrollTop; }}
          @keydown=${(e) => { if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "s") { e.preventDefault(); this.applyFile(); } }}></textarea>
      </div>
      ${diff
        ? html`<div class="diff" role="region" aria-labelledby="diff-title">
            <div class="diff-head"><h3 id="diff-title">What will change</h3>
              <span class="mono">${plural(diff.added, "line", "lines")} added, ${diff.removed.toLocaleString()} removed</span></div>
            <div class="diff-body" tabindex="0" aria-label="Changed lines">
              ${diff.added + diff.removed === 0
                ? html`<div class="diff-gap">Only the newline that ends the file differs.</div>`
                : parts.map((part, at) => {
                    if (drawn >= DIFF_DRAWN) return nothing;
                    const shown = part.slice(0, DIFF_DRAWN - drawn);
                    drawn += shown.length;
                    return html`${at ? html`<div class="diff-gap" aria-hidden="true">⋯</div>` : nothing}${shown.map((l) => html`<div class="diff-line" data-op=${l.op}>
                      <span class="n">${l.a || ""}</span><span class="n n-new">${l.b || ""}</span><span class="sign">${l.op === " " ? "" : l.op === "+" ? "+" : "−"}</span><span class="sr">${l.op === "+" ? "added: " : l.op === "-" ? "removed: " : ""}</span><span class="t">${l.text}</span></div>`)}`;
                  })}
              ${drawn >= DIFF_DRAWN ? html`<div class="diff-gap">The first ${DIFF_DRAWN} lines are shown; the rest is applied too.</div>` : nothing}
            </div>
          </div>`
        : nothing}
      <div class="cfg-check" id="file-check" role="status">
        ${!staged ? nothing
          : check?.state === "checking" ? html`<p class="result"><span class="sq"></span>Checking…</p>`
          : check?.state === "failed" ? html`<p class="result"><span class="sq" data-tone="warn"></span><span>Couldn't check it (${check.message}). rws checks again when you apply.</span></p>`
          : checked?.ok === false ? html`<p class="result"><span class="sq" data-tone="bad"></span><span>This doesn't pass <code>rws --check</code>, so it can't be applied:</span></p>
              <ul>${checked.errors.map((e) => html`<li>${e}</li>`)}</ul>`
          : checked ? html`<p class="result"><span class="sq" data-tone=${checked.restart.length ? "warn" : "ok"}></span>${checked.restart.length
                ? html`<span>Passes <code>rws --check</code>. The rest applies with a reload, but these need a restart: <span class="mono">${checked.restart.join(", ")}</span></span>`
                : html`<span>Passes <code>rws --check</code>. Applies with a reload; open connections are kept.</span>`}</p>
              ${checked.warnings.length ? html`<ul>${checked.warnings.map((w) => html`<li>${w}</li>`)}</ul>` : nothing}`
          : nothing}
        ${this.fileError ? html`<p class="result" role="alert"><span class="sq" data-tone="bad"></span><span>Not applied: ${this.fileError}</span></p>` : nothing}
      </div>
      ${staged
        ? html`<div class="banner cfg-staged">
            <span class="banner-title">Edits staged</span>
            <span class="banner-text">Nothing is applied yet: rws still runs the file as it is on disk.</span>
            <button class="btn btn-quiet" ?disabled=${applying} @click=${() => this.discardFile()}>Discard</button>
            <button class="btn btn-warn-fill" ?disabled=${applying || checked?.ok === false || !!this.fileNewer} @click=${() => this.applyFile()}>${applying ? "Applying…" : "Apply"}</button>
          </div>`
        : nothing}
    </section>`;
  }

  renderAudit() {
    const entries = this.data.audit;
    if (!entries) return html`${this.head("Audit")}${this.skeleton(2)}`;
    const result = (e) => (e.status < 300 ? status("ok", "Done") : e.status === 401 || e.status === 403 ? status("error", "Refused") : status("warning", String(e.status)));
    return html`
      ${this.head("Audit", html`Every change made through the admin API, this console or MCP tools, and every refused attempt, newest first. Kept in memory (the last 1000); <code>[admin] audit_log</code> also writes them to a file.`)}
      <section class="card" aria-labelledby="audit-title">
        <div class="card-head"><h2 id="audit-title">Changes</h2>${entries.length ? html`<span class="count">${entries.length}</span>` : nothing}</div>
        ${entries.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No changes yet">Changes appear here as people reload, drain, block or purge.</lit-material-empty-state>`
          : html`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>When</th><th>Who</th><th>Change</th><th>Details</th><th>Result</th><th>From</th></tr></thead>
              <tbody>${entries.map((e) => html`<tr>
                <td class="dim" style="white-space: nowrap">${new Date(e.at * 1000).toLocaleString()}</td>
                <td>${e.user} <span class="meta">${e.role}${e.via === "oidc" ? " · SSO" : e.via === "mcp" ? " · MCP" : ""}</span></td>
                <td class="mono">${e.method === "TOOL" ? "tool" : e.method} ${e.path.replace("/admin/api/v1", "")}</td>
                <td class="mono">${e.detail ? html`<lit-material-truncate style="max-width: min(40ch, 30vw)" .value=${e.detail}></lit-material-truncate>` : nothing}</td>
                <td>${result(e)}</td>
                <td class="mono dim">${e.address}</td>
              </tr>`)}</tbody>
            </table></div>`}
      </section>
    `;
  }

  renderLogs() {
    const f = this.logFilter;
    const chips = [["all", "All"], ["2xx", "2xx"], ["3xx", "3xx"], ["4xx", "4xx"], ["5xx", "5xx"]];
    const kept = this.paused ? this.logFrozen : this.logAll;
    const filtered = f.text || f.status !== "all";
    const shown = filtered ? kept.filter((r) => this.logShown(r)) : kept;
    const drawn = shown.slice(-LOG_DRAWN).reverse();
    const waiting = this.paused ? this.logAll.length - this.logFrozen.length : 0;
    return html`
      ${this.head("Logs", "One request per row, newest first. This console's own requests aren't logged.")}
      <div class="row log-tools">
        <lit-material-text-field class="log-filter" variant="outlined" label="Filter by path, host, address…" .value=${f.text} @input=${(e) => this.setLogFilter({ text: e.target.value })}></lit-material-text-field>
        <div class="seg" role="group" aria-label="Status">
          ${chips.map(([id, label]) => html`<button aria-pressed=${f.status === id} @click=${() => this.setLogFilter({ status: id })}>${label}</button>`)}
        </div>
        <span class="spacer"></span>
        <button class="btn" aria-pressed=${!this.paused} @click=${() => this.setPaused(!this.paused)}>
          <span class="live" data-on=${!this.paused}><span class="dot" aria-hidden="true"></span>${this.paused ? "Paused" : "Following"}</span></button>
        <button class="btn" ?disabled=${shown.length === 0} @click=${() => this.downloadLog()}><span class="icon">${icon("download-simple")}</span>Download</button>
      </div>
      <section class="card" aria-label="Access log">
        <div class="reqs" role="table" aria-label="Access log">
          ${requestHead}
          ${drawn.length === 0
            ? html`<div class="req-none">${filtered && kept.length ? "No request matches the filter." : "No requests yet. They appear here as they arrive."}</div>`
            : drawn.map(requestRow)}
        </div>
        <div class="card-foot">${this.paused
          ? html`Paused${waiting > 0 ? html`: ${plural(waiting, "new request", "new requests")} waiting.` : "."} Follow again to see new requests.`
          : html`Showing ${drawn.length.toLocaleString()}${shown.length > drawn.length ? html` of ${shown.length.toLocaleString()}` : nothing}${filtered ? " that match" : ""}. The last ${LOG_KEPT.toLocaleString()} are kept here; Download saves them.`}</div>
      </section>
    `;
  }
}

customElements.define("rws-admin", RwsAdmin);
