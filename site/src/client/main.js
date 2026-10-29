// Must load before Lit, so server-rendered shadow roots are hydrated, not re-rendered.
import "@lit-labs/ssr-client/lit-element-hydrate-support.js";

import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "@lit-material/tokens/css/index.css";
import "../theme.css";
import "../site.css";

import "../components.js";
import { initThemeToggle } from "./theme-toggle.js";
import { initTabPanels } from "./tab-panels.js";
import { initMenu } from "./menu.js";
import { initSearch } from "./search.js";
import { initInstall } from "./install.js";
import { initTerminal } from "./terminal.js";
import { initReveal } from "./reveal.js";
import "../landing.css";

initThemeToggle();
initTabPanels();
initMenu();
initSearch();
initInstall();
initTerminal();
initReveal();
