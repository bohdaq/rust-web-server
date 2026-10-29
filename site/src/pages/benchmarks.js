// /benchmarks/: the published numbers as charts, then the full tables and
// method from docs/reference/benchmarks.md. Data: src/charts/benchmark-data.js.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { barChart } from "../charts/bars.js";
import { gatewayScenarios, webServerScenarios } from "../charts/benchmark-data.js";
import { renderMarkdown } from "../docs/markdown.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/benchmarks/",
  title: "Benchmarks · rws against nginx, Caddy and five Gateway API implementations",
  description:
    "Requests per second, CPU per request and memory for rws, nginx and Caddy, and for rws against kgateway, Istio, Traefik, Envoy Gateway and NGINX Gateway Fabric. Method and reproduction steps included.",
};

const DOCS = join(import.meta.dirname, "..", "..", "..", "docs", "src", "content", "docs");

function grid(scenarios, metric, options) {
  return html`<div class="chart-grid">
    ${scenarios.map((s) =>
      barChart({
        title: s.name,
        rows: s.servers.map((x) => ({ name: x.name, value: x[metric] })),
        note: s.servers.some((x) => x[metric] == null) ? "NGINX Gateway Fabric answered 404 through the rewriting route, so it is left out." : undefined,
        ...options,
      }),
    )}
  </div>`;
}

export async function render() {
  const web = webServerScenarios(DOCS);
  const gw = gatewayScenarios(DOCS);
  const source = readFileSync(join(DOCS, "reference", "benchmarks.md"), "utf8");
  const tables = await renderMarkdown(source);
  return html`
    ${header(meta.path)}
    <main id="main" class="container bench">
      <section class="bench-intro">
        <h1>Benchmarks</h1>
        <p class="lead">Every number here comes from a public CI run you can open and rerun. Compare servers within one run: shared runners vary threefold between runs.</p>
        <ul class="bench-facts">
          <li>${unsafeHTML(icon("cpu"))}<span><b>Web servers:</b> one 4 CPU GitHub Actions runner, <code>oha</code> with 64 keep-alive connections, 20 s per scenario, no compression, access logs off. <a href="https://github.com/bohdaq/rust-web-server/actions/runs/40243377254">Run 40243377254</a></span></li>
          <li>${unsafeHTML(icon("cube"))}<span><b>Gateway API:</b> one kind cluster, the same nginx backend, chart defaults with one data plane replica each. <a href="https://github.com/bohdaq/rust-web-server/actions/runs/40681376798">Run 40681376798</a></span></li>
          <li>${unsafeHTML(icon("arrows-clockwise"))}<span><b>Reproduce:</b> <a href="https://github.com/bohdaq/rust-web-server/blob/main/bench/run.sh"><code>bench/run.sh</code></a> and <a href="https://github.com/bohdaq/rust-web-server/blob/main/bench/gateway/run.sh"><code>bench/gateway/run.sh</code></a>, or the manual workflows.</span></li>
        </ul>
      </section>

      <section class="bench-section" aria-labelledby="web-title">
        <h2 id="web-title">rws, nginx and Caddy</h2>
        <h3>Requests per second</h3>
        ${grid(web, "rps", { unit: "req/s", better: "higher" })}
        <h3>CPU per request</h3>
        ${grid(web, "cpu", { unit: "µs", better: "lower", digits: 1 })}
        <h3>Peak memory</h3>
        <p class="bench-note">nginx uses the least memory in every scenario. Release builds of rws use the mimalloc allocator: less CPU, more memory; a build without it peaks around 9 MiB.</p>
        ${grid(web, "memory", { unit: "MiB", better: "lower", digits: 1 })}
      </section>

      <section class="bench-section" aria-labelledby="gw-title">
        <h2 id="gw-title">Gateway API implementations</h2>
        <p class="bench-note">Envoy Gateway v1.9.2, NGINX Gateway Fabric 2.7.2, Traefik (chart 41.7.1), kgateway v2.4.6, Istio 1.30.5, rws 18.8.1. More in <a href="/docs/deployment/kubernetes-gateway-api/#benchmarks">the Gateway API docs</a>, including what this does not measure.</p>
        <h3>Requests per second</h3>
        ${grid(gw, "rps", { unit: "req/s", better: "higher" })}
        <h3>CPU per request</h3>
        ${grid(gw, "cpu", { unit: "µs", better: "lower" })}
      </section>

      <section class="bench-section prose" aria-labelledby="all-title">
        <h2 id="all-title">All numbers and the method</h2>
        ${unsafeHTML(tables.html)}
      </section>
    </main>
    <div class="chart-tooltip" role="tooltip" hidden></div>
    ${footer()}`;
}
