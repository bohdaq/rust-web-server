// The config builder's one source of truth: form state → rws.config.toml,
// the command to run, and per-field errors. Pure, so the browser page and
// site/scripts/check-playground.js (which runs `rws --check` on every
// combination) use the same code. Only keys from
// docs/src/content/docs/configuration/config-file.md.

export const DEFAULTS = {
  port: 7878,
  logFormat: "json",
  watch: false,
  https: false,
  domains: "example.com, www.example.com",
  email: "you@example.com",
  staging: true,
  challenge: "http-01",
  proxy: false,
  backends: "10.0.0.10:8080\n10.0.0.11:8080",
  strategy: "least_connections",
  healthPath: "/healthz",
  host: "",
  path: "/api/*",
  routeRate: false,
  routeMax: 500,
  routeWindow: 60,
  auth: false,
  tokenEnv: "API_TOKEN",
  cors: false,
  corsOrigins: "https://app.example.com",
  rateLimit: false,
  maxRequests: 1000,
  windowSecs: 60,
};

const q = (s) => JSON.stringify(s);
const list = (text, sep = /[\s,]+/) => text.split(sep).map((s) => s.trim()).filter(Boolean);
const DOMAIN = /^(\*\.)?([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/i;
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const BACKEND = /^(https?:\/\/)?[a-z0-9.-]+(:\d{1,5})?$/i;
const ENV = /^[A-Z_][A-Z0-9_]*$/;

export function generate(input) {
  const s = { ...DEFAULTS, ...input };
  const errors = {};
  const lines = [];
  const domains = list(s.domains);
  const backends = list(s.backends, /[\n,]+/);

  // Top level. With HTTPS the server listens on 443 and 80 redirects.
  if (s.https) {
    lines.push("port               = 443", "http_redirect_port = 80");
  } else {
    const port = Number(s.port);
    if (!Number.isInteger(port) || port < 1 || port > 65535) errors.port = "Use a port from 1 to 65535.";
    else if (port !== 7878) lines.push(`port = ${port}`);
  }
  if (s.logFormat !== "json") lines.push(`log_format = ${q(s.logFormat)}`);
  if (s.watch) lines.push("watch = true");

  if (s.https) {
    if (!domains.length) errors.domains = "Add at least one domain.";
    const bad = domains.find((d) => !DOMAIN.test(d));
    if (bad) errors.domains = `"${bad}" is not a domain name.`;
    if (domains.some((d) => d.startsWith("*.")) && s.challenge !== "dns-01") errors.domains = "Wildcard names need the DNS-01 challenge, which needs a DNS provider: see the ACME docs.";
    if (!EMAIL.test(s.email)) errors.email = "Enter an email address, like ops@example.com.";
    lines.push("", "[acme]", `domains = [${domains.map(q).join(", ")}]`, `email   = ${q(s.email.trim())}`);
    if (s.staging) lines.push("staging = true");
    if (s.challenge !== "http-01") lines.push(`challenge = ${q(s.challenge)}`);
  }

  if (s.cors) {
    const origins = list(s.corsOrigins);
    const bad = origins.find((o) => !/^https?:\/\/[^\s/]+$/.test(o));
    if (!origins.length || bad) errors.corsOrigins = bad ? `"${bad}" is not an origin like https://app.example.com.` : "Add at least one origin.";
    lines.push("", "[cors]", `allow_origins = ${q(origins.join(","))}`);
  }

  if (s.rateLimit) {
    if (!(Number(s.maxRequests) > 0)) errors.maxRequests = "Use a number above 0.";
    if (!(Number(s.windowSecs) > 0)) errors.windowSecs = "Use a number of seconds above 0.";
    lines.push("", "[rate_limit]", `max_requests = ${Number(s.maxRequests)}`, `window_secs  = ${Number(s.windowSecs)}`);
  }

  if (s.proxy) {
    if (!backends.length) errors.backends = "Add at least one backend, like 10.0.0.10:8080.";
    const bad = backends.find((b) => !BACKEND.test(b));
    if (bad) errors.backends = `"${bad}" is not host:port or a URL.`;
    lines.push("", "[[upstream]]", 'name     = "app"', `backends = [${backends.map(q).join(", ")}]`, `strategy = ${q(s.strategy)}`);
    if (s.healthPath.trim()) {
      if (!s.healthPath.startsWith("/")) errors.healthPath = "Start the path with /.";
      lines.push("", "  [upstream.health_check]", `  path = ${q(s.healthPath.trim())}`);
    }
    lines.push("", "[[route]]", '  name = "app"', "", "  [route.match]");
    if (s.host.trim()) {
      if (!DOMAIN.test(s.host.trim())) errors.host = `"${s.host.trim()}" is not a host name.`;
      lines.push(`  host = ${q(s.host.trim())}`);
    }
    if (!s.path.startsWith("/")) errors.path = "Start the path with /.";
    lines.push(`  path = ${q(s.path || "/*")}`, "", "  [route.action]", '  type     = "proxy"', '  upstream = "app"');
    if (s.routeRate || s.auth) {
      lines.push("", "  [route.middleware]");
      if (s.routeRate) {
        if (!(Number(s.routeMax) > 0) || !(Number(s.routeWindow) > 0)) errors.routeMax = "Use numbers above 0.";
        lines.push(`  rate_limit = { max_requests = ${Number(s.routeMax)}, window_secs = ${Number(s.routeWindow)} }`);
      }
      if (s.auth) {
        if (!ENV.test(s.tokenEnv)) errors.tokenEnv = "Use an environment variable name like API_TOKEN.";
        lines.push(`  auth       = { type = "bearer", token_env = ${q(s.tokenEnv)} }`);
      }
    }
  }

  const body = lines[0] === "" ? lines.slice(1) : lines;
  const toml = body.length
    ? `${body.join("\n")}\n`
    : "# Nothing to set: with no file at all, rws serves this folder on port 7878.\n";
  const env = s.proxy && s.auth ? `${s.tokenEnv}="$(openssl rand -hex 32)" ` : "";
  const command = `rws --check && ${env}rws`;
  return { toml, command, errors };
}
