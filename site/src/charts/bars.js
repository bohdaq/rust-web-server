// Horizontal bar chart in plain HTML (dataviz skill: emphasis form). The
// highlighted entity gets the accent, the rest the de-emphasis grey; only the
// highlighted bar carries a direct label, the others are in the tooltip and
// in the table view below the charts. Every row is focusable for keyboards.
import { html, nothing } from "lit";

const fmt = (n, digits = 0) => n.toLocaleString("en-US", { maximumFractionDigits: digits });

export function barChart({ title, unit, better, rows, highlight = "rws", digits = 0, note }) {
  const shown = rows.filter((r) => r.value != null);
  const max = Math.max(...shown.map((r) => r.value));
  return html`
    <figure class="chart">
      <figcaption>
        <span class="chart-title">${title}</span>
        <span class="chart-better">${better === "higher" ? "Higher is better" : "Lower is better"}</span>
      </figcaption>
      <div class="bars" role="list">
        ${shown.map((r) => {
          const value = `${fmt(r.value, digits)} ${unit}`;
          const mine = r.name === highlight;
          return html`
            <div class=${mine ? "bar-row is-highlight" : "bar-row"} role="listitem" tabindex="0" aria-label=${`${r.name}: ${value}`} data-tip=${`${r.name} · ${value}`}>
              <span class="bar-name">${r.name}</span>
              <span class="bar-track" style=${`--f: ${(r.value / max).toFixed(4)}`}><span class="bar"></span>${mine ? html`<span class="bar-value">${value}</span>` : nothing}</span>
            </div>`;
        })}
      </div>
      ${note ? html`<p class="chart-note">${note}</p>` : nothing}
    </figure>`;
}
