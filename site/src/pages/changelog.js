// /changelog/: CHANGELOG.md rendered with the docs pipeline, newest first,
// with a list of releases beside it.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { renderMarkdown } from "../docs/markdown.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/changelog/",
  title: "Changelog · rws",
  description: "Every rust-web-server release: what changed, what was fixed, and what breaks, newest first.",
};

export async function render() {
  const source = readFileSync(join(import.meta.dirname, "..", "..", "..", "CHANGELOG.md"), "utf8");
  // The file's own "# Changelog" heading is the page title here.
  const { html: body, toc } = await renderMarkdown(source.replace(/^# Changelog\n/, ""));
  const releases = toc.filter((t) => t.depth === 2);
  return html`
    ${header(meta.path)}
    <div class="docs-layout changelog">
      <aside class="docs-sidebar">
        <nav class="docs-toc changelog-nav" aria-label="Releases">
          <p class="docs-toc-title">Releases</p>
          <ul>${releases.map((r) => html`<li><a href=${`#${r.id}`}>${r.text}</a></li>`)}</ul>
        </nav>
      </aside>
      <main id="main" class="docs-main">
        <article class="docs-article">
          <h1>Changelog</h1>
          <div class="prose">${unsafeHTML(body)}</div>
        </article>
      </main>
    </div>
    ${footer()}`;
}
