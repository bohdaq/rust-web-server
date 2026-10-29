// lit-material-tabs leaves panels to the page: a [data-tab-group] holds the
// tabs and its [data-tab-panel] siblings, shown by index on `change`.
export function initTabPanels() {
  for (const group of document.querySelectorAll("[data-tab-group]")) {
    const tabs = group.querySelector("lit-material-tabs");
    const panels = [...group.querySelectorAll("[data-tab-panel]")];
    const show = () => {
      const index = Number(tabs.selected ?? 0);
      panels.forEach((panel, i) => (panel.hidden = i !== index));
    };
    tabs.addEventListener("change", show);
    show();
  }
}
