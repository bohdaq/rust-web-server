// Reads one docs tree (a sidebar.json plus its Markdown pages) and renders
// every page. A version is just another tree under another base, so phase 3's
// per-release builds call this once per release.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { docsIndex, docsPage } from "./layout.js";
import { renderMarkdown } from "./markdown.js";

export async function buildDocs({ contentRoot, sidebar, base = "/docs" }) {
  const pages = new Map();
  for (const group of sidebar) {
    for (const item of group.items) {
      const rel = item.link.replace(/^\/|\/$/g, "");
      const file = [".md", ".mdx"].map((ext) => rel + ext).find((f) => existsSync(join(contentRoot, f)));
      if (!file) throw new Error(`sidebar entry ${item.link} has no page in ${contentRoot}`);
      const rendered = await renderMarkdown(readFileSync(join(contentRoot, file), "utf8"), { base });
      pages.set(base + item.link, { ...rendered, title: rendered.title || item.label, path: base + item.link, file, label: item.label });
    }
  }

  const order = [...pages.values()];
  const out = order.map((page, i) => ({
    meta: { path: page.path, title: `${page.title} · rws docs`, description: page.description || page.title },
    render: () =>
      docsPage({
        page,
        sidebar,
        base,
        prev: order[i - 1] && { href: order[i - 1].path, label: order[i - 1].label },
        next: order[i + 1] && { href: order[i + 1].path, label: order[i + 1].label },
      }),
  }));
  out.unshift({
    meta: { path: `${base}/`, title: "Documentation · rws", description: "rust-web-server documentation: configuration, proxy, building apps, MCP, database and deployment." },
    render: () => docsIndex({ path: `${base}/`, sidebar, base, pages }),
  });
  return out;
}
