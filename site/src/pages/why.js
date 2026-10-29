// /why/: the argument for rws, and when to pick something else.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/why/",
  title: "Why rws · one binary for static files, HTTPS and the proxy",
  description: "What rws does differently from nginx, Caddy and Traefik, who it is for, and when another server is the better choice.",
};

const REASONS = [
  ["stack", "One binary, one file", "Static site, HTTPS, reverse proxy and gateway in one executable, configured in one rws.config.toml. No certbot, no cron job, no separate proxy process to keep in step."],
  ["code", "Its own HTTP stack", "HTTP/1.1 parsing, routing, middleware, WebSocket, SSE, CORS, ranges, JWT and the ACME client are written in the crate, and so is the HTTP/2 server, checked by h2spec and fuzzing. HTTP/3 uses quinn and h3; TLS uses rustls, so there is no OpenSSL."],
  ["git-merge", "Config and code share one engine", "Everything the TOML file does is a type you can use from Rust. Moving from configuration to code doesn't mean moving to another tool."],
  ["test-tube", "Tested against the real thing", "CI fuzzes every parser that sees untrusted input, runs h2spec, the Gateway API conformance suite and the ACME flow against Let's Encrypt's test CA on every push."],
  ["gauge", "Fast, with the numbers public", "Up to 5.2 times Caddy's throughput and less CPU per request than nginx in every published scenario, from CI runs anyone can open and rerun."],
  ["seal-check", "Semantic versioning", "Breaking changes only in a major version, each with a migration guide."],
];

const FIT = [
  ["You want HTTPS and a proxy without a stack of tools", "One process gets certificates, serves files and balances your apps."],
  ["You run Rust services", "Start with config, and when you need code, the same engine is a library."],
  ["You want a Kubernetes gateway that is also a plain server", "The Gateway API controller is the same binary you run on a VM."],
];

const ELSEWHERE = [
  ["Lowest memory and decades of operations knowledge", "nginx. It uses less memory than rws in every benchmark scenario, and almost every problem you can hit with it has been written up."],
  ["A large plugin ecosystem with automatic HTTPS", "Caddy. Its modules cover DNS providers, auth systems and storage backends rws does not."],
  ["Routing discovered from Docker labels", "Traefik. Containers announce their own routes; rws reads routes from its file or the Gateway API."],
  ["A service mesh", "Istio or Linkerd. rws is a gateway and a server, not a sidecar."],
];

export function render() {
  return html`
    ${header(meta.path)}
    <main id="main" class="container why">
      <div class="why-intro">
        <h1>Why rws</h1>
        <p class="lead">Most web stacks are four tools in a trench coat: a server, a certificate client, a scheduler to renew, and a proxy. rws is one binary that does all four, and a Rust library when you need more.</p>
      </div>

      <section class="tiles why-reasons" aria-label="Reasons">
        ${REASONS.map(([name, title, body]) => html`<div class="tile"><span class="tile-icon">${unsafeHTML(icon(name))}</span><h2>${title}</h2><p>${body}</p></div>`)}
      </section>

      <section class="why-section" aria-labelledby="fit-title">
        <h2 id="fit-title">rws fits when</h2>
        <ul class="why-list">${FIT.map(([t, b]) => html`<li>${unsafeHTML(icon("check-circle"))}<span><b>${t}.</b> ${b}</span></li>`)}</ul>
      </section>

      <section class="why-section" aria-labelledby="else-title">
        <h2 id="else-title">Pick something else when you need</h2>
        <ul class="why-list">${ELSEWHERE.map(([t, b]) => html`<li>${unsafeHTML(icon("arrow-bend-down-right"))}<span><b>${t}:</b> ${b}</span></li>`)}</ul>
      </section>

      <section class="why-section" aria-labelledby="next-title">
        <h2 id="next-title">See for yourself</h2>
        <div class="install">
          <a class="button-filled" href="/docs/getting-started/quick-start/">Start the quick start</a>
          <a class="text-link" href="/benchmarks/">Read the benchmarks ${unsafeHTML(icon("arrow-right"))}</a>
          <a class="text-link" href="/playground/">Build a config ${unsafeHTML(icon("arrow-right"))}</a>
        </div>
      </section>
    </main>
    ${footer()}`;
}
