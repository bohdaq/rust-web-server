import { LitElement, html, nothing } from "lit";
import { api, ApiError, followLogs, setToken, token } from "./api.js";
import { formatBytes as bytes, formatDuration } from "@lit-material/format";
import "@lit-material/truncate";
import { icon } from "./icons.js";

const VIEWS = [
  { id: "overview", label: "Overview", icon: "gauge" },
  { id: "traffic", label: "Traffic", icon: "chart-line" },
  { id: "upstreams", label: "Upstreams", icon: "tree-structure" },
  { id: "certificates", label: "Certificates", icon: "certificate" },
  { id: "controls", label: "Controls", icon: "sliders-horizontal" },
  { id: "config", label: "Config", icon: "list-bullets" },
  { id: "logs", label: "Logs", icon: "terminal-window" },
  { id: "audit", label: "Audit", icon: "clipboard-text" },
];

// The phone's bottom bar: these four, then "More" for the rest.
const PRIMARY = ["overview", "traffic", "upstreams", "controls"];
const NARROW = "(max-width: 599.98px)";

const SAMPLES = 60;

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
const time = (ms) => new Date(ms).toLocaleTimeString();
const logLevel = (line) => (/(?:"status":|\s)5\d\d\s/.test(` ${line} `) ? "error" : /(?:"status":|\s)4\d\d\s/.test(` ${line} `) ? "warning" : null);

/** A JSON access-log line as columns: time, status, method, path, duration, client. */
function logLine(raw) {
  let j;
  try {
    j = JSON.parse(raw);
  } catch {
    return raw;
  }
  if (!j || typeof j !== "object" || j.status === undefined) return raw;
  const when = j.time ?? j.timestamp ?? j.ts ?? "";
  const clock = typeof when === "string" && when.includes("T") ? when.slice(11, 19) : String(when);
  const path = j.path ?? j.uri ?? j.request_uri ?? "";
  const ms = j.duration_ms ?? j.ms ?? (j.duration_us !== undefined ? Math.round(j.duration_us / 1000) : undefined);
  const size = typeof j.bytes === "number" ? bytes(j.bytes) : "";
  return [clock, String(j.status), (j.method ?? "").padEnd(6), `${j.host ? j.host : ""}${path}`, ms !== undefined ? `${ms} ms` : "", size, j.remote_addr ?? ""].filter((x) => x !== "").join("  ");
}

const BACKEND_COLUMNS = (self, upstream) => [
  { key: "url", label: "Backend", mono: true },
  { key: "weight", label: "Weight", numeric: true },
  { key: "healthy", label: "Health", render: (b) => (b.healthy ? status("ok", "Healthy") : status("error", "Failing checks")) },
  {
    key: "breaker",
    label: "Breaker",
    render: (b) =>
      b.breaker === null ? status("neutral", "None")
      : b.breaker === "closed" ? status("ok", "Closed")
      : b.breaker === "half_open" ? status("warning", "Probing")
      : status("error", "Open"),
  },
  {
    key: "rotation",
    label: "In rotation",
    render: (b) => html`<div class="row">
      <lit-material-switch aria-label="${b.url} in rotation" .checked=${!b.drained} ?disabled=${self.busy === `drain:${b.url}`}
        @change=${(e) => self.drain(upstream, b.url, !e.target.checked, e.target)}></lit-material-switch>
      ${b.drained ? status("warning", "Drained") : nothing}
    </div>`,
  },
];

const CERT_COLUMNS = [
  { key: "name", label: "Name", mono: true },
  { key: "expires", label: "Expires" },
  {
    key: "days_left",
    label: "Days left",
    numeric: true,
    render: (x) => (x.days_left < 7 ? status("error", String(x.days_left)) : x.days_left < 21 ? status("warning", String(x.days_left)) : x.days_left),
  },
  { key: "source", label: "Source", render: (x) => (x.source === "on_demand" ? "On demand" : "Configured") },
];

/** The whole app, rendered into the light DOM so the theme's classes apply. */
export class RwsAdmin extends LitElement {
  static properties = {
    authed: { state: true },
    view: { state: true },
    loginError: { state: true },
    data: { state: true },
    error: { state: true },
    samples: { state: true },
    paused: { state: true },
    filter: { state: true },
    busy: { state: true },
    me: { state: true },
    sso: { state: true },
    narrow: { state: true },
    moreOpen: { state: true },
    conn: { state: true },
    notice: { state: true },
    ask: { state: true },
    fieldError: { state: true },
  };

  constructor() {
    super();
    this.authed = false;
    this.view = viewFromHash();
    this.data = {};
    this.samples = [];
    this.paused = false;
    this.filter = "";
    this.busy = "";
    this.lastTotal = null;
    this.me = null;
    this.sso = false;
    this.narrow = matchMedia(NARROW).matches;
    this.moreOpen = false;
    // The admin API's reachability: lost = the last poll failed; at = the last answer.
    this.conn = { lost: false, at: 0 };
    this.notice = null;
    this.ask = null;
    this.fieldError = {};
  }

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.onHash = () => this.show(viewFromHash(), { push: false });
    addEventListener("hashchange", this.onHash);
    this.media = matchMedia(NARROW);
    this.onMedia = () => (this.narrow = this.media.matches);
    this.media.addEventListener("change", this.onMedia);
    // A token in this tab, or an OIDC sign-in cookie: try either.
    this.check({ quiet: !token() });
    api("/auth").then((m) => (this.sso = Boolean(m?.oidc))).catch(() => {});
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    removeEventListener("hashchange", this.onHash);
    this.media?.removeEventListener("change", this.onMedia);
    this.stop();
    clearInterval(this.pulse);
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
   * (healthy backends), so the header tells the truth wherever you are,
   * and a lost connection shows with the time of the last answer.
   */
  heartbeat() {
    clearInterval(this.pulse);
    const beat = async () => {
      if (!this.authed) return;
      try {
        const [info, ups] = await Promise.all([api("/status"), api("/upstreams").catch(() => this.data.upstreams ?? [])]);
        this.data = { ...this.data, status: info, upstreams: ups };
        this.conn = { lost: false, at: Date.now() };
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
    clearInterval(this.pulse);
    if (viaSso) location.assign("/admin/auth/logout");
  }

  /** An API call from a view: errors land in the page, 401 signs out. */
  async call(path, options) {
    try {
      const result = await api(path, options);
      this.error = "";
      if (this.conn.lost) this.conn = { lost: false, at: Date.now() };
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
  toast(message, undo = null) {
    clearTimeout(this.noticeTimer);
    this.notice = { message, undo };
    this.noticeTimer = setTimeout(() => (this.notice = null), undo ? 8000 : 4000);
  }

  /** A confirmation dialog; resolves true when confirmed. */
  confirm({ title, body, action, danger = true }) {
    return new Promise((resolve) => {
      this.ask = { title, body, action, danger, resolve };
      this.updateComplete.then(() => this.querySelector("#confirm")?.showModal());
    });
  }

  answer(yes) {
    const ask = this.ask;
    this.querySelector("#confirm")?.close();
    this.ask = null;
    ask?.resolve(yes);
  }

  // ── views and their polling ───────────────────────────────────────────
  show(view, { push = true } = {}) {
    this.stop();
    this.view = view;
    this.moreOpen = false;
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
    if (view === "logs") this.updateComplete.then(() => this.follow());
    queueMicrotask(() => this.querySelector("main")?.focus({ preventScroll: true }));
  }

  stop() {
    clearInterval(this.timer);
    this.timer = undefined;
    this.abort?.abort();
    this.abort = undefined;
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
    if (view === "overview") {
      await Promise.all([get("status", "/status"), get("metrics", "/metrics")]);
      const total = this.data.metrics?.requests_total;
      if (typeof total === "number") {
        const now = Date.now();
        if (this.lastTotal) {
          const rate = Math.max(0, (total - this.lastTotal.total) / ((now - this.lastTotal.at) / 1000));
          this.samples = [...this.samples, { x: now, y: Math.round(rate * 100) / 100 }].slice(-SAMPLES);
        }
        this.lastTotal = { total, at: now };
      }
    } else if (view === "upstreams") {
      await get("upstreams", "/upstreams");
    } else if (view === "certificates") {
      await get("certificates", "/certificates");
    } else if (view === "controls") {
      await Promise.all([get("maintenance", "/maintenance"), get("blocklist", "/blocklist"), get("flags", "/flags"), get("rateLimit", "/rate-limit"), get("caches", "/caches"), this.loadAuthz()]);
    } else if (view === "config") {
      await get("config", "/config");
    } else if (view === "audit") {
      await get("audit", "/audit?n=200");
    }
  }

  async follow() {
    this.abort = new AbortController();
    const log = this.querySelector("lit-material-log-viewer");
    if (!log) return;
    log.highlight = logLevel;
    this.logAll = [];
    const keep = (lines) => {
      const list = (Array.isArray(lines) ? lines : [lines]).map(logLine);
      this.logAll.push(...list);
      if (this.logAll.length > 1000) this.logAll.splice(0, this.logAll.length - 1000);
      return list.filter((l) => this.logShown(l));
    };
    try {
      log.lines = keep(await this.call("/logs?n=200"));
      await followLogs((line) => {
        const shown = keep(line);
        if (shown.length) log.appendLines(shown);
      }, this.abort.signal);
    } catch (e) {
      if (e.name !== "AbortError" && !this.error) this.error = `The live log stopped: ${e.message}`;
    }
  }

  /** Whether a formatted log line passes the filter box and status chips. */
  logShown(line) {
    const f = this.logFilter ?? { text: "", status: "all" };
    if (f.status !== "all") {
      const code = /(?:^|\s)([1-5])\d\d(?:\s|$)/.exec(line)?.[1];
      if (code !== f.status[0]) return false;
    }
    return !f.text || line.toLowerCase().includes(f.text.toLowerCase());
  }

  setLogFilter(change) {
    this.logFilter = { text: "", status: "all", ...this.logFilter, ...change };
    const log = this.querySelector("lit-material-log-viewer");
    if (log) log.lines = (this.logAll ?? []).filter((l) => this.logShown(l));
    this.requestUpdate();
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

  async drain(upstream, backend, drained, control) {
    const backends = (this.data.upstreams ?? []).find((u) => u.name === upstream)?.backends ?? [];
    const serving = backends.filter((b) => !b.drained && b.healthy && b.url !== backend);
    if (drained && serving.length === 0) {
      const ok = await this.confirm({
        title: `Take ${backend} out of rotation?`,
        body: `It is the last healthy backend of ${upstream}: every request to ${upstream} will get 503 until one is back.`,
        action: "Drain anyway",
      });
      if (!ok) {
        if (control) control.checked = true;
        return false;
      }
    }
    return this.act(`drain:${backend}`, `/upstreams/${encodeURIComponent(upstream)}/drain`, { method: "PUT", body: { backend, drained } },
      () => (drained ? `${backend} drained: it gets no new requests` : `${backend} is back in rotation`),
      () => this.drain(upstream, backend, !drained));
  }

  async setMaintenance(active, control) {
    if (active) {
      const ok = await this.confirm({
        title: "Turn on maintenance mode?",
        body: "Every visitor gets 503 Service Unavailable (browsers see a maintenance page) until you turn it off. Health checks keep passing.",
        action: "Turn on",
      });
      if (!ok) {
        if (control) control.checked = false;
        return;
      }
    }
    await this.act("maintenance", "/maintenance", { method: "PUT", body: { active } }, (r) => (r.active ? "Maintenance on: visitors get 503" : "Maintenance off"));
    this.data = { ...this.data, status: { ...this.data.status, maintenance: active } };
  }

  reload() {
    return this.act("reload", "/reload", { method: "POST" }, (r) =>
      r.applied ? "Configuration reloaded" : `Reload kept the running configuration: ${r.errors.join("; ")}`);
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
    this.act("block", "/blocklist", { method: "POST", body: { ip } }, (r) => `${r.blocked} blocked`,
      () => this.act(`unblock:${ip}`, `/blocklist/${encodeURIComponent(ip)}`, { method: "DELETE" }, () => `${ip} unblocked`)).then((ok) => {
      if (ok && field) field.value = "";
    });
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
    this.act("authz-add", "/authz/tuples", { method: "POST", body: { tuple } }, (r) => (r.written ? `${r.tuple} added` : `${r.tuple} was already there`)).then(() => {
      if (!this.error && field) field.value = "";
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

  unblock(ip) {
    return this.act(`unblock:${ip}`, `/blocklist/${encodeURIComponent(ip)}`, { method: "DELETE" }, () => `${ip} unblocked`,
      () => this.act("block", "/blocklist", { method: "POST", body: { ip } }, () => `${ip} blocked again`));
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
      (r) => `${r.purged} ${r.purged === 1 ? "entry" : "entries"} purged from ${r.name}`);
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
      (r) => `${r.purged} ${r.purged === 1 ? "entry" : "entries"} purged from ${r.name}`);
  }

  // ── rendering ─────────────────────────────────────────────────────────
  /** Backends of every upstream: [healthy and in rotation, all]. */
  backendHealth() {
    const all = (this.data.upstreams ?? []).flatMap((u) => u.backends);
    return [all.filter((b) => b.healthy && !b.drained).length, all.length];
  }

  /** The one header chip: disconnected, maintenance, degraded, not ready or ready. */
  renderState() {
    const info = this.data.status;
    if (this.conn.lost) {
      const at = this.conn.at ? ` · last update ${time(this.conn.at)}` : "";
      return html`<span class="state" title="The admin API doesn't answer; retrying every 5 seconds">${status("error", `Disconnected${at}`)}</span>`;
    }
    if (!info) return nothing;
    if (info.maintenance) return html`<span class="state">${status("warning", "Maintenance")}</span>`;
    const [up, all] = this.backendHealth();
    if (all > 0 && up < all) return html`<span class="state" title="Healthy backends in rotation">${status(up === 0 ? "error" : "warning", `Degraded: ${up} of ${all} backends`)}</span>`;
    return html`<span class="state">${info.ready ? status("ok", "Ready") : status("error", "Not ready")}</span>`;
  }

  renderNav() {
    if (!this.narrow) {
      const destinations = VIEWS.map((v) => ({ label: v.label, icon: icon(v.icon) }));
      return html`<lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${destinations}
        .selected=${VIEWS.findIndex((v) => v.id === this.view)}
        @change=${(e) => this.show(VIEWS[e.target.selected].id)}></lit-material-adaptive-navigation>`;
    }
    // A phone: four sections and More (eight labels didn't fit: "Con…" twice).
    const primary = PRIMARY.map((id) => VIEWS.find((v) => v.id === id));
    const destinations = [...primary.map((v) => ({ label: v.label, icon: icon(v.icon) })), { label: "More", icon: icon("dots-three") }];
    const at = PRIMARY.indexOf(this.view);
    return html`<lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${destinations}
        .selected=${at >= 0 ? at : 4}
        @change=${(e) => (e.target.selected === 4 ? (this.moreOpen = true) : this.show(PRIMARY[e.target.selected]))}></lit-material-adaptive-navigation>
      ${this.moreOpen
        ? html`<div class="more-scrim" @click=${() => (this.moreOpen = false)}></div>
          <nav class="more-sheet" aria-label="More sections" @keydown=${(e) => e.key === "Escape" && (this.moreOpen = false)}>
            ${VIEWS.filter((v) => !PRIMARY.includes(v.id)).map((v) => html`<a href="#/${v.id}" aria-current=${this.view === v.id ? "page" : "false"}
              @click=${(e) => { e.preventDefault(); this.show(v.id); }}><span class="icon">${icon(v.icon)}</span>${v.label}</a>`)}
          </nav>`
        : nothing}`;
  }

  render() {
    if (!this.authed) return this.renderLogin();
    const info = this.data.status;
    return html`
      <div class="shell">
        <header class="top">
          <span class="brand">${brandMark}<span class="brand-text">rws admin</span></span>
          ${this.renderState()}
          <span class="spacer"></span>
          ${info ? html`<span class="meta">${info.version} · ${info.listeners?.main?.address ?? ""}</span>` : nothing}
          ${this.me ? html`<span class="meta" title="Signed in ${this.me.via === "oidc" ? "with single sign-on" : "with a token"}"><span class="icon">${icon("user-circle")}</span> ${this.me.user} · ${this.me.role}</span>` : nothing}
          <lit-material-button variant="text" @click=${() => this.logout()}><span slot="icon" class="icon">${icon("sign-out")}</span>Sign out</lit-material-button>
        </header>
        ${this.renderNav()}
        <main id="main" tabindex="-1">
          ${info?.maintenance
            ? html`<lit-material-alert variant="warning"><span slot="title">Maintenance mode is on</span>
                Visitors get 503 Service Unavailable.
                <lit-material-button variant="text" ?disabled=${this.busy === "maintenance"} @click=${() => this.setMaintenance(false)}>Turn off</lit-material-button></lit-material-alert>`
            : nothing}
          ${this.error ? html`<lit-material-alert variant="error"><span slot="title">Couldn't load this page</span>${this.error}
              <lit-material-button variant="text" @click=${() => this.load(this.view)}>Retry</lit-material-button></lit-material-alert>` : nothing}
          ${this.renderView()}
        </main>
      </div>
      <div class="toast" role="status" aria-live="polite">
        ${this.notice
          ? html`<div class="toast-body"><span>${this.notice.message}</span>
              ${this.notice.undo ? html`<button class="toast-action" @click=${() => { const undo = this.notice.undo; this.notice = null; undo(); }}>Undo</button>` : nothing}
              <button class="toast-close" aria-label="Dismiss" @click=${() => (this.notice = null)}>✕</button></div>`
          : nothing}
      </div>
      <dialog id="confirm" class="confirm" aria-labelledby="confirm-title" @cancel=${(e) => { e.preventDefault(); this.answer(false); }}>
        ${this.ask
          ? html`<h2 id="confirm-title">${this.ask.title}</h2>
            <p>${this.ask.body}</p>
            <div class="confirm-actions">
              <lit-material-button variant="text" autofocus @click=${() => this.answer(false)}>Cancel</lit-material-button>
              <lit-material-button class=${this.ask.danger ? "danger" : ""} @click=${() => this.answer(true)}>${this.ask.action}</lit-material-button>
            </div>`
          : nothing}
      </dialog>
    `;
  }

  renderLogin() {
    return html`
      <div class="login">
        <section class="login-card tile" aria-labelledby="login-title">
          <h1 id="login-title"><span class="icon">${icon("lock-key")}</span>Sign in to rws admin</h1>
          ${this.sso ? html`<lit-material-button href="/admin/auth/login" style="width: 100%">Sign in with single sign-on</lit-material-button>
            <p class="lede" style="text-align: center">or with a token</p>` : nothing}
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

  skeleton(rows = 3) {
    return html`<div class="grid" aria-busy="true">${Array.from({ length: rows }, () => html`<div class="tile"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`;
  }

  renderOverview() {
    const s = this.data.status, m = this.data.metrics;
    if (!s || !m) return html`<h1>Overview</h1>${this.skeleton(4)}`;
    const errorRate = m.requests_total ? ((m.errors_total / m.requests_total) * 100).toFixed(2) : "0.00";
    const rates = this.samples.map((p) => p.y);
    return html`
      <h1>Overview</h1>
      <p class="lede">Up ${duration(s.uptime_secs)}, process ${s.pid}. Counters refresh every 2 seconds; the admin listener's own requests aren't counted.</p>
      ${(s.disk ?? []).filter((d) => d.low).map((d) => html`<lit-material-alert variant="warning" style="margin-bottom: var(--space-200)">
        <span slot="title">Disk almost full</span>
        ${d.dir} has ${bytes(d.free_bytes)} free of ${bytes(d.total_bytes)}: rws has stopped writing the access log and disk cache there until space is freed.</lit-material-alert>`)}
      <div class="grid">
        <lit-material-stat label="Requests served" value=${m.requests_total.toLocaleString()}>
          ${rates.length > 1 ? html`<lit-material-sparkline slot="caption" label="Requests per second" .values=${rates}></lit-material-sparkline>` : nothing}
        </lit-material-stat>
        <lit-material-stat label="Errors" value=${m.errors_total.toLocaleString()} caption="${errorRate}% of requests"></lit-material-stat>
        <lit-material-stat label="Open connections" value=${String(m.active_connections)}></lit-material-stat>
        <lit-material-stat label="Queued for a worker" value=${String(m.thread_pool_queued)}></lit-material-stat>
        ${this.renderHealthTile()}
      </div>
      <section class="section" aria-labelledby="rate-title">
        <h2 id="rate-title">Requests per second</h2>
        ${this.samples.length < 2
          ? html`<lit-material-empty-state size="compact" headline="Collecting samples">The chart fills in as this page stays open.</lit-material-empty-state>`
          : html`<lit-material-line-chart label="Requests per second, last ${Math.round((this.samples.at(-1).x - this.samples[0].x) / 1000)} seconds"
              unit="req/s" height="200" .formatX=${time} .series=${[{ name: "Requests per second", points: this.samples }]}></lit-material-line-chart>`}
      </section>
      <section class="section" aria-labelledby="listen-title">
        <h2 id="listen-title">Listeners</h2>
        <div class="grid">
          <lit-material-stat class="mono" label="Main" value=${s.listeners.main.address} caption=${s.listeners.main.tls ? "HTTPS, HTTP/2, HTTP/3" : "Plain HTTP"}></lit-material-stat>
          <lit-material-stat class="mono" label="HTTP redirect" value=${s.listeners.redirect ?? "off"}></lit-material-stat>
          <lit-material-stat class="mono" label="Admin" value=${s.listeners.admin}></lit-material-stat>
        </div>
      </section>
      <section class="section" aria-labelledby="build-title">
        <h2 id="build-title">Build</h2>
        <p class="lede">Version ${s.version} with ${s.features.join(", ")}.</p>
      </section>
    `;
  }

  renderHealthTile() {
    const [up, all] = this.backendHealth();
    if (!all) return nothing;
    return html`<button class="tile tile-link" @click=${() => this.show("upstreams")}>
      <span class="label">Backends healthy</span>
      <span class="value">${up} of ${all}</span>
      ${up < all ? status(up === 0 ? "error" : "warning", `${all - up} out: see Upstreams`) : status("ok", "All serving")}
    </button>`;
  }

  renderUpstreams() {
    const ups = this.data.upstreams;
    if (!ups) return html`<h1>Upstreams</h1>${this.skeleton(2)}`;
    return html`
      <h1>Upstreams</h1>
      <p class="lede">Drain a backend before deploying it: new requests go elsewhere, requests already on it finish, and its health checks keep running.</p>
      ${ups.length === 0
        ? html`<lit-material-empty-state size="compact" headline="No upstreams configured">Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</lit-material-empty-state>`
        : ups.map((u) => html`
          <section class="section" aria-labelledby="up-${u.name}">
            <h2 id="up-${u.name}">${u.name}</h2>
            <div class="table-wrap"><lit-material-table class="backends" caption="Backends of ${u.name}" caption-hidden .columns=${BACKEND_COLUMNS(this, u.name)} .rows=${u.backends}></lit-material-table></div>
          </section>`)}
    `;
  }

  renderCertificates() {
    const c = this.data.certificates;
    if (!c) return html`<h1>Certificates</h1>${this.skeleton(2)}`;
    const acme = c.acme;
    return html`
      <h1>Certificates</h1>
      <p class="lede">Every certificate this server presents, soonest expiry first.</p>
      ${c.certificates.length === 0
        ? html`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`
        : html`<div class="table-wrap"><lit-material-table caption="Certificates" caption-hidden .columns=${CERT_COLUMNS} .rows=${c.certificates}></lit-material-table></div>`}
      <section class="section" aria-labelledby="acme-title">
        <h2 id="acme-title">ACME</h2>
        ${acme.running
          ? html`<p class="lede">Renewal runs automatically. ${acme.last_check ? html`Last check ${new Date(acme.last_check.at * 1000).toLocaleString()}: ${acme.last_check.outcome}.` : "No check has finished yet."}</p>
              <lit-material-button variant="outlined" ?disabled=${this.busy === "renew"}
                @click=${() => this.act("renew", "/certificates/renew", { method: "POST" }, () => "Renewal requested: the new certificate is swapped in when it's issued")}>
                <span slot="icon" class="icon">${icon("arrow-clockwise")}</span>Renew now</lit-material-button>`
          : html`<lit-material-empty-state size="compact" headline="ACME is off">Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</lit-material-empty-state>`}
      </section>
    `;
  }

  renderControls() {
    const d = this.data;
    if (!d.maintenance || !d.blocklist || !d.flags) return html`<h1>Controls</h1>${this.skeleton(3)}`;
    const blockColumns = [
      { key: "ip", label: "Address", mono: true },
      {
        key: "action",
        label: "Action",
        hiddenLabel: true,
        render: (r) => html`<lit-material-button variant="text" ?disabled=${this.busy === `unblock:${r.ip}`} @click=${() => this.unblock(r.ip)}>Unblock</lit-material-button>`,
      },
    ];
    const cacheColumns = [
      { key: "name", label: "Cache", mono: true },
      { key: "kind", label: "Store" },
      { key: "entries", label: "Entries", numeric: true },
      { key: "bytes", label: "On disk", numeric: true, render: (c) => (c.bytes === null ? "—" : bytes(c.bytes)) },
      { key: "hits", label: "Hits / misses", numeric: true, render: (c) => `${c.hits} / ${c.misses}` },
      {
        key: "action",
        label: "Action",
        hiddenLabel: true,
        render: (c) => html`<lit-material-button variant="text" class="danger-text" ?disabled=${this.busy === `purge:${c.name}` || !c.entries} @click=${() => this.purgeAll(c.name, c.entries)}><span slot="icon" class="icon">${icon("trash")}</span>Purge all</lit-material-button>`,
      },
    ];
    const flagColumns = [
      { key: "name", label: "Flag", mono: true },
      {
        key: "enabled",
        label: "Enabled",
        render: (f) => html`<lit-material-switch aria-label="${f.name}" .checked=${f.enabled}
          @change=${(e) => this.act(`flag:${f.name}`, `/flags/${encodeURIComponent(f.name)}`, { method: "PUT", body: { enabled: e.target.checked } }, (r) => `${r.name} ${r.enabled ? "on" : "off"}`)}></lit-material-switch>`,
      },
    ];
    return html`
      <h1>Controls</h1>
      <p class="lede">Changes here apply at once and last until the server restarts.</p>
      <div class="grid">
        <div class="tile">
          <div class="label">Maintenance mode</div>
          <div class="row" style="margin-top: var(--space-75)">
            <lit-material-switch aria-label="Maintenance mode" .checked=${d.maintenance.active} ?disabled=${this.busy === "maintenance"}
              @change=${(e) => this.setMaintenance(e.target.checked, e.target)}></lit-material-switch>
            ${d.maintenance.active ? status("warning", "On: 503 for visitors") : status("ok", "Off")}
          </div>
        </div>
        <div class="tile">
          <div class="label">Configuration</div>
          <p class="label" style="margin: var(--space-50) 0 var(--space-100)">Re-read rws.config.toml and the certificates.</p>
          <lit-material-button variant="tonal" ?disabled=${this.busy === "reload"} @click=${() => this.reload()}><span slot="icon" class="icon">${icon("arrow-clockwise")}</span>Reload</lit-material-button>
        </div>
        <lit-material-stat label="Global rate limit" value=${d.rateLimit?.configured ? `${d.rateLimit.max_requests} per ${d.rateLimit.window_secs}s` : "Off"}></lit-material-stat>
      </div>

      <section class="section" aria-labelledby="block-title">
        <h2 id="block-title">Blocked addresses</h2>
        <form class="row" @submit=${(e) => this.block(e)} novalidate>
          <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"
            ?error=${Boolean(this.fieldError.block)} error-text=${this.fieldError.block ?? ""} @input=${() => this.fieldError.block && (this.fieldError = { ...this.fieldError, block: "" })}></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy === "block"}><span slot="icon" class="icon">${icon("plus")}</span>Block</lit-material-button>
        </form>
        ${d.blocklist.length === 0
          ? html`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`
          : html`<div class="table-wrap" style="margin-top: var(--space-200)"><lit-material-table caption="Blocked addresses" caption-hidden density="compact"
              .columns=${blockColumns} .rows=${d.blocklist.map((ip) => ({ ip }))}></lit-material-table></div>`}
      </section>

      ${d.authz ? this.renderAuthz(d.authz, d.authzCheck) : nothing}

      <section class="section" aria-labelledby="caches-title">
        <h2 id="caches-title">Response caches</h2>
        ${!d.caches || d.caches.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`
          : html`<div class="table-wrap"><lit-material-table caption="Response caches" caption-hidden density="compact" .columns=${cacheColumns} .rows=${d.caches}></lit-material-table></div>
              ${d.caches.map((c) => html`<form class="row purge" @submit=${(e) => this.purgePath(e, c.name)} novalidate>
                <lit-material-text-field variant="outlined" label="Path to purge from ${c.name}" supporting-text="One path, or a prefix ending in *: /products/*"
                  ?error=${Boolean(this.fieldError[`purge:${c.name}`])} error-text=${this.fieldError[`purge:${c.name}`] ?? ""}></lit-material-text-field>
                <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy === `purge:${c.name}`}>Purge path</lit-material-button>
              </form>`)}`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${d.flags.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`
          : html`<div class="table-wrap"><lit-material-table caption="Feature flags" caption-hidden density="compact" .columns=${flagColumns} .rows=${d.flags}></lit-material-table></div>`}
      </section>
    `;
  }

  renderAuthz(authz, check) {
    const columns = [
      { key: "tuple", label: "Tuple", mono: true },
      {
        key: "action",
        label: "Action",
        hiddenLabel: true,
        render: (r) => html`<lit-material-button variant="text" ?disabled=${this.busy === `authz-del:${r.tuple}`}
          @click=${() => this.act(`authz-del:${r.tuple}`, `/authz/tuples?tuple=${encodeURIComponent(r.tuple)}`, { method: "DELETE" }, () => `${r.tuple} removed`)}>Remove</lit-material-button>`,
      },
    ];
    const types = Object.entries(authz.schema.types).filter(([, relations]) => relations.length).map(([type, relations]) => `${type}: ${relations.join(", ")}`);
    return html`
      <section class="section" aria-labelledby="authz-title">
        <h2 id="authz-title">Authorization</h2>
        <p class="lede">Relationship tuples, object#relation@subject. Schema: ${types.join(" · ")}.</p>
        <form class="row" @submit=${(e) => this.addTuple(e)} novalidate>
          <lit-material-text-field id="authz-tuple" variant="outlined" label="Tuple" supporting-text="e.g. doc:42#viewer@user:alice"></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy === "authz-add"}><span slot="icon" class="icon">${icon("plus")}</span>Add</lit-material-button>
        </form>
        ${authz.tuples.length === 0
          ? html`<p class="lede" style="margin-top: var(--space-200)">No tuples yet.</p>`
          : html`<lit-material-table style="margin-top: var(--space-200)" caption="Relationship tuples" caption-hidden density="compact"
              .columns=${columns} .rows=${authz.tuples.map((tuple) => ({ tuple }))}></lit-material-table>`}
        <form class="row" style="margin-top: var(--space-300)" @submit=${(e) => this.checkAccess(e)} novalidate>
          <lit-material-text-field id="authz-subject" variant="outlined" label="Subject" supporting-text="user:alice"></lit-material-text-field>
          <lit-material-text-field id="authz-relation" variant="outlined" label="Relation" supporting-text="viewer"></lit-material-text-field>
          <lit-material-text-field id="authz-object" variant="outlined" label="Object" supporting-text="doc:42"></lit-material-text-field>
          <lit-material-button type="submit" variant="tonal">Check</lit-material-button>
        </form>
        ${check
          ? html`<p class="lede" id="authz-result" style="margin-top: var(--space-200)">
              ${check.allowed ? status("ok", "Allowed") : status("error", "Denied")}
              ${check.subject} ${check.relation} ${check.object}${check.allowed ? html` — via <code>${check.path.join(" → ")}</code>` : nothing}</p>`
          : nothing}
      </section>
    `;
  }

  renderConfig() {
    const config = this.data.config;
    if (!config) return html`<h1>Config</h1>${this.skeleton(2)}`;
    const needle = this.filter.toLowerCase();
    const rows = Object.entries(config)
      .filter(([k, v]) => !needle || k.toLowerCase().includes(needle) || v.toLowerCase().includes(needle))
      .map(([setting, value]) => ({ setting, value }));
    const columns = [
      { key: "setting", label: "Setting", mono: true },
      { key: "value", label: "Value", mono: true, render: (r) => (r.value === "" ? status("neutral", "empty") : html`<lit-material-truncate position="middle" style="max-width: min(60ch, 50vw)" .value=${r.value}></lit-material-truncate>`) },
    ];
    return html`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${(e) => (this.filter = e.target.value)}></lit-material-text-field>
      ${rows.length === 0
        ? html`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${Object.keys(config).length}.</lit-material-empty-state>`
        : html`<div class="table-wrap"><lit-material-table caption="Settings in effect" caption-hidden density="compact" .columns=${columns} .rows=${rows}></lit-material-table></div>`}
    `;
  }

  renderAudit() {
    const entries = this.data.audit;
    if (!entries) return html`<h1>Audit</h1>${this.skeleton(2)}`;
    const columns = [
      { key: "time", label: "When", render: (e) => new Date(e.at * 1000).toLocaleString() },
      { key: "user", label: "Who", render: (e) => html`${e.user} <span class="meta">${e.role}${e.via === "oidc" ? " · SSO" : e.via === "mcp" ? " · MCP" : ""}</span>` },
      { key: "change", label: "Change", mono: true, render: (e) => `${e.method === "TOOL" ? "tool" : e.method} ${e.path.replace("/admin/api/v1", "")}` },
      { key: "detail", label: "Details", mono: true, render: (e) => (e.detail ? html`<lit-material-truncate style="max-width: min(40ch, 30vw)" .value=${e.detail}></lit-material-truncate>` : nothing) },
      { key: "status", label: "Result", render: (e) => (e.status < 300 ? status("ok", "Done") : e.status === 401 || e.status === 403 ? status("error", "Refused") : status("warning", String(e.status))) },
      { key: "address", label: "From", mono: true },
    ];
    return html`
      <h1>Audit</h1>
      <p class="lede">Every change made through the admin API, the UI or MCP tools, and every refused attempt, newest first. Kept in memory (the last 1000); <code>[admin] audit_log</code> also writes them to a file.</p>
      ${entries.length === 0
        ? html`<lit-material-empty-state size="compact" headline="No changes yet">Changes appear here as people reload, drain, block or purge.</lit-material-empty-state>`
        : html`<div class="table-wrap"><lit-material-table caption="Recent changes" caption-hidden density="compact" .columns=${columns} .rows=${entries}></lit-material-table></div>`}
    `;
  }

  renderLogs() {
    const f = this.logFilter ?? { text: "", status: "all" };
    const chips = [["all", "All"], ["2xx", "2xx"], ["3xx", "3xx"], ["4xx", "4xx"], ["5xx", "5xx"]];
    return html`
      <h1>Logs</h1>
      <p class="lede">The access log as it's written, newest at the bottom; scroll up to read, and it follows again at the bottom. The admin listener's own requests aren't in it.</p>
      <div class="row log-tools">
        <lit-material-text-field class="log-filter" variant="outlined" label="Filter" .value=${f.text} @input=${(e) => this.setLogFilter({ text: e.target.value })}></lit-material-text-field>
        <div class="seg" role="group" aria-label="Status">
          ${chips.map(([id, label]) => html`<button aria-pressed=${f.status === id} @click=${() => this.setLogFilter({ status: id })}>${label}</button>`)}
        </div>
        <span class="spacer"></span>
        <lit-material-button variant="outlined" @click=${() => (this.paused = !this.paused)}>
          <span slot="icon" class="icon">${icon(this.paused ? "play" : "pause")}</span>${this.paused ? "Resume" : "Pause"}</lit-material-button>
      </div>
      <lit-material-log-viewer label="Access log" max-lines="1000" wrap ?paused=${this.paused}></lit-material-log-viewer>
    `;
  }
}

customElements.define("rws-admin", RwsAdmin);
