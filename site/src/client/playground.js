// Wires the playground form to the shared generator: every change rebuilds
// the file and the command, shows or hides dependent fields, and puts each
// error next to its field (never an alert).
import { generate } from "../playground/generate.js";

export function initPlayground() {
  const form = document.querySelector("[data-playground]");
  if (!form) return;
  const toml = document.querySelector('[data-output="toml"]');
  const command = document.querySelector('[data-output="command"]');
  const status = document.querySelector("[data-status]");
  const okHtml = status.innerHTML;

  const read = () => {
    const state = {};
    for (const el of form.elements) {
      if (!el.name) continue;
      state[el.name] = el.type === "checkbox" ? el.checked : el.value;
    }
    return state;
  };

  const update = () => {
    const state = read();
    for (const el of form.querySelectorAll("[data-when]")) el.hidden = !state[el.dataset.when];
    const out = generate(state);
    toml.textContent = out.toml;
    command.textContent = out.command;
    for (const el of form.querySelectorAll("[data-error]")) {
      const message = out.errors[el.dataset.error];
      el.hidden = !message;
      el.textContent = message ?? "";
      el.closest(".field")?.classList.toggle("has-error", Boolean(message));
      el.closest(".field")?.querySelector("input, textarea")?.setAttribute("aria-invalid", String(Boolean(message)));
    }
    const count = Object.keys(out.errors).length;
    status.classList.toggle("is-error", count > 0);
    status.innerHTML = count ? `<span>Fix ${count === 1 ? "the field" : `${count} fields`} marked in red; the file above would not pass <code>rws --check</code>.</span>` : okHtml;
  };

  form.addEventListener("input", update);
  form.addEventListener("change", update);
  form.addEventListener("submit", (e) => e.preventDefault());
  update();
}
