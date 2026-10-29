// /privacy/: what the site collects (nothing), stated plainly.
import { html } from "lit";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/privacy/",
  title: "Privacy · rws",
  description: "rws8.tech sets no cookies, runs no analytics and collects no personal data.",
};

export function render() {
  return html`
    ${header(meta.path)}
    <main id="main" class="container page-text">
      <h1>Privacy</h1>
      <p class="lead">rws8.tech sets no cookies, runs no analytics or tracking scripts, and collects no personal data.</p>
      <h2>What the server sees</h2>
      <p>Like any web server, the one serving this site (rws itself) receives your IP address, the page you ask for and your browser's user agent with each request. These appear in its access log, which is used only to keep the site running and is not shared.</p>
      <h2>What your browser keeps</h2>
      <p>If you pick a light or dark theme, your browser stores that choice in its own local storage under <code>rws-theme</code>. It never leaves your device. Clearing site data removes it.</p>
      <h2>Search</h2>
      <p>Docs search runs entirely in your browser against a prebuilt index. Your queries are not sent anywhere.</p>
      <h2>Questions</h2>
      <p>Open an issue on <a href="https://github.com/bohdaq/rust-web-server/issues">GitHub</a>.</p>
    </main>
    ${footer()}`;
}
