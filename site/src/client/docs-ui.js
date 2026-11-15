// Small behaviours of the docs pages: the version menu closes on Escape, an
// outside click or a choice; open code-block wrap toggles remember nothing.
export function initDocsUi() {
  for (const menu of document.querySelectorAll("details.version-switch")) {
    document.addEventListener("click", (e) => {
      if (menu.open && !menu.contains(e.target)) menu.open = false;
    });
    menu.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && menu.open) {
        menu.open = false;
        menu.querySelector("summary")?.focus();
      }
    });
  }
}
