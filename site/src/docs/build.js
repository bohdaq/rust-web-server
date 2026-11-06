// Reads one docs tree (a sidebar.json plus its Markdown pages) and renders
// every page. A version is just another tree under another base, so phase 3's
// per-release builds call this once per release.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { docsIndex, docsPage } from "./layout.js";
import { renderMarkdown } from "./markdown.js";

// Older versions are noindex, with the canonical on the latest copy of the
// same page when it still exists.
function versionMeta(version, rel) {
  if (version.latest) return {};
  return { noindex: true, canonical: version.latestPages.has(rel) ? `/docs${rel}` : undefined };
}

// When a page last changed: its file's last commit (the latest version only;
// older versions are noindex). Null without git history.
function lastModified(contentRoot, file) {
  try {
    const out = execFileSync("git", ["-C", contentRoot, "log", "-1", "--format=%cI", "--", file], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
    return out || null;
  } catch {
    return null;
  }
}

// Structured data for a docs page: the article and where it sits.
function pageJsonLd(page, base, url, modified) {
  const article = {
    "@context": "https://schema.org",
    "@type": "TechArticle",
    headline: page.title,
    description: page.description || page.title,
    url,
    inLanguage: "en",
    isPartOf: { "@type": "WebSite", name: "rws", url: "https://rws8.tech/" },
    author: { "@type": "Organization", name: "rust-web-server", url: "https://rws8.tech/" },
    publisher: { "@type": "Organization", name: "rust-web-server", url: "https://rws8.tech/", logo: { "@type": "ImageObject", url: "https://rws8.tech/brand/icon-orange.svg" } },
    about: { "@type": "SoftwareApplication", name: "rust-web-server (rws)", url: "https://rws8.tech/" },
  };
  if (modified) article.dateModified = modified;
  const crumbs = [
    { name: "rws", url: "https://rws8.tech/" },
    { name: "Docs", url: `https://rws8.tech${base}/` },
    { name: page.group },
    { name: page.title, url },
  ];
  const breadcrumb = {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement: crumbs.map((c, i) => ({ "@type": "ListItem", position: i + 1, name: c.name, ...(c.url ? { item: c.url } : {}) })),
  };
  return [article, breadcrumb];
}

export async function buildDocs({ contentRoot, sidebar, base = "/docs", version }) {
  const pages = new Map();
  for (const group of sidebar) {
    for (const item of group.items) {
      const rel = item.link.replace(/^\/|\/$/g, "");
      const file = [".md", ".mdx"].map((ext) => rel + ext).find((f) => existsSync(join(contentRoot, f)));
      if (!file) throw new Error(`sidebar entry ${item.link} has no page in ${contentRoot}`);
      const rendered = await renderMarkdown(readFileSync(join(contentRoot, file), "utf8"), { base });
      const modified = version.latest ? lastModified(contentRoot, file) : null;
      pages.set(base + item.link, { ...rendered, title: rendered.title || item.label, path: base + item.link, file, label: item.label, group: group.label, modified, source: join(contentRoot, file) });
    }
  }

  const order = [...pages.values()];
  const out = order.map((page, i) => ({
    page,
    meta: {
      path: page.path,
      title: `${page.title} · rws ${version.latest ? "docs" : `${version.key} docs`}`,
      description: page.description || page.title,
      ogType: "article",
      ...(version.latest
        ? { jsonLd: pageJsonLd(page, base, `https://rws8.tech${page.path}`, page.modified), markdown: page.path.replace(/\/$/, ".md"), modified: page.modified }
        : {}),
      ...versionMeta(version, page.path.slice(base.length)),
    },
    render: () =>
      docsPage({
        page,
        sidebar,
        base,
        version,
        prev: order[i - 1] && { href: order[i - 1].path, label: order[i - 1].label },
        next: order[i + 1] && { href: order[i + 1].path, label: order[i + 1].label },
      }),
  }));
  out.unshift({
    meta: {
      path: `${base}/`,
      title: version.latest ? "Documentation · rws" : `Documentation for ${version.key} · rws`,
      description: "rust-web-server documentation: configuration, proxy, building apps, MCP, database and deployment.",
      ...versionMeta(version, "/"),
    },
    render: () => docsIndex({ path: `${base}/`, sidebar, base, pages, version }),
  });
  return out;
}
