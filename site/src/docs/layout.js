// Docs pages: sidebar from docs/sidebar.json, the rendered article, an on-page
// table of contents, prev/next and an edit link. Prerender only.
import { html, nothing } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

const REPO = "https://github.com/bohdaq/rust-web-server";

// Links to this page in every version that has it, else to that version's index.
function versionSwitch(version, current, base) {
  const rel = current.slice(base.length);
  return html`
    <details class="version-switch">
      <summary>Version <b>${version.key}</b>${version.latest ? html` <span class="latest-tag">latest</span>` : nothing}</summary>
      <ul>
        ${version.all.map(
          (v) => html`<li><a href=${v.pages.has(rel) ? v.base + rel : `${v.base}/`} aria-current=${v.key === version.key ? "page" : "false"}>${v.key}${v.latest ? " (latest)" : ""}</a></li>`,
        )}
      </ul>
    </details>`;
}

function sidebarNav(sidebar, current, base, version) {
  return html`
    <nav class="docs-nav" aria-label="Docs">
      ${versionSwitch(version, current, base)}
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

function oldVersionNotice(version, path, base) {
  if (version.latest) return nothing;
  const rel = path.slice(base.length);
  const target = version.latestPages.has(rel) ? `/docs${rel}` : "/docs/";
  return html`
    <lit-material-alert variant="info" class="version-notice">
      <span slot="icon">${unsafeHTML(icon("clock-counter-clockwise"))}</span>
      <span slot="title">You are reading the docs for rws ${version.key}</span>
      The latest release is ${version.latestKey}. <a href=${target}>${version.latestPages.has(rel) ? "Read this page for the latest release" : "Go to the latest docs"}</a>.
    </lit-material-alert>`;
}

function shell({ path, sidebar, base, content, toc = [], version }) {
  return html`
    ${header(path)}
    <div class="docs-layout">
      <aside class="docs-sidebar">
        <details class="docs-nav-mobile">
          <summary>${unsafeHTML(icon("list"))} Docs menu</summary>
          ${sidebarNav(sidebar, path, base, version)}
        </details>
        <div class="docs-nav-desktop">${sidebarNav(sidebar, path, base, version)}</div>
      </aside>
      <main id="main" class="docs-main" data-docs-version=${version.key}>${oldVersionNotice(version, path, base)}${content}</main>
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

export function docsPage({ page, sidebar, base, prev, next, version }) {
  return shell({
    path: page.path,
    sidebar,
    base,
    version,
    toc: page.toc,
    content: html`
      <article class="docs-article" data-pagefind-body data-pagefind-filter=${`version:${version.key}`}>
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

export function docsIndex({ path, sidebar, base, pages, version }) {
  return shell({
    path,
    sidebar,
    base,
    version,
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
