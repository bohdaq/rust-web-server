// The docs MCP server's data (spec/WEBSITE.md "Hosting and the docs MCP
// server"): one dist/mcp/<minor>.json per docs version, written by the
// prerender from the same Markdown the site renders, plus dist/mcp/index.json
// (the versions and the changelog by release). site/mcp/ loads them.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import GithubSlugger from "github-slugger";
import { markdownTables } from "../src/charts/benchmark-data.js";
import { splitFrontmatter } from "../src/docs/markdown.js";

const SITE = "https://rws8.tech";

// Sections at ## and ###, with the same ids rehype-slug gives the headings.
// Fenced code is kept whole; headings inside fences don't split.
function sections(body) {
  const slugger = new GithubSlugger();
  const out = [{ id: "", heading: "", depth: 1, lines: [] }];
  let fence = null;
  for (const line of body.split("\n")) {
    const f = line.match(/^\s*(```+|~~~+)/);
    if (f) fence = fence && f[1].startsWith(fence) ? null : (fence ?? f[1]);
    const h = !fence && line.match(/^(#{2,3})\s+(.*?)\s*#*\s*$/);
    if (h) {
      const heading = h[2].replace(/`/g, "");
      out.push({ id: slugger.slug(heading), heading, depth: h[1].length, lines: [] });
    } else {
      out.at(-1).lines.push(line);
    }
  }
  return out
    .map(({ lines, ...s }) => ({ ...s, text: lines.join("\n").trim() }))
    .filter((s) => s.text || s.heading);
}

const cell = (s) => s.replace(/`/g, "").replace(/\[([^\]]+)\]\([^)]+\)/g, "$1").trim();

// configuration/env-vars.md: | Variable | Default | Type | Description |
function settings(read) {
  const text = read("configuration/env-vars.md");
  if (!text) return [];
  const rows = [];
  for (const table of markdownTables(text)) {
    const [head, ...body] = table;
    if (head[0] !== "Variable") continue;
    for (const r of body) {
      const env = cell(r[0]);
      if (!env.startsWith("RWS_CONFIG_")) continue;
      rows.push({ env, key: env.slice("RWS_CONFIG_".length).toLowerCase(), default: cell(r[1] ?? ""), type: cell(r[2] ?? ""), description: (r[3] ?? "").trim() });
    }
  }
  return rows;
}

// reference/api.md: | Type / Trait | Module | Description |, under ## groups.
function apiItems(read) {
  const text = read("reference/api.md");
  if (!text) return [];
  const items = [];
  let group = "";
  let table = null;
  for (const line of text.split("\n")) {
    const g = line.match(/^##\s+(.*)/);
    if (g) group = g[1].trim();
    if (!/^\|.*\|\s*$/.test(line)) {
      table = null;
      continue;
    }
    const cells = line.trim().slice(1, -1).split("|").map((c) => c.trim());
    if (cells.every((c) => /^:?-{3,}:?$/.test(c))) continue;
    if (!table) {
      table = cells;
      continue;
    }
    items.push({ name: cell(cells[0]), module: cell(cells[1] ?? ""), description: (cells[2] ?? "").trim(), group });
  }
  return items;
}

// examples/*.rs at that version, with the description from its //! header.
function examples(repo, tag) {
  let names;
  try {
    names = tag
      ? execFileSync("git", ["-C", repo, "ls-tree", "--name-only", `${tag}:examples`], { encoding: "utf8" }).split("\n").filter((n) => n.endsWith(".rs"))
      : readdirSync(join(repo, "examples")).filter((n) => n.endsWith(".rs"));
  } catch {
    return [];
  }
  return names.sort().map((file) => {
    const source = tag ? execFileSync("git", ["-C", repo, "show", `${tag}:examples/${file}`], { encoding: "utf8" }) : readFileSync(join(repo, "examples", file), "utf8");
    const doc = source.split("\n").filter((l) => l.startsWith("//!")).map((l) => l.replace(/^\/\/! ?/, "")).join("\n").trim();
    return { name: file.replace(/\.rs$/, ""), file: `examples/${file}`, description: doc, source };
  });
}

// The coding-agent guidance (agents/skills/rws/SKILL.md) at that version.
function skill(repo, tag) {
  const file = "agents/skills/rws/SKILL.md";
  try {
    return tag ? execFileSync("git", ["-C", repo, "show", `${tag}:${file}`], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }) : readFileSync(join(repo, file), "utf8");
  } catch {
    return "";
  }
}

// CHANGELOG.md split at "## <version>" headings.
export function changelog(text) {
  const releases = [];
  for (const part of text.split(/\n(?=## )/)) {
    const m = part.match(/^## (\d+\.\d+\.\d+)(?:\s*[—-]\s*(.*))?\n([\s\S]*)$/);
    if (m) releases.push({ version: m[1], title: (m[2] ?? "").trim(), markdown: m[3].trim() });
  }
  return releases;
}

export function versionIndex({ version, repo }) {
  const read = (rel) => {
    for (const ext of [".md", ".mdx"]) {
      const f = join(version.contentRoot, rel.replace(/\.md$/, "") + ext);
      if (existsSync(f)) return readFileSync(f, "utf8");
    }
    return null;
  };
  const pages = [];
  for (const group of version.sidebar) {
    for (const item of group.items) {
      const rel = item.link.replace(/^\/|\/$/g, "");
      const source = read(`${rel}.md`);
      if (!source) continue;
      const { meta, body } = splitFrontmatter(source);
      pages.push({
        path: rel,
        group: group.label,
        title: meta.title ?? item.label,
        description: meta.description ?? "",
        url: `${SITE}${version.base}${item.link}`,
        markdown: body.trim(),
        sections: sections(body),
      });
    }
  }
  return {
    version: version.key,
    tag: version.tag,
    latest: Boolean(version.latest),
    pages,
    settings: settings(read),
    api: apiItems(read),
    examples: examples(repo, version.latest ? null : version.tag),
    skill: skill(repo, version.latest ? null : version.tag),
  };
}
