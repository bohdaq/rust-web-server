// Phosphor icons (landing-page-design B6), inlined as strings at build time:
// only the ones the app uses, registered by name with @lit-material/icon.
import { registerIcons } from "@lit-material/icon";

import gauge from "../node_modules/@phosphor-icons/core/assets/regular/gauge.svg?raw";
import chart_line from "../node_modules/@phosphor-icons/core/assets/regular/chart-line.svg?raw";
import tree_structure from "../node_modules/@phosphor-icons/core/assets/regular/tree-structure.svg?raw";
import certificate from "../node_modules/@phosphor-icons/core/assets/regular/certificate.svg?raw";
import sliders_horizontal from "../node_modules/@phosphor-icons/core/assets/regular/sliders-horizontal.svg?raw";
import list_bullets from "../node_modules/@phosphor-icons/core/assets/regular/list-bullets.svg?raw";
import terminal_window from "../node_modules/@phosphor-icons/core/assets/regular/terminal-window.svg?raw";
import sign_out from "../node_modules/@phosphor-icons/core/assets/regular/sign-out.svg?raw";
import check_circle from "../node_modules/@phosphor-icons/core/assets/regular/check-circle.svg?raw";
import warning_circle from "../node_modules/@phosphor-icons/core/assets/regular/warning-circle.svg?raw";
import x_circle from "../node_modules/@phosphor-icons/core/assets/regular/x-circle.svg?raw";
import pause from "../node_modules/@phosphor-icons/core/assets/regular/pause.svg?raw";
import play from "../node_modules/@phosphor-icons/core/assets/regular/play.svg?raw";
import arrow_clockwise from "../node_modules/@phosphor-icons/core/assets/regular/arrow-clockwise.svg?raw";
import trash from "../node_modules/@phosphor-icons/core/assets/regular/trash.svg?raw";
import plus from "../node_modules/@phosphor-icons/core/assets/regular/plus.svg?raw";
import lock_key from "../node_modules/@phosphor-icons/core/assets/regular/lock-key.svg?raw";
import clipboard_text from "../node_modules/@phosphor-icons/core/assets/regular/clipboard-text.svg?raw";
import user_circle from "../node_modules/@phosphor-icons/core/assets/regular/user-circle.svg?raw";

registerIcons({
  "gauge": gauge,
  "chart-line": chart_line,
  "tree-structure": tree_structure,
  "certificate": certificate,
  "sliders-horizontal": sliders_horizontal,
  "list-bullets": list_bullets,
  "terminal-window": terminal_window,
  "sign-out": sign_out,
  "check-circle": check_circle,
  "warning-circle": warning_circle,
  "x-circle": x_circle,
  "pause": pause,
  "play": play,
  "arrow-clockwise": arrow_clockwise,
  "trash": trash,
  "plus": plus,
  "lock-key": lock_key,
  "clipboard-text": clipboard_text,
  "user-circle": user_circle,
});

/** The icon `name` as a template value (a `lit-material-icon`). */
export { icon } from "@lit-material/icon";
