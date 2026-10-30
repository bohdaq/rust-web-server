// Requests per second over the last minutes: one series, so no legend (the
// heading names it); 2px line, recessive grid, crosshair + tooltip on hover
// and focus, and a text summary for screen readers.
import { html, svg } from "lit";

const W = 640, H = 180, PAD_L = 44, PAD_B = 22, PAD_T = 8;

function niceMax(value) {
  if (value <= 1) return 1;
  const step = 10 ** Math.floor(Math.log10(value));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * step >= value) return m * step;
  return 10 * step;
}

function fmt(rate) {
  return rate >= 100 ? rate.toFixed(0) : rate >= 10 ? rate.toFixed(1) : rate.toFixed(2);
}

export function requestChart(samples) {
  const max = niceMax(Math.max(...samples.map((s) => s.rate)));
  const t0 = samples[0].at, t1 = samples[samples.length - 1].at;
  const x = (at) => PAD_L + ((at - t0) / Math.max(1, t1 - t0)) * (W - PAD_L - 4);
  const y = (rate) => PAD_T + (1 - rate / max) * (H - PAD_T - PAD_B);
  const points = samples.map((s) => `${x(s.at).toFixed(1)},${y(s.rate).toFixed(1)}`).join(" ");
  const ticks = [0, max / 2, max];
  const last = samples[samples.length - 1];
  const peak = Math.max(...samples.map((s) => s.rate));

  const move = (event) => {
    const box = event.currentTarget.getBoundingClientRect();
    const px = ((event.clientX - box.left) / box.width) * W;
    let best = samples[0];
    for (const s of samples) if (Math.abs(x(s.at) - px) < Math.abs(x(best.at) - px)) best = s;
    show(event.currentTarget.closest(".chart"), best);
  };
  const show = (chart, s) => {
    const hover = chart.querySelector(".hover"), tip = chart.querySelector(".tip");
    hover.setAttribute("transform", `translate(${x(s.at)},0)`);
    hover.querySelector(".dot").setAttribute("cy", y(s.rate));
    hover.style.display = "";
    tip.hidden = false;
    tip.style.left = `${(x(s.at) / W) * 100}%`;
    tip.textContent = `${fmt(s.rate)} req/s · ${new Date(s.at).toLocaleTimeString()}`;
  };
  const hide = (event) => {
    const chart = event.currentTarget.closest(".chart");
    chart.querySelector(".hover").style.display = "none";
    chart.querySelector(".tip").hidden = true;
  };

  return html`
    <div class="chart">
      <svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" role="img" tabindex="0"
        aria-label="Requests per second: now ${fmt(last.rate)}, peak ${fmt(peak)} over the last ${Math.round((t1 - t0) / 1000)} seconds"
        @pointermove=${move} @pointerleave=${hide} @focus=${(e) => show(e.currentTarget.closest(".chart"), last)} @blur=${hide}>
        ${ticks.map((t) => svg`
          <line class="grid-line" x1=${PAD_L} x2=${W} y1=${y(t)} y2=${y(t)}></line>
          <text class="axis-text" x=${PAD_L - 8} y=${y(t) + 4} text-anchor="end">${fmt(t)}</text>`)}
        <polyline class="series" points=${points} vector-effect="non-scaling-stroke"></polyline>
        <g class="hover" style="display:none">
          <line class="crosshair" x1="0" x2="0" y1=${PAD_T} y2=${H - PAD_B} vector-effect="non-scaling-stroke"></line>
          <circle class="dot" r="4" cx="0" cy="0" vector-effect="non-scaling-stroke"></circle>
        </g>
      </svg>
      <div class="tip" hidden></div>
      <p class="label" style="margin: var(--space-75) 0 0">Now ${fmt(last.rate)} req/s, peak ${fmt(peak)}.</p>
    </div>`;
}
