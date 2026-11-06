// The document around every prerendered page: head, header, footer.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { mark } from "./brand/mark.js";
import { icon } from "./icons.js";
import { themeBootScript } from "@lit-material/site-kit/client/theme.js";

export const NAV = [
  ["/", "Home"],
  ["/why/", "Why"],
  ["/benchmarks/", "Benchmarks"],
  ["/playground/", "Playground"],
  ["/docs/", "Docs"],
];

function links(path) {
  return html`${NAV.slice(1).map(
    ([href, label]) => html`<a href=${href} aria-current=${path.startsWith(href) ? "page" : "false"}>${label}</a>`,
  )}<a href="https://github.com/bohdaq/rust-web-server">GitHub</a>`;
}

export function header(path) {
  return html`
    <header class="site-header">
      <nav aria-label="Main">
        <a class="lockup" href="/" aria-label="rust-web-server home">
          ${unsafeHTML(mark({ title: "rust-web-server" }))}
          <span class="word"><b>rust-web-server</b><small>RWS</small></span>
        </a>
        <span class="nav-links">${links(path)}</span>
        <button class="icon-button" type="button" data-theme-toggle aria-pressed="false" aria-label="Switch theme">
          <span class="sun">${unsafeHTML(icon("sun"))}</span>
          <span class="moon">${unsafeHTML(icon("moon"))}</span>
        </button>
        <button class="icon-button menu-toggle" type="button" data-menu-toggle aria-expanded="false" aria-controls="mobile-menu" aria-label="Open menu">
          <span class="menu-icon" aria-hidden="true"><span></span><span></span></span>
        </button>
      </nav>
      <div id="mobile-menu" class="mobile-menu" hidden>
        <nav aria-label="Main">${links(path)}</nav>
      </div>
    </header>`;
}

export function footer() {
  return html`
    <footer class="site-footer">
      <div class="container">
        <ul>
          <li><a href="/docs/">Docs</a></li>
          <li><a href="https://github.com/bohdaq/rust-web-server">GitHub</a></li>
          <li><a href="https://crates.io/crates/rust-web-server">crates.io</a></li>
          <li><a href="https://hub.docker.com/r/bohdaq/rws">Docker Hub</a></li>
          <li><a href="/changelog/">Changelog</a></li>
          <li><a href="/llms.txt">llms.txt</a></li>
          <li><a href="/privacy/">Privacy</a></li>
          <li><a href="https://github.com/bohdaq/rust-web-server/blob/main/LICENSE">MIT license</a></li>
        </ul>
      </div>
    </footer>`;
}

// Applies a saved theme before first paint, so a dark-mode visitor never sees
// a cream flash. Storage can throw (private windows), hence the try.
const THEME_BOOT = themeBootScript("rws-theme");

export const SITE = "https://rws8.tech";

const attr = (s) => String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

export function documentHtml({ title, description, path, body, assets, noindex = false, canonical = path, jsonLd = [], ogType = "website", markdown = null, modified = null }) {
  const css = assets.css.map((href) => `<link rel="stylesheet" href="${href}">`).join("");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title}</title>
<meta name="description" content="${description}">
${noindex ? '<meta name="robots" content="noindex">' : ""}
${canonical ? `<link rel="canonical" href="${SITE}${canonical}">` : ""}
${markdown ? `<link rel="alternate" type="text/markdown" href="${markdown}" title="This page as Markdown">` : ""}
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<meta property="og:type" content="${ogType}">
<meta property="og:site_name" content="rws">
<meta property="og:url" content="${SITE}${canonical ?? path}">
<meta property="og:title" content="${title}">
<meta property="og:description" content="${description}">
<meta property="og:image" content="${SITE}/brand/og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
${modified ? `<meta property="article:modified_time" content="${attr(modified)}">` : ""}
<meta name="theme-color" content="#16120F">
<script>${THEME_BOOT}</script>
${jsonLd.map((d) => `<script type="application/ld+json">${JSON.stringify(d).replace(/</g, "\\u003c")}</script>`).join("")}
${css}
<script type="module" src="${assets.js}"></script>
<script defer src="/_rws/t.js"></script>
</head>
<body>
<a class="skip-link" href="#main">Skip to content</a>
${body}
</body>
</html>
`;
}
