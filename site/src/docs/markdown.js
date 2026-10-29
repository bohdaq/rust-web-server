// Markdown → HTML for docs pages (prerender only). The source is the Starlight
// content in docs/src/content/docs/, unchanged: frontmatter title/description,
// GitHub-flavoured Markdown, `:::note[Title]` asides and fenced code with an
// optional `title="file"`. Site-root links (/features/acme/) are rebased under
// the docs route (/docs/features/acme/ or /docs/18.7/features/acme/).
import rehypeShiki from "@shikijs/rehype";
import { toString } from "hast-util-to-string";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import rehypeStringify from "rehype-stringify";
import remarkDirective from "remark-directive";
import remarkGfm from "remark-gfm";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import { unified } from "unified";
import { visit } from "unist-util-visit";
import { parse as parseYaml } from "yaml";
import { icon } from "../icons.js";

const ASIDES = {
  note: { variant: "info", title: "Note", icon: "info" },
  tip: { variant: "success", title: "Tip", icon: "lightbulb" },
  caution: { variant: "warning", title: "Caution", icon: "warning" },
  danger: { variant: "error", title: "Danger", icon: "warning-octagon" },
};

// Fence languages Shiki has no grammar for, or spelled differently.
const LANGUAGE_ALIASES = { promql: "text", "rust,no_run": "rust", sh: "bash" };

export function splitFrontmatter(source) {
  const m = source.match(/^---\n([\s\S]*?)\n---\n/);
  if (!m) return { meta: {}, body: source };
  return { meta: parseYaml(m[1]) ?? {}, body: source.slice(m[0].length) };
}

// :::note[Title] … ::: → <lit-material-alert variant=…><span slot=title>…
function remarkAsides() {
  return (tree) => {
    visit(tree, "containerDirective", (node) => {
      const aside = ASIDES[node.name];
      if (!aside) return;
      let title = aside.title;
      const first = node.children[0];
      if (first?.data?.directiveLabel) {
        title = first.children.map((c) => c.value ?? "").join("") || title;
        node.children.shift();
      }
      node.data = {
        hName: "lit-material-alert",
        hProperties: { variant: aside.variant },
      };
      node.children.unshift(
        { type: "paragraph", data: { hName: "span", hProperties: { slot: "icon", dataIcon: aside.icon } }, children: [] },
        { type: "paragraph", data: { hName: "span", hProperties: { slot: "title" } }, children: [{ type: "text", value: title }] },
      );
    });
    // Leaf/text directives are not used by the docs; keep their text.
    visit(tree, ["leafDirective", "textDirective"], (node, index, parent) => {
      parent.children.splice(index, 1, { type: "text", value: `:${node.name}` });
    });
    // The docs have no intended raw HTML; what parses as HTML is prose like
    // `Arc<Mutex<T>>` in a heading, which Starlight silently dropped. Show it.
    visit(tree, "html", (node) => {
      node.type = "text";
    });
  };
}

// <pre><code class="language-x"> → <lit-material-code-block label=…><pre>…,
// before Shiki highlights the <pre> in place.
function rehypeCodeBlocks() {
  return (tree) => {
    visit(tree, "element", (node, index, parent) => {
      if (node.tagName !== "pre" || !parent || parent.tagName === "lit-material-code-block") return;
      const code = node.children.find((c) => c.tagName === "code");
      if (!code) return;
      const classes = code.properties.className ?? [];
      const langClass = classes.find((c) => String(c).startsWith("language-"));
      let lang = langClass ? String(langClass).slice("language-".length) : "text";
      lang = LANGUAGE_ALIASES[lang] ?? lang.split(",")[0];
      code.properties.className = [`language-${lang}`];
      const meta = code.data?.meta ?? "";
      const title = meta.match(/title="([^"]+)"/)?.[1];
      parent.children[index] = {
        type: "element",
        tagName: "lit-material-code-block",
        properties: { label: title ?? (lang === "text" ? "" : lang), copyable: true },
        children: [node],
      };
    });
  };
}

// Site-root links point at the docs route; tables scroll inside a wrapper;
// collect h2/h3 for the on-page table of contents.
function rehypeDocsLinks({ base, toc }) {
  return (tree) => {
    visit(tree, "element", (node, index, parent) => {
      if (node.tagName === "a") {
        const href = String(node.properties.href ?? "");
        if (href.startsWith("/") && !href.startsWith("//") && !href.startsWith("/docs/")) {
          // Pages are directories: /a/b and /a/b#x mean /a/b/ and /a/b/#x.
          const [path, hash = ""] = href.split(/(?=#)/);
          const slashed = /\/$|\.[a-z0-9]+$/i.test(path) ? path : `${path}/`;
          node.properties.href = base + slashed + hash;
        }
      } else if (node.tagName === "table" && parent && parent.properties?.className?.[0] !== "table-wrap") {
        parent.children[index] = { type: "element", tagName: "div", properties: { className: ["table-wrap"] }, children: [node] };
      } else if ((node.tagName === "h2" || node.tagName === "h3") && node.properties.id) {
        toc.push({ depth: Number(node.tagName[1]), id: node.properties.id, text: toString(node).replace(/#$/, "").trim() });
      }
    });
  };
}

let highlighter;

export async function renderMarkdown(source, { base = "/docs" } = {}) {
  const { meta, body } = splitFrontmatter(source);
  const toc = [];
  highlighter ??= unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkDirective)
    .use(remarkAsides)
    .use(remarkRehype, { allowDangerousHtml: false })
    .use(rehypeSlug)
    .use(rehypeCodeBlocks)
    .use(rehypeShiki, {
      themes: { light: "github-light", dark: "github-dark-dimmed" },
      defaultColor: false,
      fallbackLanguage: "text",
    })
    .use(rehypeAutolinkHeadings, {
      behavior: "append",
      properties: { className: ["heading-anchor"], ariaHidden: "true", tabIndex: -1 },
      content: { type: "text", value: "#" },
    })
    .use(rehypeStringify)
    .freeze();
  const processor = highlighter().use(rehypeDocsLinks, { base, toc });
  let html = String(await processor.process(body));
  html = html.replace(/<span slot="icon" data-icon="([a-z-]+)"><\/span>/g, (_, name) => `<span slot="icon">${icon(name)}</span>`);
  return { title: meta.title ?? "", description: meta.description ?? "", html, toc };
}
