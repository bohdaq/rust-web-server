// / — the landing page. Copy and decisions: spec/WEBSITE_LANDING.md.
import { html } from "lit";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import { renderMarkdown } from "../docs/markdown.js";
import { icon } from "../icons.js";
import { footer, header } from "../shell.js";

export const meta = {
  path: "/",
  title: "rws: web server, reverse proxy and automatic HTTPS in one binary",
  description:
    "rws serves static files, gets Let's Encrypt certificates and load balances your apps from one TOML file. HTTP/3, auth, rate limits and a Gateway API controller. MIT licensed.",
};

// The same configs the README shows: real keys, checked by `rws --check`.
const MODES = [
  {
    tab: "Static files",
    result: "Serves the folder on port 7878 with directory listings, ETags, byte ranges and compression.",
    code: "```bash\ncd my-site && rws\n```",
  },
  {
    tab: "Automatic HTTPS",
    result: "Gets certificates from Let's Encrypt on first start, serves HTTP/2 and HTTP/3, redirects HTTP and renews in the background.",
    code: `\`\`\`toml title="rws.config.toml"
port               = 443
http_redirect_port = 80

[acme]
domains = ["example.com", "www.example.com"]
email   = "you@example.com"
\`\`\``,
  },
  {
    tab: "Reverse proxy",
    result: "Balances two backends by open connections, checks their health and puts a rate limit and a token in front.",
    code: `\`\`\`toml title="rws.config.toml"
[[upstream]]
name     = "app"
backends = ["10.0.0.10:8080", "10.0.0.11:8080"]
strategy = "least_connections"

  [upstream.health_check]
  path = "/healthz"

[[route]]
  [route.match]
  host = "api.example.com"
  path = "/v1/*"

  [route.action]
  type     = "proxy"
  upstream = "app"

  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  auth       = { type = "bearer", token_env = "API_TOKEN" }
\`\`\``,
  },
];

const INSTALL = [
  { tab: "Homebrew", os: "mac", code: "```bash\nbrew install bohdaq/tap/rws\n```", note: "macOS and Linux, x86_64 and arm64." },
  { tab: "Cargo", os: "windows", code: "```bash\ncargo binstall rust-web-server\n```", note: "Prebuilt binary through cargo binstall; works on Windows too." },
  { tab: "Docker", os: "", code: '```bash\ndocker run -p 7878:7878 -v "$PWD:/srv" bohdaq/rws\n```', note: "linux/amd64 and linux/arm64, also on ghcr.io/bohdaq/rws." },
  { tab: "apt / dnf", os: "linux", code: "```bash\nsudo apt install rws    # or: sudo dnf install rws\n```", note: "After adding the repository; installs and starts the rws service.", link: ["/docs/deployment/systemd/#install-from-a-package", "Add the repository"] },
];

const TILES = [
  ["stack", "One process instead of four", "Static files, certificates, redirects and the proxy run in one binary; no certbot, cron job or sidecar.", "/docs/getting-started/features/"],
  ["certificate", "Certificates that look after themselves", "Ordered on first start, renewed at two thirds of their lifetime, reloaded without dropping connections. HTTP-01, TLS-ALPN-01 and DNS-01.", "/docs/features/acme/"],
  ["arrows-split", "Traffic you can steer", "Round robin, least connections, IP hash, weights, health checks, retries, circuit breakers, sticky sessions and canary splits.", "/docs/proxy/load-balancing/"],
  ["shield-check", "Protection on by config", "Bearer, JWT and basic auth that fail closed, rate limits, IP allow and deny lists, request size limits.", "/docs/features/auth/"],
  ["chart-line", "Answers you can measure", "Prometheus metrics with native histograms, OpenTelemetry traces and metrics, JSON access logs.", "/docs/features/metrics/"],
  ["cube", "Kubernetes without another controller", "Built with gateway-api, rws is the Gateway API controller and data plane: HTTP, gRPC, TLS, TCP and UDP routes, installed with the Helm chart.", "/docs/deployment/kubernetes-gateway-api/"],
];

const STEPS = [
  ["Install one binary", html`Homebrew, Cargo, Docker, apt or dnf. Static builds for Linux, plus macOS and Windows.`],
  ["Write one file", html`Put what you need in <code>rws.config.toml</code>. <code>rws --check</code> validates it before anything starts.`],
  ["Run rws", html`It serves and gets certificates, and reloads on <code>SIGHUP</code>, or by itself when the file changes with <code>watch = true</code>.`],
];

const STATS = [
  ["96,670", "req/s", "1 KiB static file. nginx: 81,161. Caddy: 18,592."],
  ["16.9", "µs", "CPU per static request. nginx: 25.1 µs."],
  ["1.9×", "", "Gateway API throughput of the next gateway: 38,501 vs 20,225 req/s (kgateway)."],
];

const PROOF = [
  ["h2spec: 146 of 146 HTTP/2 conformance tests", "https://github.com/bohdaq/rust-web-server/blob/main/ci/e2e/h2spec.sh"],
  ["Gateway API v1.6.3: all five Gateway profiles, core and extended", "https://github.com/kubernetes-sigs/gateway-api/pull/5430"],
  ["ACME tested end to end against Pebble, Let's Encrypt's test CA", "https://github.com/bohdaq/rust-web-server/blob/main/ci/e2e/acme_pebble.sh"],
  ["Every parser of untrusted input is fuzzed", "https://github.com/bohdaq/rust-web-server/tree/main/fuzz"],
  ["Releases carry build attestations and a cosign signature", "/docs/getting-started/installation/#verifying-a-release"],
  ["MSRV pinned at Rust 1.99, MIT licensed", "https://github.com/bohdaq/rust-web-server/blob/main/LICENSE"],
];

const LIBRARY = `\`\`\`rust title="src/main.rs"
use rust_web_server::prelude::*;

fn hello(_req: &Request, _params: &PathParams, _conn: &ConnectionInfo, _state: &()) -> Response {
    Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "Hello, world!")
}

#[tokio::main]
async fn main() {
    let app = routes! {
        App::with_state(()),
        GET "/hello" => hello,
    };
    let (listener, pool) = Server::setup().unwrap();
    tokio::join!(
        Server::run_tls(listener, pool, app.clone()),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}
\`\`\``;

export const FAQ = [
  ["How is rws different from nginx or Caddy?", "One binary covers static files, certificates, the reverse proxy and a Kubernetes gateway, configured in TOML. When config runs out, the same engine is a Rust library, so you don't switch tools."],
  ["Is it ready for production?", "It runs the HTTP/2 conformance suite, the Gateway API suite and ACME against Pebble on every change, and fuzzes its parsers. It is a younger project than nginx; read the changelog and pin a version."],
  ["Does it run on Windows?", "Yes. There are prebuilt binaries for Windows, Linux (static, any distribution) and macOS, and it can run as a Windows service."],
  ["What does it cost?", "Nothing. rws is MIT licensed."],
  ["How do I upgrade?", "Replace the binary and restart, or reload. Breaking changes are listed in the changelog with a migration guide per major version."],
  ["Does it support HTTP/3?", "Yes, over QUIC on the same port as HTTPS, alongside HTTP/2 and HTTP/1.1."],
  ["Can I run it on Kubernetes?", "Yes, as a plain Deployment or as a Gateway API controller with the Helm chart."],
  ["Can my AI coding agent use these docs?", "Yes. llms.txt and llms-full.txt cover every page, and an agent skill pack lives at /agents/."],
  ["What if I outgrow the config file?", "Use the rust-web-server crate: the config file's features are library types you can compose in code."],
  ["How do I remove it?", "Delete the binary and the config file. rws keeps nothing else, except certificates under .rws/acme/ if you used ACME."],
];

const TAGLINE = ["One binary serves your files, gets your certificates", "and routes your traffic. One file tells it how."];

// Real output of `rws` 18.8.2 in a folder, condensed (the startup help text is left out).
const TERMINAL = [
  { cmd: "cd my-site && rws" },
  { out: "Rust Web Server" },
  { out: "Version:           18.8.2" },
  { out: "No TLS certificate configured — serving plain HTTP/1.1." },
  { out: "Started 8 per-core runtimes on 0.0.0.0:7878.", accent: true },
  { cmd: "curl -I http://localhost:7878" },
  { out: "HTTP/1.1 200 OK", accent: true },
  { out: "Accept-Ranges: bytes" },
];

async function code(md) {
  return unsafeHTML((await renderMarkdown(md)).html);
}

async function tabGroup(label, items, id) {
  const panels = await Promise.all(items.map((item) => code(item.code)));
  return html`
    <div data-tab-group id=${id}>
      <lit-material-tabs aria-label=${label}>
        ${items.map((item) => html`<lit-material-tab data-os=${item.os ?? ""}>${item.tab}</lit-material-tab>`)}
      </lit-material-tabs>
      ${items.map(
        (item, i) => html`
          <div data-tab-panel class="tab-panel">
            ${panels[i]}
            <p class="tab-note">${item.result ?? item.note}${item.link ? html` <a href=${item.link[0]}>${item.link[1]}</a>.` : ""}</p>
          </div>`,
      )}
    </div>`;
}

async function installAction(id) {
  return html`
    <div class="install">
      <button class="button-filled" type="button" data-install-toggle aria-expanded="false" aria-controls=${id}>
        ${unsafeHTML(icon("download-simple"))} Install rws
      </button>
      <a class="text-link" href="/docs/getting-started/quick-start/">Read the docs ${unsafeHTML(icon("arrow-right"))}</a>
    </div>
    <div class="install-panel" id=${id} hidden>${await tabGroup("Install rws", INSTALL, `${id}-tabs`)}</div>`;
}

export async function render() {
  const [heroInstall, finalInstall, modes, library] = await Promise.all([
    installAction("install-hero"),
    installAction("install-final"),
    tabGroup("Configuration examples", MODES, "modes"),
    code(LIBRARY),
  ]);
  return html`
    ${header(meta.path)}
    <main id="main">
      <section class="hero container">
        <div class="hero-text">
          <h1 class="hero-heading">The web server that gets <br />its own certificates <br />and routes your apps</h1>
          <p class="lead">Static files, automatic Let's Encrypt certificates, HTTP/3 and a load balancing reverse proxy, configured in one TOML file. When you need code, the same engine is a Rust crate.</p>
          ${heroInstall}
          <p class="proof-line">Gateway API v1.6.3 conformant · h2spec 146 of 146 · MIT licensed</p>
        </div>
        <div class="terminal" role="img" aria-label="A terminal: running rws in a folder starts a server on port 7878, and curl gets HTTP 200 OK.">
          <div class="terminal-bar" aria-hidden="true"><span></span><span></span><span></span></div>
          <pre aria-hidden="true" data-terminal=${JSON.stringify(TERMINAL)}>${TERMINAL.map((l) =>
            l.cmd ? html`<span class="t-line"><span class="t-prompt">$ </span>${l.cmd}</span>` : html`<span class=${l.accent ? "t-line t-accent" : "t-line t-out"}>${l.out}</span>`,
          )}</pre>
        </div>
      </section>

      <section class="section container reveal" aria-labelledby="modes-title">
        <h2 id="modes-title" class="section-title">Three jobs, three short configs</h2>
        ${modes}
      </section>

      <section class="tagline container" aria-label="What rws does">
        <p class="tagline-text">${TAGLINE.map(
          (line) => html`<span class="tagline-line">${line.split(" ").map((w) => html`<span class="word">${w}</span> `)}</span>`,
        )}</p>
      </section>

      <section class="section container reveal" aria-labelledby="box-title">
        <h2 id="box-title" class="section-title">What's in the box</h2>
        <div class="tiles">
          ${TILES.map(
            ([name, title, body, href]) => html`
              <a class="tile" href=${href}>
                <span class="tile-icon">${unsafeHTML(icon(name))}</span>
                <h3>${title}</h3>
                <p>${body}</p>
              </a>`,
          )}
        </div>
      </section>

      <section class="section container reveal" aria-labelledby="how-title">
        <h2 id="how-title" class="section-title">How it works</h2>
        <ol class="steps">
          ${STEPS.map(([title, body], i) => html`<li><span class="step-number">${i + 1}</span><h3>${title}</h3><p>${body}</p></li>`)}
        </ol>
      </section>

      <section class="section container reveal" aria-labelledby="perf-title">
        <h2 id="perf-title" class="section-title">Fast where it counts</h2>
        <p class="section-lead">Same 4 CPU runner, same load: HTTP/1.1, 64 connections, 20 seconds per scenario.</p>
        <div class="stats">
          ${STATS.map(([n, unit, caption]) => html`<div class="stat"><p class="stat-number">${n}<span>${unit}</span></p><p class="stat-caption">${caption}</p></div>`)}
        </div>
        <a class="text-link" href="/benchmarks/">See every scenario and how to reproduce it ${unsafeHTML(icon("arrow-right"))}</a>
      </section>

      <section class="section container reveal" aria-labelledby="open-title">
        <h2 id="open-title" class="section-title">Built in the open</h2>
        <ul class="proof-list">
          ${PROOF.map(([text, href]) => html`<li>${unsafeHTML(icon("check-circle"))}<a href=${href}>${text}</a></li>`)}
        </ul>
      </section>

      <section class="section container reveal library" aria-labelledby="lib-title">
        <div>
          <h2 id="lib-title" class="section-title">When config isn't enough, it's a crate</h2>
          <p class="section-lead">The same engine is <code>rust-web-server</code> on crates.io: routes with typed extractors, one <code>Middleware</code> trait, an MCP server, OAuth, OIDC and SAML, an async ORM and background jobs, without hyper, Tower or Axum underneath.</p>
          <a class="text-link" href="/docs/building-apps/overview/">Build an app ${unsafeHTML(icon("arrow-right"))}</a>
        </div>
        <div>${library}</div>
      </section>

      <section class="section container reveal" aria-labelledby="faq-title">
        <h2 id="faq-title" class="section-title">Questions</h2>
        <div class="faq">
          ${FAQ.map(([q, a]) => html`<details><summary>${q}${unsafeHTML(icon("plus"))}</summary><p>${a}</p></details>`)}
        </div>
      </section>

      <section class="final-cta container reveal" aria-labelledby="final-title">
        <h2 id="final-title" class="section-title">Run it in one minute</h2>
        ${finalInstall}
        <p class="proof-line">MIT licensed. One binary. Remove it by deleting one file.</p>
      </section>
    </main>
    ${footer()}`;
}

// schema.org: the FAQ for answer engines, and the software itself.
export function structuredData(version) {
  return [
    {
      "@context": "https://schema.org",
      "@type": "SoftwareApplication",
      name: "rust-web-server (rws)",
      applicationCategory: "DeveloperApplication",
      operatingSystem: "Linux, macOS, Windows",
      softwareVersion: version,
      license: "https://opensource.org/licenses/MIT",
      url: "https://rws8.tech/",
      offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    },
    {
      "@context": "https://schema.org",
      "@type": "FAQPage",
      mainEntity: FAQ.map(([q, a]) => ({ "@type": "Question", name: q, acceptedAnswer: { "@type": "Answer", text: a } })),
    },
  ];
}
