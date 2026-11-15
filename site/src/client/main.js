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
import { initSearch } from "./search.js";
import { initDocsUi } from "./docs-ui.js";
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
initSearch({ emptyHint: "Try a setting name like <code>acme</code> or <code>upstream</code>." });
initDocsUi();
initInstall();
initTerminal();
initReveal();
initTagline();
initChartTooltip();
initPlayground();
