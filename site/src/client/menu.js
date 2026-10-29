// Mobile menu (skill B7): a full-height panel; links rise in one after another.
export function initMenu() {
  const toggle = document.querySelector("[data-menu-toggle]");
  const panel = document.getElementById("mobile-menu");
  if (!toggle || !panel) return;
  const set = (open) => {
    toggle.setAttribute("aria-expanded", String(open));
    toggle.setAttribute("aria-label", open ? "Close menu" : "Open menu");
    document.documentElement.classList.toggle("menu-open", open);
    if (open) {
      panel.hidden = false;
      requestAnimationFrame(() => panel.classList.add("open"));
    } else {
      panel.classList.remove("open");
      panel.addEventListener("transitionend", () => (panel.hidden = !panel.classList.contains("open") || panel.hidden), { once: true });
      setTimeout(() => { if (!panel.classList.contains("open")) panel.hidden = true; }, 500);
    }
  };
  toggle.addEventListener("click", () => set(toggle.getAttribute("aria-expanded") !== "true"));
  document.addEventListener("keydown", (e) => e.key === "Escape" && set(false));
  matchMedia("(min-width: 768px)").addEventListener("change", (e) => e.matches && set(false));
}
