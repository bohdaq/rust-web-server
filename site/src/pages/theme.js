// /theme/: every lit-material element the site uses, in the site theme. The
// visual check for theme changes (spec/WEBSITE_REBUILD.md, phase 1); noindex.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { icon as tile, mark } from "../brand/mark.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/theme/",
  title: "Theme check",
  description: "Every component the rws8.tech site uses, in light and dark.",
  noindex: true,
};

const STATIC = `[server]
root = "./public"
port = 8080`;

const HTTPS = `[server]
root = "./public"

[acme]
domains = ["example.com", "www.example.com"]
email = "ops@example.com"`;

const PROXY = `[[upstream]]
name = "app"
backends = ["http://10.0.0.11:3000", "http://10.0.0.12:3000"]
strategy = "least_connections"

[[route]]
match = { host = "example.com", path_prefix = "/" }
action = { proxy = { upstream = "app" } }`;

const SIZES = ["xs", "sm", "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl"];
const SPACES = [25, 50, 75, 100, 200, 300, 400, 500, 600, 700, 800, 900];

export function render() {
  return html`
    ${header(meta.path)}
    <main id="main" class="container theme-page">
      <section>
        <h1 class="hero-heading">One binary.<br />HTTPS, proxy, done.</h1>
        <p class="lead">Static files, automatic certificates and a reverse proxy, configured in one TOML file.</p>
        <div class="row">
          <lit-material-button variant="filled">Install rws</lit-material-button>
          <lit-material-button variant="outlined">Read the docs</lit-material-button>
          <lit-material-button variant="text">View on GitHub</lit-material-button>
          <lit-material-button variant="filled" disabled>Disabled</lit-material-button>
        </div>
      </section>

      <section>
        <h2>Brand</h2>
        <div class="row brand">
          <span class="mark-large">${unsafeHTML(mark())}</span>
          ${["black", "orange"].map(
            (v) => html`<img src="/brand/icon-${v}.svg" width="96" height="96" alt="rws app icon, ${v}" />`,
          )}
          <img src="/favicon.svg" width="32" height="32" alt="rws favicon" />
        </div>
      </section>

      <section>
        <h2>Three modes</h2>
        <div data-tab-group>
          <lit-material-tabs aria-label="Configuration examples">
            <lit-material-tab>Static server</lit-material-tab>
            <lit-material-tab>Automatic HTTPS</lit-material-tab>
            <lit-material-tab>Reverse proxy</lit-material-tab>
          </lit-material-tabs>
          ${[STATIC, HTTPS, PROXY].map(
            (code) => html`
              <div data-tab-panel>
                <lit-material-code-block label="rws.config.toml" copyable><pre><code>${code}</code></pre></lit-material-code-block>
              </div>`,
          )}
        </div>
      </section>

      <section>
        <h2>Alerts</h2>
        <div class="stack">
          <lit-material-alert variant="info">
            <span slot="icon">${unsafeHTML(icon("info"))}</span>
            <span slot="title">Note</span>
            Settings in <code>rws.config.toml</code> override environment variables.
          </lit-material-alert>
          <lit-material-alert variant="warning">
            <span slot="icon">${unsafeHTML(icon("warning"))}</span>
            <span slot="title">Caution</span>
            Port 80 must be reachable for the HTTP-01 challenge.
          </lit-material-alert>
          <lit-material-alert variant="success">
            <span slot="icon">${unsafeHTML(icon("check-circle"))}</span>
            <span slot="title">Tip</span>
            Run <code>rws --check</code> before reloading a changed config.
          </lit-material-alert>
          <lit-material-alert variant="error">
            <span slot="icon">${unsafeHTML(icon("x-circle"))}</span>
            <span slot="title">Error</span>
            Upload failed. The file is over 20 MB.
          </lit-material-alert>
        </div>
      </section>

      <section>
        <h2>Navigation rail</h2>
        <div class="rail-demo">
          <lit-material-navigation-rail selected="0" aria-label="Docs sections">
            ${[
              ["rocket-launch", "Start"],
              ["sliders", "Config"],
              ["code", "Build"],
              ["shield-check", "Deploy"],
            ].map(
              ([name, label]) =>
                html`<lit-material-navigation-rail-item>
                  <span slot="icon">${unsafeHTML(icon(name))}</span>${label}
                </lit-material-navigation-rail-item>`,
            )}
          </lit-material-navigation-rail>
        </div>
      </section>

      <section>
        <h2>Type scale</h2>
        <div class="stack">
          ${SIZES.map((s) => html`<p class="type-sample" style="font-size: var(--text-${s}); line-height: var(--leading-${s})">text-${s} · Static files, HTTPS and a proxy</p>`)}
          <p class="type-sample mono">Geist Mono · 38,501 req/s · p99 5.81 ms</p>
        </div>
      </section>

      <section>
        <h2>Spacing</h2>
        <div class="stack">
          ${SPACES.map((s) => html`<div class="space-row"><code>${s}</code><span class="space-bar" style="width: var(--space-${s})"></span></div>`)}
        </div>
      </section>
    </main>
    ${footer()}`;
}
