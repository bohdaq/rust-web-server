// Docs versions: the working tree is the latest (the release workflow builds
// from the tag's checkout), and every older minor of the current major is
// built from its newest patch tag, extracted with `git archive` into
// site/.versions/<tag>/ (cached; tags don't change).
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

function git(repo, args, options = {}) {
  return execFileSync("git", ["-C", repo, ...args], { encoding: "utf8", ...options });
}

// Tags made before docs/sidebar.json existed keep the sidebar inside
// docs/astro.config.mjs. Take the array literal after `sidebar:` by bracket
// matching (plain data: object and array literals of strings and booleans)
// and evaluate just that.
export function sidebarFromAstroConfig(text) {
  const start = text.indexOf("[", text.indexOf("sidebar:"));
  if (start < 0) throw new Error("no sidebar in astro.config.mjs");
  let depth = 0;
  let quote = null;
  for (let i = start; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === "\\") i++;
      else if (c === quote) quote = null;
    } else if (c === "'" || c === '"' || c === "`") quote = c;
    else if (c === "[") depth++;
    else if (c === "]" && --depth === 0) {
      return new Function(`return (${text.slice(start, i + 1)});`)();
    }
  }
  throw new Error("unbalanced sidebar in astro.config.mjs");
}

export function docsVersions({ repo, cacheDir }) {
  const current = readFileSync(join(repo, "Cargo.toml"), "utf8").match(/^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"/m);
  const [major, minor] = [Number(current[1]), Number(current[2])];
  const newest = new Map();
  for (const tag of git(repo, ["tag", "-l", `${major}.*`]).split("\n").filter(Boolean)) {
    const m = tag.match(/^(\d+)\.(\d+)\.(\d+)$/);
    if (!m || Number(m[2]) >= minor) continue;
    const key = `${m[1]}.${m[2]}`;
    const best = newest.get(key);
    if (!best || Number(m[3]) > best.patch) newest.set(key, { tag, patch: Number(m[3]) });
  }

  const older = [...newest.entries()]
    .sort(([a], [b]) => Number(b.split(".")[1]) - Number(a.split(".")[1]))
    .map(([key, { tag }]) => {
      const dir = join(cacheDir, tag);
      if (!existsSync(join(dir, "docs"))) {
        mkdirSync(dir, { recursive: true });
        // The sidebar moved from astro.config.mjs to sidebar.json in 18.8;
        // `git archive` fails on a path the tag doesn't have.
        const paths = ["docs/src/content/docs", "docs/sidebar.json", "docs/astro.config.mjs"].filter((path) => {
          try {
            execFileSync("git", ["-C", repo, "cat-file", "-e", `${tag}:${path}`], { stdio: "ignore" });
            return true;
          } catch {
            return false;
          }
        });
        const archive = execFileSync("git", ["-C", repo, "archive", tag, ...paths]);
        execFileSync("tar", ["-x", "-C", dir], { input: archive });
      }
      const json = join(dir, "docs", "sidebar.json");
      const sidebar = existsSync(json)
        ? JSON.parse(readFileSync(json, "utf8"))
        : sidebarFromAstroConfig(readFileSync(join(dir, "docs", "astro.config.mjs"), "utf8"));
      return { key, tag, base: `/docs/${key}`, contentRoot: join(dir, "docs", "src", "content", "docs"), sidebar };
    });

  const latest = {
    key: `${major}.${minor}`,
    tag: current.slice(1).join("."),
    base: "/docs",
    contentRoot: join(repo, "docs", "src", "content", "docs"),
    sidebar: JSON.parse(readFileSync(join(repo, "docs", "sidebar.json"), "utf8")),
    latest: true,
  };
  return [latest, ...older];
}
