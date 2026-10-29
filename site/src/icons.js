// Phosphor icons (skill B6), inlined at build time. Prerender-only: reads the
// SVG files from @phosphor-icons/core, so never import this in the browser.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// The package exports no subpath for its assets, so resolve from site/ itself.
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "node_modules", "@phosphor-icons", "core", "assets");
const cache = new Map();

export function icon(name, weight = "regular") {
  const key = `${weight}/${name}`;
  if (!cache.has(key)) {
    const file = weight === "regular" ? `${name}.svg` : `${name}-${weight}.svg`;
    const svg = readFileSync(join(root, weight, file), "utf8")
      .replace("<svg ", '<svg aria-hidden="true" focusable="false" ');
    cache.set(key, svg);
  }
  return cache.get(key);
}
