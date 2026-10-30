import { LitElement, html, nothing } from "lit";
import { api, ApiError, followLogs, setToken, token } from "./api.js";
import { icon } from "./icons.js";
import { requestChart } from "./chart.js";

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

function status(kind, label, name) {
  return html`<span class="status ${kind}"><span class="icon">${icon(name)}</span>${label}</span>`;
}

/** The whole app, rendered into the light DOM so the theme's classes apply. */
export class RwsAdmin extends LitElement {
  static properties = {
    authed: { state: true },
    view: { state: true },
    loginError: { state: true },
    data: { state: true },
    error: { state: true },
    samples: { state: true },
    lines: { state: true },
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
    this.lines = [];
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
    const bar = this.querySelector("#toast");
    if (!bar) return;
    bar.textContent = message;
    bar.show?.();
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
    if (view === "logs") this.follow();
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
          this.samples = [...this.samples, { at: now, rate }].slice(-SAMPLES);
        }
        this.lastTotal = { total, at: now };
      }
    } else if (view === "upstreams") {
      await get("upstreams", "/upstreams");
    } else if (view === "certificates") {
      await get("certificates", "/certificates");
    } else if (view === "controls") {
      await Promise.all([get("maintenance", "/maintenance"), get("blocklist", "/blocklist"), get("flags", "/flags"), get("rateLimit", "/rate-limit"), this.loadCache()]);
    } else if (view === "config") {
      await get("config", "/config");
    }
  }

  async loadCache() {
    try {
      const cache = await api("/cache");
      this.data = { ...this.data, cache };
    } catch (e) {
      this.data = { ...this.data, cache: e instanceof ApiError && e.status === 404 ? null : this.data.cache };
    }
  }

  async follow() {
    this.abort = new AbortController();
    try {
      const recent = await this.call("/logs?n=200");
      this.lines = recent.map((text) => ({ text }));
      await followLogs((text) => {
        if (this.paused) return;
        this.lines = [...this.lines, { text }].slice(-1000);
        this.updateComplete.then(() => {
          const box = this.querySelector(".log");
          if (box && box.scrollHeight - box.scrollTop - box.clientHeight < 80) box.scrollTop = box.scrollHeight;
        });
      }, this.abort.signal);
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

  // ── rendering ─────────────────────────────────────────────────────────
  render() {
    if (!this.authed) return this.renderLogin();
    const info = this.data.status;
    return html`
      <div class="shell">
        <header class="top">
          <span class="brand"><span class="brand-mark" aria-hidden="true"></span>rws admin</span>
          ${info ? (info.ready ? status("ok", "Ready", "check-circle") : status("bad", "Not ready", "x-circle")) : nothing}
          <span class="spacer"></span>
          ${info ? html`<span class="meta">${info.version} · ${info.listeners?.main?.address ?? ""}</span>` : nothing}
          <lit-material-button variant="text" @click=${() => this.logout()}><span slot="icon" class="icon">${icon("sign-out")}</span>Sign out</lit-material-button>
        </header>
        <nav class="side" aria-label="Sections">
          <lit-material-navigation-drawer .selected=${VIEWS.findIndex((v) => v.id === this.view)} @change=${(e) => this.show(VIEWS[e.target.selected].id)}>
            ${VIEWS.map((v) => html`<lit-material-navigation-drawer-item ?selected=${v.id === this.view}><span slot="icon">${icon(v.icon)}</span>${v.label}</lit-material-navigation-drawer-item>`)}
          </lit-material-navigation-drawer>
        </nav>
        <main id="main" tabindex="-1">
          ${this.error ? html`<lit-material-alert variant="error" class="section-alert"><span slot="title">Something failed</span>${this.error}</lit-material-alert>` : nothing}
          ${this.renderView()}
        </main>
        <nav class="bottom-nav" aria-label="Sections">
          <lit-material-navigation-bar .selected=${VIEWS.findIndex((v) => v.id === this.view)} @change=${(e) => this.show(VIEWS[e.target.selected].id)}>
            ${VIEWS.map((v) => html`<lit-material-navigation-bar-item ?selected=${v.id === this.view}><span slot="icon">${icon(v.icon)}</span>${v.label}</lit-material-navigation-bar-item>`)}
          </lit-material-navigation-bar>
        </nav>
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
              @keydown=${(e) => e.key === "Enter" && this.login(e)}
              ?error=${Boolean(this.loginError)} error-text=${this.loginError ?? ""}></lit-material-text-field>
            <lit-material-button type="submit" @click=${(e) => this.login(e)}>Sign in</lit-material-button>
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
    return html`
      <h1>Overview</h1>
      <p class="lede">Up ${duration(s.uptime_secs)}, process ${s.pid}. Counters refresh every 2 seconds.</p>
      <div class="grid">
        <div class="tile"><div class="label">Requests served</div><div class="value">${m.requests_total.toLocaleString()}</div></div>
        <div class="tile"><div class="label">Errors</div><div class="value">${m.errors_total.toLocaleString()}</div><div class="label">${errorRate}% of requests</div></div>
        <div class="tile"><div class="label">Open connections</div><div class="value">${m.active_connections}</div></div>
        <div class="tile"><div class="label">Queued for a worker</div><div class="value">${m.thread_pool_queued}</div></div>
      </div>
      <section class="section" aria-labelledby="rate-title">
        <h2 id="rate-title">Requests per second</h2>
        ${this.samples.length < 2
          ? html`<div class="empty"><strong>Collecting samples</strong>The chart fills in as this page stays open.</div>`
          : requestChart(this.samples)}
      </section>
      <section class="section" aria-labelledby="listen-title">
        <h2 id="listen-title">Listeners</h2>
        <div class="grid">
          <div class="tile"><div class="label">Main</div><div class="value mono">${s.listeners.main.address}</div><div class="label">${s.listeners.main.tls ? "HTTPS, HTTP/2, HTTP/3" : "Plain HTTP"}</div></div>
          <div class="tile"><div class="label">HTTP redirect</div><div class="value mono">${s.listeners.redirect ?? "off"}</div></div>
          <div class="tile"><div class="label">Admin</div><div class="value mono">${s.listeners.admin}</div></div>
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
        ? html`<div class="empty"><strong>No upstreams configured</strong>Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</div>`
        : ups.map((u) => html`
          <section class="section" aria-labelledby="up-${u.name}">
            <h2 id="up-${u.name}">${u.name}</h2>
            <div class="table-wrap"><table>
              <thead><tr><th scope="col">Backend</th><th scope="col" class="num">Weight</th><th scope="col">Health</th><th scope="col">Breaker</th><th scope="col">In rotation</th></tr></thead>
              <tbody>${u.backends.map((b) => html`<tr>
                <td class="mono">${b.url}</td>
                <td class="num">${b.weight}</td>
                <td>${b.healthy ? status("ok", "Healthy", "check-circle") : status("bad", "Failing checks", "x-circle")}</td>
                <td>${b.breaker === null ? status("muted", "None", "list-bullets")
                  : b.breaker === "closed" ? status("ok", "Closed", "check-circle")
                  : b.breaker === "half_open" ? status("warn", "Probing", "warning-circle")
                  : status("bad", "Open", "x-circle")}</td>
                <td><div class="row">
                  <lit-material-switch aria-label="${b.url} in rotation" .checked=${!b.drained} ?disabled=${this.busy === `drain:${b.url}`}
                    @change=${(e) => this.drain(u.name, b.url, !e.target.checked)}></lit-material-switch>
                  ${b.drained ? status("warn", "Drained", "pause") : nothing}
                </div></td>
              </tr>`)}</tbody>
            </table></div>
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
        ? html`<div class="empty"><strong>No TLS certificates</strong>This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</div>`
        : html`<div class="table-wrap"><table>
            <thead><tr><th scope="col">Name</th><th scope="col">Expires</th><th scope="col" class="num">Days left</th><th scope="col">Source</th></tr></thead>
            <tbody>${c.certificates.map((x) => html`<tr>
              <td class="mono">${x.name}</td>
              <td>${x.expires}</td>
              <td class="num">${x.days_left < 7 ? status("bad", String(x.days_left), "warning-circle") : x.days_left < 21 ? status("warn", String(x.days_left), "warning-circle") : x.days_left}</td>
              <td>${x.source === "on_demand" ? "On demand" : "Configured"}</td>
            </tr>`)}</tbody>
          </table></div>`}
      <section class="section" aria-labelledby="acme-title">
        <h2 id="acme-title">ACME</h2>
        ${acme.running
          ? html`<p class="lede">Renewal runs automatically. ${acme.last_check ? html`Last check ${new Date(acme.last_check.at * 1000).toLocaleString()}: ${acme.last_check.outcome}.` : "No check has finished yet."}</p>
              <lit-material-button variant="outlined" ?disabled=${this.busy === "renew"}
                @click=${() => this.act("renew", "/certificates/renew", { method: "POST" }, () => "Renewal requested: the new certificate is swapped in when it's issued")}>
                <span slot="icon" class="icon">${icon("arrow-clockwise")}</span>Renew now</lit-material-button>`
          : html`<div class="empty"><strong>ACME is off</strong>Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</div>`}
      </section>
    `;
  }

  renderControls() {
    const d = this.data;
    if (!d.maintenance || !d.blocklist || !d.flags) return html`<h1>Controls</h1>${this.skeleton(3)}`;
    return html`
      <h1>Controls</h1>
      <p class="lede">Changes here apply at once and last until the server restarts.</p>
      <div class="grid">
        <div class="tile">
          <div class="label">Maintenance mode</div>
          <div class="row" style="margin-top: var(--space-75)">
            <lit-material-switch aria-label="Maintenance mode" .checked=${d.maintenance.active} ?disabled=${this.busy === "maintenance"}
              @change=${(e) => this.act("maintenance", "/maintenance", { method: "PUT", body: { active: e.target.checked } }, (r) => (r.active ? "Maintenance on: visitors get 503" : "Maintenance off"))}></lit-material-switch>
            ${d.maintenance.active ? status("warn", "On: 503 for visitors", "warning-circle") : status("ok", "Off", "check-circle")}
          </div>
        </div>
        <div class="tile">
          <div class="label">Configuration</div>
          <p class="label" style="margin: var(--space-50) 0 var(--space-100)">Re-read rws.config.toml and the certificates.</p>
          <lit-material-button variant="tonal" ?disabled=${this.busy === "reload"} @click=${() => this.reload()}><span slot="icon" class="icon">${icon("arrow-clockwise")}</span>Reload</lit-material-button>
        </div>
        <div class="tile">
          <div class="label">Response cache</div>
          ${d.cache
            ? html`<div class="value">${d.cache.size}</div><div class="label">${d.cache.hits} hits, ${d.cache.misses} misses</div>
                <lit-material-button variant="text" ?disabled=${this.busy === "cache"} @click=${() => this.act("cache", "/cache", { method: "DELETE" }, () => "Cache cleared")}><span slot="icon" class="icon">${icon("trash")}</span>Clear</lit-material-button>`
            : html`<div class="value mono">None</div><div class="label">This server has no response cache.</div>`}
        </div>
        <div class="tile">
          <div class="label">Global rate limit</div>
          <div class="value mono">${d.rateLimit?.configured ? `${d.rateLimit.max_requests} per ${d.rateLimit.window_secs}s` : "Off"}</div>
        </div>
      </div>

      <section class="section" aria-labelledby="block-title">
        <h2 id="block-title">Blocked addresses</h2>
        <form class="row" @submit=${(e) => this.block(e)} novalidate>
          <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"
            @keydown=${(e) => e.key === "Enter" && this.block(e)}></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy === "block"} @click=${(e) => this.block(e)}><span slot="icon" class="icon">${icon("plus")}</span>Block</lit-material-button>
        </form>
        ${d.blocklist.length === 0
          ? html`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`
          : html`<div class="table-wrap section" style="margin-top: var(--space-200)"><table>
              <thead><tr><th scope="col">Address</th><th scope="col"><span class="visually-hidden">Action</span></th></tr></thead>
              <tbody>${d.blocklist.map((ip) => html`<tr><td class="mono">${ip}</td><td>
                <lit-material-button variant="text" @click=${() => this.act(`unblock:${ip}`, `/blocklist/${encodeURIComponent(ip)}`, { method: "DELETE" }, () => `${ip} unblocked`)}>Unblock</lit-material-button>
              </td></tr>`)}</tbody>
            </table></div>`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${d.flags.length === 0
          ? html`<div class="empty"><strong>No feature flags</strong>Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</div>`
          : html`<div class="table-wrap"><table>
              <thead><tr><th scope="col">Flag</th><th scope="col">Enabled</th></tr></thead>
              <tbody>${d.flags.map((f) => html`<tr><td class="mono">${f.name}</td><td>
                <lit-material-switch aria-label="${f.name}" .checked=${f.enabled}
                  @change=${(e) => this.act(`flag:${f.name}`, `/flags/${encodeURIComponent(f.name)}`, { method: "PUT", body: { enabled: e.target.checked } }, (r) => `${r.name} ${r.enabled ? "on" : "off"}`)}></lit-material-switch>
              </td></tr>`)}</tbody>
            </table></div>`}
      </section>
    `;
  }

  renderConfig() {
    const config = this.data.config;
    if (!config) return html`<h1>Config</h1>${this.skeleton(2)}`;
    const needle = this.filter.toLowerCase();
    const rows = Object.entries(config).filter(([k, v]) => !needle || k.toLowerCase().includes(needle) || v.toLowerCase().includes(needle));
    return html`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${(e) => (this.filter = e.target.value)}></lit-material-text-field>
      ${rows.length === 0
        ? html`<div class="empty"><strong>No setting matches "${this.filter}"</strong>Clear the filter to see all ${Object.keys(config).length}.</div>`
        : html`<div class="table-wrap"><table>
            <thead><tr><th scope="col">Setting</th><th scope="col">Value</th></tr></thead>
            <tbody>${rows.map(([k, v]) => html`<tr><td class="mono">${k}</td><td class="mono">${v === "" ? html`<span class="status muted">empty</span>` : v}</td></tr>`)}</tbody>
          </table></div>`}
    `;
  }

  renderLogs() {
    return html`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom.</p>
        <lit-material-button variant="outlined" @click=${() => (this.paused = !this.paused)}>
          <span slot="icon" class="icon">${icon(this.paused ? "play" : "pause")}</span>${this.paused ? "Resume" : "Pause"}</lit-material-button>
      </div>
      ${this.lines.length === 0
        ? html`<div class="empty"><strong>Waiting for requests</strong>Lines appear as the server handles requests. With <code>log_format = "off"</code> there are none.</div>`
        : html`<div class="log" role="log" aria-live="off">${this.lines.map((l) => html`<div class=${/"status":[45]\d\d/.test(l.text) ? "err" : ""}>${l.text}</div>`)}</div>`}
    `;
  }
}

customElements.define("rws-admin", RwsAdmin);
