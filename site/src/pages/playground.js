// /playground/: build an rws.config.toml from a form. The generator
// (src/playground/generate.js) is shared with scripts/check-playground.js,
// which proves every combination passes `rws --check`.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { icon } from "../icons.js";
import { DEFAULTS, generate } from "../playground/generate.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/playground/",
  title: "Config playground · rws",
  description: "Build an rws.config.toml for static files, automatic HTTPS and a reverse proxy, with the command to run it. Every combination is checked with rws --check.",
};

// Lit allows no binding inside <textarea>, so its default is written as text.
const escapeHtml = (s) => String(s).replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[ch]);

const field = (name, label, attrs = {}, hint = "") => html`
  <label class="field">
    <span class="field-label">${label}</span>
    ${attrs.textarea
      ? unsafeHTML(`<textarea name="${name}" rows="3" spellcheck="false">${escapeHtml(DEFAULTS[name])}</textarea>`)
      : html`<input name=${name} type=${attrs.type ?? "text"} value=${String(DEFAULTS[name])} spellcheck="false" autocomplete="off" inputmode=${attrs.inputmode ?? "text"} />`}
    ${hint ? html`<span class="field-hint">${hint}</span>` : ""}
    <span class="field-error" data-error=${name} hidden></span>
  </label>`;

const select = (name, label, options) => html`
  <label class="field">
    <span class="field-label">${label}</span>
    <select name=${name}>${options.map(([v, t]) => html`<option value=${v} ?selected=${DEFAULTS[name] === v}>${t}</option>`)}</select>
  </label>`;

const toggle = (name, label, hint) => html`
  <label class="toggle">
    <input type="checkbox" name=${name} ?checked=${DEFAULTS[name]} />
    <span><b>${label}</b>${hint ? html`<small>${hint}</small>` : ""}</span>
  </label>`;

export function render() {
  const initial = generate({});
  return html`
    ${header(meta.path)}
    <main id="main" class="container playground">
      <div class="playground-intro">
        <h1>Config playground</h1>
        <p class="lead">Pick what rws should do. The file and the command update as you go, and every combination this page can build passes <code>rws --check</code>.</p>
      </div>
      <div class="playground-grid">
        <form class="playground-form" data-playground novalidate>
          <fieldset>
            <legend>Basics</legend>
            <div class="field-row">
              ${field("port", "Port", { type: "number", inputmode: "numeric" }, "Ignored with HTTPS, which uses 443 and 80.")}
              ${select("logFormat", "Access log", [["json", "JSON"], ["combined", "Combined"], ["off", "Off"]])}
            </div>
            ${toggle("watch", "Reload when the file changes", "watch = true; SIGHUP always works.")}
          </fieldset>

          <fieldset>
            <legend>${toggle("https", "Automatic HTTPS", "Certificates from Let's Encrypt, renewed by rws.")}</legend>
            <div class="fieldset-body" data-when="https">
              ${field("domains", "Domains", {}, "Comma or space separated. Point their DNS at this server first.")}
              ${field("email", "Email for the CA", { type: "email", inputmode: "email" })}
              ${select("challenge", "Challenge", [["http-01", "HTTP-01 on port 80"], ["tls-alpn-01", "TLS-ALPN-01 on port 443"]])}
              ${toggle("staging", "Use the staging CA first", "Untrusted test certificates, generous rate limits.")}
            </div>
          </fieldset>

          <fieldset>
            <legend>${toggle("proxy", "Reverse proxy", "Load balance requests to your app servers.")}</legend>
            <div class="fieldset-body" data-when="proxy">
              ${field("backends", "Backends", { textarea: true }, "One per line: host:port or a URL.")}
              <div class="field-row">
                ${select("strategy", "Strategy", [["least_connections", "Least connections"], ["round_robin", "Round robin"], ["ip_hash", "IP hash"], ["random", "Random"]])}
                ${field("healthPath", "Health check path", {}, "Leave empty for no health checks.")}
              </div>
              <div class="field-row">
                ${field("host", "Only for host", {}, "Optional, like api.example.com.")}
                ${field("path", "Path", {}, "Ending in * matches a prefix.")}
              </div>
              ${toggle("routeRate", "Rate limit this route")}
              <div class="field-row" data-when="routeRate">
                ${field("routeMax", "Requests", { type: "number", inputmode: "numeric" })}
                ${field("routeWindow", "Per seconds", { type: "number", inputmode: "numeric" })}
              </div>
              ${toggle("auth", "Require a bearer token", "Fails closed: no token configured means 503.")}
              <div data-when="auth">${field("tokenEnv", "Token environment variable")}</div>
            </div>
          </fieldset>

          <fieldset>
            <legend>${toggle("cors", "CORS", "Let browser apps on other origins call this server.")}</legend>
            <div class="fieldset-body" data-when="cors">${field("corsOrigins", "Allowed origins", {}, "Comma separated.")}</div>
          </fieldset>

          <fieldset>
            <legend>${toggle("rateLimit", "Global rate limit", "Per client IP, for every request.")}</legend>
            <div class="fieldset-body field-row" data-when="rateLimit">
              ${field("maxRequests", "Requests", { type: "number", inputmode: "numeric" })}
              ${field("windowSecs", "Per seconds", { type: "number", inputmode: "numeric" })}
            </div>
          </fieldset>
        </form>

        <div class="playground-output">
          <div class="output-sticky">
            <lit-material-code-block label="rws.config.toml" copyable><pre><code data-output="toml">${initial.toml}</code></pre></lit-material-code-block>
            <p class="output-step">Save it next to the files you serve, then run:</p>
            <lit-material-code-block label="bash" copyable><pre><code data-output="command">${initial.command}</code></pre></lit-material-code-block>
            <p class="output-status" data-status role="status">${unsafeHTML(icon("check-circle"))}<span>Valid. Every key here is in the <a href="/docs/configuration/config-file/">config file reference</a>.</span></p>
            <noscript><p class="output-step">The builder needs JavaScript. The file above is the default; the <a href="/docs/configuration/config-file/">config file reference</a> lists every key.</p></noscript>
          </div>
        </div>
      </div>
    </main>
    ${footer()}`;
}
