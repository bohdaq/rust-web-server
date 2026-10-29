// Writes dist/<route>/index.html for every page with Lit SSR, plus the brand
// files. Runs after `vite build` (needs dist/.vite/manifest.json).
import { render } from "@lit-labs/ssr";
import { collectResult } from "@lit-labs/ssr/lib/render-result.js";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import "../src/components.js";
import { favicon, icon, mark } from "../src/brand/mark.js";
import { documentHtml } from "../src/shell.js";
import * as themePage from "../src/pages/theme.js";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const manifest = JSON.parse(readFileSync(join(dist, ".vite", "manifest.json"), "utf8"));
const entry = manifest["src/client/main.js"];
const assets = { js: `/${entry.file}`, css: (entry.css ?? []).map((f) => `/${f}`) };

function write(path, content) {
  const file = join(dist, path);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, content);
}

const pages = [themePage];
for (const page of pages) {
  const body = await collectResult(render(page.render()));
  const out = page.meta.path.endsWith("/") ? `${page.meta.path}index.html` : page.meta.path;
  write(out, documentHtml({ ...page.meta, body, assets }));
  console.log("page", page.meta.path);
}

write("favicon.svg", favicon());
write("brand/mark.svg", mark({ rack: "#16120F", dot: "#F2EBE3", slot: "#7A706A", accent: "#D2551E", accentDot: "#16120F" }));
write("brand/mark-dark.svg", mark({ rack: "#F2EBE3", dot: "#16120F", slot: "#6B625C", accent: "#D2551E", accentDot: "#16120F" }));
write("brand/icon-black.svg", icon("black"));
write("brand/icon-orange.svg", icon("orange"));
console.log("brand files written");
