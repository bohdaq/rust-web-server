// Light / dark / system. The choice is a per-visitor convenience in
// localStorage; without it (private window, blocked storage) the page follows
// the OS. The inline script in the page head applies it before first paint.
const KEY = "rws-theme";

function read() {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function write(value) {
  try {
    if (value) localStorage.setItem(KEY, value);
    else localStorage.removeItem(KEY);
  } catch {
    // Storage unavailable: the choice lasts for this page only.
  }
}

function apply(value) {
  if (value) document.documentElement.dataset.theme = value;
  else delete document.documentElement.dataset.theme;
}

export function initThemeToggle() {
  for (const button of document.querySelectorAll("[data-theme-toggle]")) {
    const sync = () => {
      const current =
        document.documentElement.dataset.theme ??
        (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
      button.setAttribute("aria-pressed", String(current === "dark"));
      button.setAttribute("aria-label", current === "dark" ? "Switch to light theme" : "Switch to dark theme");
    };
    button.addEventListener("click", () => {
      const next = button.getAttribute("aria-pressed") === "true" ? "light" : "dark";
      apply(next);
      write(next);
      sync();
    });
    sync();
  }
  apply(read());
}
