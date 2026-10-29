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
import * as homePage from "../src/pages/home.js";
import * as privacyPage from "../src/pages/privacy.js";
import * as benchmarksPage from "../src/pages/benchmarks.js";
import * as playgroundPage from "../src/pages/playground.js";
import * as whyPage from "../src/pages/why.js";
import * as changelogPage from "../src/pages/changelog.js";
import * as notFoundPage from "../src/pages/not-found.js";
import { copyFileSync, cpSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { buildDocs } from "../src/docs/build.js";
import { docsVersions } from "./versions.js";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(site, "dist");
const manifest = JSON.parse(readFileSync(join(dist, ".vite", "manifest.json"), "utf8"));
const entry = manifest["src/client/main.js"];
const assets = { js: `/${entry.file}`, css: (entry.css ?? []).map((f) => `/${f}`) };

function write(path, content) {
  const file = join(dist, path);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, content);
}

const versions = docsVersions({ repo: join(site, ".."), cacheDir: join(site, ".versions") });
for (const v of versions) v.pages = new Set(["/", ...v.sidebar.flatMap((g) => g.items.map((i) => i.link))]);
const docs = [];
for (const v of versions) {
  const version = { ...v, all: versions, latestKey: versions[0].key, latestPages: versions[0].pages };
  docs.push(...(await buildDocs({ contentRoot: v.contentRoot, sidebar: v.sidebar, base: v.base, version })));
  console.log("docs", v.key, v.latest ? "(latest, working tree)" : `(tag ${v.tag})`);
}
homePage.meta.jsonLd = homePage.structuredData(versions[0].tag);
const pages = [homePage, whyPage, benchmarksPage, playgroundPage, changelogPage, privacyPage, notFoundPage, themePage, ...docs];
for (const page of pages) {
  const body = await collectResult(render(await page.render()));
  const out = page.meta.path.endsWith("/") ? `${page.meta.path}index.html` : page.meta.path;
  write(out, documentHtml({ ...page.meta, body, assets }));
}
console.log(pages.length, "pages")

write("favicon.svg", favicon());
mkdirSync(join(dist, "brand"), { recursive: true });
copyFileSync(join(site, "src", "brand", "og.png"), join(dist, "brand", "og.png"));
write("brand/mark.svg", mark({ rack: "#16120F", dot: "#F2EBE3", slot: "#7A706A", accent: "#D2551E", accentDot: "#16120F" }));
write("brand/mark-dark.svg", mark({ rack: "#F2EBE3", dot: "#16120F", slot: "#6B625C", accent: "#D2551E", accentDot: "#16120F" }));
write("brand/icon-black.svg", icon("black"));
write("brand/icon-orange.svg", icon("orange"));
console.log("brand files written");

// Generated agent files from the repo root, served at the site root as the
// release workflow does today (per-release copies come with phase 7).
const repo = join(site, "..");
for (const f of ["llms.txt", "llms-full.txt"]) copyFileSync(join(repo, f), join(dist, f));
// /<major.minor>/llms*.txt for agents pinned to a release: this tree for the
// newest, each older minor from its newest patch tag (tags before llms.txt
// existed have none).
for (const v of versions) {
  mkdirSync(join(dist, v.key), { recursive: true });
  for (const f of ["llms.txt", "llms-full.txt"]) {
    if (v.latest) copyFileSync(join(repo, f), join(dist, v.key, f));
    else {
      try {
        writeFileSync(join(dist, v.key, f), execFileSync("git", ["-C", repo, "show", `${v.tag}:${f}`]));
      } catch {
        // not in this release
      }
    }
  }
}
cpSync(join(repo, "agents"), join(dist, "agents"), { recursive: true });
