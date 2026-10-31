import { LitElement, html, nothing } from "lit";
import { api, ApiError, followLogs, setToken, token } from "./api.js";
import { icon } from "./icons.js";

const bytes = (n) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KiB` : `${(n / 1048576).toFixed(1)} MiB`);

const VIEWS = [
  { id: "overview", label: "Overview", icon: "gauge" },
  { id: "upstreams", label: "Upstreams", icon: "tree-structure" },
  { id: "certificates", label: "Certificates", icon: "certificate" },
  { id: "controls", label: "Controls", icon: "sliders-horizontal" },
  { id: "config", label: "Config", icon: "list-bullets" },
  { id: "logs", label: "Logs", icon: "terminal-window" },
];

const SAMPLES = 60;

function viewFromHash() {
  const id = location.hash.replace(/^#\/?/, "");
  return VIEWS.some((v) => v.id === id) ? id : "overview";
}

function duration(secs) {
  const d = Math.floor(secs / 86400), h = Math.floor((secs % 86400) / 3600), m = Math.floor((secs % 3600) / 60);
  if (d) return `${d}d ${h}h`;
  if (h) return `${h}h ${m}m`;
  return `${m}m ${secs % 60}s`;
}

const status = (kind, label) => html`<lit-material-status status=${kind}>${label}</lit-material-status>`;
const time = (ms) => new Date(ms).toLocaleTimeString();
const logLevel = (line) => (/"status":5\d\d/.test(line) ? "error" : /"status":4\d\d/.test(line) ? "warning" : null);

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
        @change=${(e) => self.drain(upstream, b.url, !e.target.checked)}></lit-material-switch>
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
  }

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    this.onHash = () => this.show(viewFromHash());
    addEventListener("hashchange", this.onHash);
    if (token()) this.check();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    removeEventListener("hashchange", this.onHash);
    this.stop();
  }

  // ── session ───────────────────────────────────────────────────────────
  async check() {
    try {
      const info = await api("/status");
      this.data = { ...this.data, status: info };
      this.authed = true;
      this.loginError = "";
      this.show(this.view);
    } catch (e) {
      this.authed = false;
      this.loginError = e instanceof ApiError && e.status === 401 ? "That token isn't the admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server." : `The admin API didn't answer: ${e.message}`;
      if (e instanceof ApiError && e.status === 401) setToken("");
    }
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
    setToken("");
    this.stop();
    this.authed = false;
    this.data = {};
  }

  /** An API call from a view: errors land in the page, 401 signs out. */
  async call(path, options) {
    try {
      const result = await api(path, options);
      this.error = "";
      return result;
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) {
        this.logout();
        this.loginError = "The server no longer accepts this token. Sign in again.";
      } else {
        this.error = e.message;
      }
      throw e;
    }
  }

  toast(message) {
    this.querySelector("#toast")?.show(message);
  }

  // ── views and their polling ───────────────────────────────────────────
  show(view) {
    this.stop();
    this.view = view;
    if (location.hash !== `#/${view}`) history.replaceState(null, "", `#/${view}`);
    this.error = "";
    const every = { overview: 2000, upstreams: 3000, certificates: 30000, controls: 5000 }[view];
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
      await Promise.all([get("maintenance", "/maintenance"), get("blocklist", "/blocklist"), get("flags", "/flags"), get("rateLimit", "/rate-limit"), get("caches", "/caches")]);
    } else if (view === "config") {
      await get("config", "/config");
    }
  }

  async follow() {
    this.abort = new AbortController();
    const log = this.querySelector("lit-material-log-viewer");
    if (!log) return;
    log.highlight = logLevel;
    try {
      log.lines = await this.call("/logs?n=200");
      await followLogs((line) => log.appendLines(line), this.abort.signal);
    } catch (e) {
      if (e.name !== "AbortError" && !this.error) this.error = `The live log stopped: ${e.message}`;
    }
  }

  // ── actions ───────────────────────────────────────────────────────────
  async act(label, path, options, after) {
    this.busy = label;
    try {
      const result = await this.call(path, options);
      if (after) this.toast(after(result));
      await this.load(this.view);
    } catch {
      // Shown through this.error.
    } finally {
      this.busy = "";
    }
  }

  drain(upstream, backend, drained) {
    return this.act(`drain:${backend}`, `/upstreams/${encodeURIComponent(upstream)}/drain`, { method: "PUT", body: { backend, drained } },
      () => (drained ? `${backend} drained: it gets no new requests` : `${backend} is back in rotation`));
  }

  reload() {
    return this.act("reload", "/reload", { method: "POST" }, (r) =>
      r.applied ? "Configuration reloaded" : `Reload kept the running configuration: ${r.errors.join("; ")}`);
  }

  block(event) {
    event.preventDefault();
    const field = this.querySelector("#block-ip");
    const ip = (field?.value ?? "").trim();
    if (!ip) return;
    this.act("block", "/blocklist", { method: "POST", body: { ip } }, (r) => `${r.blocked} blocked`).then(() => {
      if (!this.error && field) field.value = "";
    });
  }

  purge(name) {
    const path = (this.querySelector("#purge-path")?.value ?? "").trim();
    const query = !path ? "" : path.endsWith("*") ? `?prefix=${encodeURIComponent(path.slice(0, -1))}` : `?path=${encodeURIComponent(path)}`;
    return this.act(`purge:${name}`, `/caches/${encodeURIComponent(name)}${query}`, { method: "DELETE" },
      (r) => `${r.purged} ${r.purged === 1 ? "entry" : "entries"} purged from ${r.name}`);
  }

  // ── rendering ─────────────────────────────────────────────────────────
  render() {
    if (!this.authed) return this.renderLogin();
    const info = this.data.status;
    const destinations = VIEWS.map((v) => ({ label: v.label, icon: icon(v.icon) }));
    return html`
      <div class="shell">
        <header class="top">
          <span class="brand"><span class="brand-mark" aria-hidden="true"></span>rws admin</span>
          ${info ? (info.ready ? status("ok", "Ready") : status("error", "Not ready")) : nothing}
          <span class="spacer"></span>
          ${info ? html`<span class="meta">${info.version} · ${info.listeners?.main?.address ?? ""}</span>` : nothing}
          <lit-material-button variant="text" @click=${() => this.logout()}><span slot="icon" class="icon">${icon("sign-out")}</span>Sign out</lit-material-button>
        </header>
        <lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${destinations}
          .selected=${VIEWS.findIndex((v) => v.id === this.view)}
          @change=${(e) => this.show(VIEWS[e.target.selected].id)}></lit-material-adaptive-navigation>
        <main id="main" tabindex="-1">
          ${this.error ? html`<lit-material-alert variant="error"><span slot="title">Something failed</span>${this.error}</lit-material-alert>` : nothing}
          ${this.renderView()}
        </main>
      </div>
      <lit-material-snackbar id="toast"></lit-material-snackbar>
    `;
  }

  renderLogin() {
    return html`
      <div class="login">
        <section class="login-card tile" aria-labelledby="login-title">
          <h1 id="login-title"><span class="icon">${icon("lock-key")}</span>Sign in to rws admin</h1>
          <p>Paste the admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names). It stays in this tab only.</p>
          <form @submit=${(e) => this.login(e)} novalidate>
            <lit-material-text-field id="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required
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
      case "config": return this.renderConfig();
      case "logs": return this.renderLogs();
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
      <p class="lede">Up ${duration(s.uptime_secs)}, process ${s.pid}. Counters refresh every 2 seconds.</p>
      <div class="grid">
        <lit-material-stat label="Requests served" value=${m.requests_total.toLocaleString()}>
          ${rates.length > 1 ? html`<lit-material-sparkline slot="caption" label="Requests per second" .values=${rates}></lit-material-sparkline>` : nothing}
        </lit-material-stat>
        <lit-material-stat label="Errors" value=${m.errors_total.toLocaleString()} caption="${errorRate}% of requests"></lit-material-stat>
        <lit-material-stat label="Open connections" value=${String(m.active_connections)}></lit-material-stat>
        <lit-material-stat label="Queued for a worker" value=${String(m.thread_pool_queued)}></lit-material-stat>
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
            <lit-material-table caption="Backends of ${u.name}" caption-hidden .columns=${BACKEND_COLUMNS(this, u.name)} .rows=${u.backends}></lit-material-table>
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
        : html`<lit-material-table caption="Certificates" caption-hidden .columns=${CERT_COLUMNS} .rows=${c.certificates}></lit-material-table>`}
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
        render: (r) => html`<lit-material-button variant="text" @click=${() => this.act(`unblock:${r.ip}`, `/blocklist/${encodeURIComponent(r.ip)}`, { method: "DELETE" }, () => `${r.ip} unblocked`)}>Unblock</lit-material-button>`,
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
        render: (c) => html`<lit-material-button variant="text" ?disabled=${this.busy === `purge:${c.name}`} @click=${() => this.purge(c.name)}><span slot="icon" class="icon">${icon("trash")}</span>Purge</lit-material-button>`,
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
              @change=${(e) => this.act("maintenance", "/maintenance", { method: "PUT", body: { active: e.target.checked } }, (r) => (r.active ? "Maintenance on: visitors get 503" : "Maintenance off"))}></lit-material-switch>
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
          <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy === "block"}><span slot="icon" class="icon">${icon("plus")}</span>Block</lit-material-button>
        </form>
        ${d.blocklist.length === 0
          ? html`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`
          : html`<lit-material-table style="margin-top: var(--space-200)" caption="Blocked addresses" caption-hidden density="compact"
              .columns=${blockColumns} .rows=${d.blocklist.map((ip) => ({ ip }))}></lit-material-table>`}
      </section>

      <section class="section" aria-labelledby="caches-title">
        <h2 id="caches-title">Response caches</h2>
        ${!d.caches || d.caches.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`
          : html`<lit-material-text-field id="purge-path" variant="outlined" label="Path to purge" supporting-text="Empty purges everything; /products/* purges a prefix"></lit-material-text-field>
              <lit-material-table style="margin-top: var(--space-200)" caption="Response caches" caption-hidden density="compact" .columns=${cacheColumns} .rows=${d.caches}></lit-material-table>`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${d.flags.length === 0
          ? html`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`
          : html`<lit-material-table caption="Feature flags" caption-hidden density="compact" .columns=${flagColumns} .rows=${d.flags}></lit-material-table>`}
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
      { key: "value", label: "Value", mono: true, render: (r) => (r.value === "" ? status("neutral", "empty") : r.value) },
    ];
    return html`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${(e) => (this.filter = e.target.value)}></lit-material-text-field>
      ${rows.length === 0
        ? html`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${Object.keys(config).length}.</lit-material-empty-state>`
        : html`<lit-material-table caption="Settings in effect" caption-hidden density="compact" .columns=${columns} .rows=${rows}></lit-material-table>`}
    `;
  }

  renderLogs() {
    return html`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom. Scroll up to read; it stops following until you're back at the bottom.</p>
        <lit-material-button variant="outlined" @click=${() => (this.paused = !this.paused)}>
          <span slot="icon" class="icon">${icon(this.paused ? "play" : "pause")}</span>${this.paused ? "Resume" : "Pause"}</lit-material-button>
      </div>
      <lit-material-log-viewer label="Access log" max-lines="1000" ?paused=${this.paused}></lit-material-log-viewer>
    `;
  }
}

customElements.define("rws-admin", RwsAdmin);
