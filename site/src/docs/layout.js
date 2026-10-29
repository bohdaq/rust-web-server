// Docs pages: sidebar from docs/sidebar.json, the rendered article, an on-page
// table of contents, prev/next and an edit link. Prerender only.
import { html, nothing } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

const REPO = "https://github.com/bohdaq/rust-web-server";

function sidebarNav(sidebar, current, base) {
  return html`
    <nav class="docs-nav" aria-label="Docs">
      <button class="docs-search-button" type="button" data-search-open>
        ${unsafeHTML(icon("magnifying-glass"))}<span>Search docs</span><kbd>/</kbd>
      </button>
      ${sidebar.map((group) => {
        const here = group.items.some((i) => base + i.link === current);
        return html`
          <details class="docs-group" ?open=${!group.collapsed || here}>
            <summary>${group.label}</summary>
            <ul>
              ${group.items.map(
                (item) => html`<li><a href=${base + item.link} aria-current=${base + item.link === current ? "page" : "false"}>${item.label}</a></li>`,
              )}
            </ul>
          </details>`;
      })}
    </nav>`;
}

function tocNav(toc) {
  if (toc.length < 2) return nothing;
  return html`
    <nav class="docs-toc" aria-label="On this page">
      <p class="docs-toc-title">On this page</p>
      <ul>
        ${toc.map((t) => html`<li class=${`depth-${t.depth}`}><a href=${`#${t.id}`}>${t.text}</a></li>`)}
      </ul>
    </nav>`;
}

function pager(prev, next) {
  return html`
    <nav class="docs-pager" aria-label="Previous and next page">
      ${prev ? html`<a class="prev" href=${prev.href}><small>Previous</small><span>${prev.label}</span></a>` : html`<span></span>`}
      ${next ? html`<a class="next" href=${next.href}><small>Next</small><span>${next.label}</span></a>` : nothing}
    </nav>`;
}

function shell({ path, sidebar, base, content, toc = [] }) {
  return html`
    ${header(path)}
    <div class="docs-layout">
      <aside class="docs-sidebar">
        <details class="docs-nav-mobile">
          <summary>${unsafeHTML(icon("list"))} Docs menu</summary>
          ${sidebarNav(sidebar, path, base)}
        </details>
        <div class="docs-nav-desktop">${sidebarNav(sidebar, path, base)}</div>
      </aside>
      <main id="main" class="docs-main">${content}</main>
      <aside class="docs-aside">${tocNav(toc)}</aside>
    </div>
    <dialog class="search-dialog" aria-label="Search docs">
      <form method="dialog" class="search-form" role="search">
        ${unsafeHTML(icon("magnifying-glass"))}
        <input type="search" name="q" placeholder="Search the docs" autocomplete="off" aria-label="Search the docs" />
        <button class="icon-button" value="close" aria-label="Close search">${unsafeHTML(icon("x"))}</button>
      </form>
      <div class="search-results" aria-live="polite"></div>
    </dialog>
    ${footer()}`;
}

export function docsPage({ page, sidebar, base, prev, next }) {
  return shell({
    path: page.path,
    sidebar,
    base,
    toc: page.toc,
    content: html`
      <article class="docs-article" data-pagefind-body>
        <h1>${page.title}</h1>
        ${page.description ? html`<p class="docs-lead">${page.description}</p>` : nothing}
        <div class="prose">${unsafeHTML(page.html)}</div>
      </article>
      <p class="docs-edit">
        <a href=${`${REPO}/edit/main/docs/src/content/docs/${page.file}`}>${unsafeHTML(icon("pencil-simple"))} Edit this page on GitHub</a>
      </p>
      ${pager(prev, next)}`,
  });
}

export function docsIndex({ path, sidebar, base, pages }) {
  return shell({
    path,
    sidebar,
    base,
    content: html`
      <article class="docs-article">
        <h1>Documentation</h1>
        <p class="docs-lead">Run rws from one config file, or build on it as a Rust crate. Start with the quick start, then pick the part you need.</p>
        <div class="docs-index">
          ${sidebar.map(
            (group) => html`
              <section>
                <h2>${group.label}</h2>
                <ul>
                  ${group.items.map((item) => {
                    const p = pages.get(base + item.link);
                    return html`<li><a href=${base + item.link}>${item.label}</a>${p?.description ? html`<span>${p.description}</span>` : nothing}</li>`;
                  })}
                </ul>
              </section>`,
          )}
        </div>
      </article>`,
  });
}
