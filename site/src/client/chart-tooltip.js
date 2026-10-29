// One tooltip for every bar row: follows the pointer, or sits under a row
// that has keyboard focus. The value is also in each row's aria-label and in
// the tables below, so the tooltip only adds, never gates.
export function initChartTooltip() {
  const tip = document.querySelector(".chart-tooltip");
  if (!tip) return;
  const show = (row, x, y) => {
    tip.textContent = row.dataset.tip;
    tip.hidden = false;
    const r = tip.getBoundingClientRect();
    tip.style.transform = `translate(${Math.min(x + 12, innerWidth - r.width - 8)}px, ${y - r.height - 12}px)`;
  };
  for (const row of document.querySelectorAll("[data-tip]")) {
    row.addEventListener("pointermove", (e) => show(row, e.clientX, e.clientY));
    row.addEventListener("pointerleave", () => (tip.hidden = true));
    row.addEventListener("focus", () => {
      const r = row.getBoundingClientRect();
      show(row, r.left + r.width / 3, r.top);
    });
    row.addEventListener("blur", () => (tip.hidden = true));
  }
}
