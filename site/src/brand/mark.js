// The rws mark, redrawn as SVG from docs/src/assets/brand/rws-brand-sheet.png:
// a three-unit server rack whose bottom unit breaks forward into shards.
// Every function returns SVG markup; colours default to CSS custom properties
// so an inline mark follows the page theme, and the build writes fixed-colour
// copies for <img>, favicons and app icons (scripts/brand.js).

const INK = "#16120F";
const CREAM = "#F2EBE3";
const ORANGE = "#D2551E";
const LINE = "#7A706A";

// Full mark, 290 × 224: two dark units with a status dot and a drive slot, the
// orange unit with a jagged front edge, six shards flying off it.
export function mark({
  rack = `var(--rws-mark-rack, ${INK})`,
  dot = `var(--rws-mark-dot, ${CREAM})`,
  slot = `var(--rws-mark-slot, ${LINE})`,
  accent = `var(--rws-mark-accent, ${ORANGE})`,
  accentDot = `var(--rws-mark-accent-dot, ${INK})`,
  title = "rust-web-server",
} = {}) {
  const shards = [
    [213, 179, 18, 12],
    [243, 161, 14, -25],
    [270, 171, 8, 30],
    [249, 192, 10, 10],
    [215, 211, 14, -20],
    [275, 141, 6, 0],
  ]
    .map(
      ([x, y, s, r]) =>
        `<rect x="${x - s / 2}" y="${y - s / 2}" width="${s}" height="${s}" transform="rotate(${r} ${x} ${y})" fill="${accent}"/>`,
    )
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 290 224" role="img" aria-label="${title}"><title>${title}</title>`
    + `<rect width="280" height="60" rx="14" fill="${rack}"/>`
    + `<rect y="82" width="280" height="60" rx="14" fill="${rack}"/>`
    + `<circle cx="36" cy="30" r="10" fill="${dot}"/><circle cx="36" cy="112" r="10" fill="${dot}"/>`
    + `<path d="M196 30H256M196 112H256" stroke="${slot}" stroke-width="8" stroke-linecap="round"/>`
    + `<path d="M14 164H188L174 180L190 194L176 210L186 224H14A14 14 0 0 1 0 210V178A14 14 0 0 1 14 164Z" fill="${accent}"/>`
    + `<circle cx="36" cy="194" r="10" fill="${accentDot}"/>`
    + shards
    + `</svg>`;
}

// Simplified mark for small sizes (app icons, favicon): bars only, two shards.
// Drawn in a 24 × 24 box so it can sit inside any tile.
function glyph(bars, accent, accentShards) {
  return `<rect x="2" y="5" width="20" height="4" rx="1.5" fill="${bars}"/>`
    + `<rect x="2" y="10.5" width="20" height="4" rx="1.5" fill="${bars}"/>`
    + `<path d="M3.5 16H15L14 17L15 18L14 19L15 20H3.5A1.5 1.5 0 0 1 2 18.5V17.5A1.5 1.5 0 0 1 3.5 16Z" fill="${accent}"/>`
    + `<rect x="16.5" y="16.5" width="2" height="2" fill="${accentShards}"/>`
    + `<rect x="19.5" y="15" width="1.4" height="1.4" fill="${accentShards}"/>`;
}

// App icon tiles from the brand sheet: "black" (cream bars, orange unit on
// near-black) and "orange" (near-black bars, cream unit on orange).
export function icon(variant = "black", size = 512) {
  const [bg, bars, accent, shards] =
    variant === "orange" ? [ORANGE, INK, CREAM, CREAM] : [INK, CREAM, ORANGE, ORANGE];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 32 32">`
    + `<rect width="32" height="32" rx="7" fill="${bg}"/>`
    + `<g transform="translate(4 3.5)">${glyph(bars, accent, shards)}</g>`
    + `</svg>`;
}

// Favicon: the black tile, switching to cream bars on a dark tile in dark
// browser chrome is unnecessary because the tile carries its own ground.
export function favicon() {
  return icon("black", 32);
}
