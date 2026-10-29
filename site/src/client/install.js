// "Install rws": opens the install panel under the button and picks the tab
// for the visitor's OS (tabs carry data-os). Both panels on the page behave
// the same.
function guessOs() {
  const p = (navigator.userAgentData?.platform ?? navigator.platform ?? "").toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("mac")) return "mac";
  if (p.includes("linux")) return "linux";
  return "";
}

export function initInstall() {
  const os = guessOs();
  for (const button of document.querySelectorAll("[data-install-toggle]")) {
    const panel = document.getElementById(button.getAttribute("aria-controls"));
    if (!panel) continue;
    const tabs = panel.querySelector("lit-material-tabs");
    const index = [...panel.querySelectorAll("lit-material-tab")].findIndex((t) => t.dataset.os === os);
    if (tabs && index > 0) {
      customElements.whenDefined("lit-material-tabs").then(() => {
        tabs.selected = index;
        tabs.dispatchEvent(new Event("change", { bubbles: true }));
      });
    }
    button.addEventListener("click", () => {
      const open = button.getAttribute("aria-expanded") !== "true";
      button.setAttribute("aria-expanded", String(open));
      panel.hidden = !open;
      if (open) requestAnimationFrame(() => panel.classList.add("open"));
      else panel.classList.remove("open");
    });
  }
}
