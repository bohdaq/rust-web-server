// /404.html: served by rws for any path that doesn't exist.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { mark } from "../brand/mark.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/404.html",
  title: "Page not found · rws",
  description: "This page does not exist on rws8.tech.",
  noindex: true,
};

export function render() {
  return html`
    ${header("/404")}
    <main id="main" class="container not-found">
      <span class="not-found-mark">${unsafeHTML(mark({ title: "rws" }))}</span>
      <h1>This page broke off the rack</h1>
      <p class="lead">Nothing lives at this address. Docs pages moved under /docs/ in October 2026, so an old link may point there now.</p>
      <div class="install">
        <a class="button-filled" href="/docs/">Go to the docs</a>
        <a class="text-link" href="/">Back to the home page</a>
      </div>
    </main>
    ${footer()}`;
}
