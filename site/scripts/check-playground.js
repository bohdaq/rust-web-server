// Runs `rws --check --format=json` on every config the playground can build
// (each feature on and off, with each strategy and challenge), so the builder
// never emits a key or value the binary rejects. Usage:
//   node scripts/check-playground.js <path to rws>
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { generate } from "../src/playground/generate.js";

const rws = process.argv[2];
if (!rws) throw new Error("usage: node scripts/check-playground.js <rws binary>");
const dir = mkdtempSync(join(tmpdir(), "rws-playground-"));
const flags = ["https", "proxy", "cors", "rateLimit", "routeRate", "auth", "watch", "staging"];
let checked = 0;
const failures = [];
for (let mask = 0; mask < 1 << flags.length; mask++) {
  const state = Object.fromEntries(flags.map((f, i) => [f, Boolean(mask & (1 << i))]));
  for (const extra of [{}, { strategy: "round_robin", challenge: "tls-alpn-01", logFormat: "combined", host: "api.example.com", port: 8080 }, { strategy: "ip_hash", healthPath: "", logFormat: "off" }]) {
    const { toml, errors } = generate({ ...state, ...extra });
    if (Object.keys(errors).length) throw new Error(`defaults produced errors: ${JSON.stringify(errors)}`);
    const file = join(dir, "rws.config.toml");
    writeFileSync(file, toml);
    let out;
    try {
      out = execFileSync(rws, [`--check=${file}`, "--format=json"], { encoding: "utf8", env: { ...process.env, API_TOKEN: "x" } });
    } catch (e) {
      out = e.stdout;
    }
    const report = JSON.parse(out.trim().split("\n").pop());
    checked++;
    if (!report.ok || report.errors.length) failures.push({ state: { ...state, ...extra }, toml, errors: report.errors, warnings: report.warnings });
  }
}
rmSync(dir, { recursive: true, force: true });
console.log(`${checked} configs checked, ${failures.length} rejected`);
for (const f of failures.slice(0, 5)) console.log(JSON.stringify(f, null, 2));
process.exit(failures.length ? 1 : 0);
