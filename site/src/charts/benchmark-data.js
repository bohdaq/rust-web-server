// Benchmark numbers, read from the docs pages that publish them, so the
// charts and the tables can never disagree. Prerender only.
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Every GFM table in a Markdown text, as arrays of trimmed cells.
export function markdownTables(text) {
  const tables = [];
  let current = null;
  for (const line of text.split("\n")) {
    if (/^\|.*\|\s*$/.test(line)) {
      const cells = line.trim().slice(1, -1).split("|").map((c) => c.trim());
      if (cells.every((c) => /^:?-{3,}:?$/.test(c))) continue;
      (current ??= []).push(cells);
    } else if (current) {
      tables.push(current);
      current = null;
    }
  }
  if (current) tables.push(current);
  return tables;
}

// "**38 501** · 5.8" → 38501; "96,670" → 96670; "—¹" → null.
export function leadingNumber(cell) {
  const m = cell.replace(/\*\*/g, "").match(/^[\d\s,.]+/);
  if (!m) return null;
  const n = Number(m[0].replace(/[\s,]/g, ""));
  return Number.isFinite(n) && m[0].trim() ? n : null;
}

// docs/reference/benchmarks.md: | Scenario | Server | Requests/s | p50 | p99 | CPU/request | Peak memory |,
// with the scenario cell left blank on its second and third rows.
export function webServerScenarios(docsRoot) {
  const text = readFileSync(join(docsRoot, "reference", "benchmarks.md"), "utf8");
  const scenarios = [];
  for (const table of markdownTables(text)) {
    const [head, ...rows] = table;
    if (head[0] !== "Scenario" || head[1] !== "Server") continue;
    let current;
    for (const r of rows) {
      if (r[0]) scenarios.push((current = { name: r[0], servers: [] }));
      current.servers.push({ name: r[1], rps: leadingNumber(r[2]), p99: leadingNumber(r[4]), cpu: leadingNumber(r[5]), memory: leadingNumber(r[6]) });
    }
  }
  return scenarios;
}

// docs/deployment/kubernetes-gateway-api.md, "## Benchmarks": a requests/s ·
// p99 table, then a CPU µs per request table, one column per implementation.
export function gatewayScenarios(docsRoot) {
  const text = readFileSync(join(docsRoot, "deployment", "kubernetes-gateway-api.md"), "utf8");
  const section = text.slice(text.indexOf("## Benchmarks"));
  const [throughput, cpu] = markdownTables(section);
  const names = throughput[0].slice(1);
  return throughput.slice(1).map((row, i) => ({
    name: row[0],
    servers: names.map((name, j) => ({
      name,
      rps: leadingNumber(row[j + 1]),
      p99: leadingNumber(row[j + 1].split("·")[1] ?? ""),
      cpu: leadingNumber(cpu[i + 1][j + 1]),
    })),
  }));
}
