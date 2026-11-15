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

// The newest releases are shown in full; older ones are grouped by minor
// version (18.20.x) and collapsed: the single page was 53,000 px tall.
const RECENT = 10;

export async function render() {
  const source = readFileSync(join(import.meta.dirname, "..", "..", "..", "CHANGELOG.md"), "utf8");
  // The file's own "# Changelog" heading is the page title here.
  const [intro, ...sections] = source.replace(/^# Changelog\n/, "").split(/^(?=## )/m);
  const releases = sections.map((text) => {
    const heading = text.match(/^## (.+)$/m)?.[1] ?? "";
    const minor = heading.match(/^(\d+\.\d+)/)?.[1] ?? heading;
    return { text, heading, minor };
  });
  const recent = releases.slice(0, RECENT);
  const groups = [];
  for (const r of releases.slice(RECENT)) {
    const last = groups.at(-1);
    if (last?.minor === r.minor) last.items.push(r);
    else groups.push({ minor: r.minor, items: [r] });
  }
  const introHtml = (await renderMarkdown(intro)).html;
  const recentHtml = await renderMarkdown(recent.map((r) => r.text).join("\n"));
  const groupHtml = await Promise.all(groups.map((g) => renderMarkdown(g.items.map((r) => r.text).join("\n"))));
  const ids = recentHtml.toc.filter((t) => t.depth === 2);
  const slug = (minor) => `release-${minor.replace(/\W+/g, "-")}`;
  return html`
    ${header(meta.path)}
    <div class="docs-layout changelog">
      <aside class="docs-sidebar">
        <nav class="docs-toc changelog-nav" aria-label="Releases">
          <p class="docs-toc-title">Latest releases</p>
          <ul>${ids.map((r) => html`<li><a href=${`#${r.id}`}>${r.text}</a></li>`)}</ul>
          ${groups.length
            ? html`<p class="docs-toc-title changelog-older-title">Older, by version</p>
              <ul class="changelog-minors">${groups.map((g) => html`<li><a href=${`#${slug(g.minor)}`}>${g.minor}</a></li>`)}</ul>`
            : ""}
        </nav>
      </aside>
      <main id="main" class="docs-main">
        <article class="docs-article">
          <h1>Changelog</h1>
          <div class="prose">${unsafeHTML(introHtml)}${unsafeHTML(recentHtml.html)}</div>
          ${groups.length ? html`<h2 class="changelog-older">Older releases</h2>` : ""}
          ${groups.map(
            (g, i) => html`<details class="changelog-group" id=${slug(g.minor)}>
              <summary><b>${g.minor}</b> <span>${g.items.length === 1 ? g.items[0].heading : `${g.items.length} releases: ${g.items.at(-1).heading.split(" ")[0]} – ${g.items[0].heading.split(" ")[0]}`}</span></summary>
              <div class="prose">${unsafeHTML(groupHtml[i].html)}</div>
            </details>`,
          )}
        </article>
      </main>
    </div>
    ${footer()}`;
}
