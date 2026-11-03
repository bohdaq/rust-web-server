// Must load before Lit, so server-rendered shadow roots are hydrated, not re-rendered.
import "@lit-labs/ssr-client/lit-element-hydrate-support.js";

import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "@lit-material/tokens/css/index.css";
import "../theme.css";
import "../site.css";

import "../components.js";
import { initThemeToggle } from "@lit-material/site-kit/client/theme.js";
import { initMenu } from "@lit-material/site-kit/client/menu.js";
import { initSearch } from "@lit-material/site-kit/client/search.js";
import { initInstall } from "./install.js";
import { initTerminal } from "@lit-material/site-kit/client/terminal.js";
import { initReveal } from "@lit-material/site-kit/client/reveal.js";
import { initTagline } from "./reveal.js";
import "../landing.css";
import "../charts.css";
import { initChartTooltip } from "./chart-tooltip.js";
import { initPlayground } from "./playground.js";

initThemeToggle({ key: "rws-theme" });
initMenu();
initSearch({
  // Docs pages carry their version; search only that version's pages.
  options: () => {
    const version = document.querySelector("[data-docs-version]")?.dataset.docsVersion;
    return version ? { filters: { version } } : {};
  },
  emptyHint: "Try a setting name like <code>acme</code> or <code>upstream</code>.",
});
initInstall();
initTerminal();
initReveal();
initTagline();
initChartTooltip();
initPlayground();
