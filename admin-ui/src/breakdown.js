// Response times and "where requests go" for the Overview, from GET /breakdown.
// The server's numbers only grow, so the difference of two reads is what
// happened between them.

const SCHEMA = 8; // native buckets: bucket i covers (2^((i-1)/8), 2^(i/8)] seconds

const minus = (a, b) => Math.max(0, (a ?? 0) - (b ?? 0));

/**
 * What happened between two /breakdown answers (`base` null: since the
 * server started): the request duration buckets, requests per host
 * (busiest first) and per upstream.
 */
export function between(latest, base) {
  const d = latest.duration, b = base?.duration;
  const buckets = new Map();
  d.buckets.forEach((n, i) => n && buckets.set(d.offset + i, n));
  b?.buckets.forEach((n, i) => {
    const at = b.offset + i, left = minus(buckets.get(at), n);
    if (left) buckets.set(at, left);
    else buckets.delete(at);
  });
  const was = new Map((base?.hosts ?? []).map((h) => [h.host, h]));
  const hosts = latest.hosts
    .map((h) => ({ host: h.host, requests: minus(h.requests, was.get(h.host)?.requests), errors: minus(h.errors, was.get(h.host)?.errors) }))
    .filter((h) => h.requests > 0)
    .sort((x, y) => y.requests - x.requests || x.host.localeCompare(y.host));
  const before = new Map((base?.upstreams ?? []).map((u) => [u.name, u]));
  const upstreams = latest.upstreams.map((u) => {
    const old = before.get(u.name);
    return {
      name: u.name,
      requests: minus(u.requests, old?.requests),
      errors: minus(u.errors, old?.errors),
      buckets: u.duration.buckets.map((n, i) => minus(n, old?.duration.buckets[i])),
    };
  });
  return {
    count: minus(d.count, b?.count),
    zero: minus(d.zero, b?.zero),
    buckets: [...buckets].sort((x, y) => x[0] - y[0]),
    hosts,
    otherHosts: { requests: minus(latest.hosts_other.requests, base?.hosts_other.requests), errors: minus(latest.hosts_other.errors, base?.hosts_other.errors) },
    upstreams,
  };
}

/** The response time `q` (0 to 1) of a window's requests were at or under, in seconds; null without requests. */
export function quantile(win, q) {
  const total = win.zero + win.buckets.reduce((n, [, c]) => n + c, 0);
  if (!total) return null;
  const rank = Math.max(1, Math.ceil(q * total));
  let seen = win.zero;
  if (seen >= rank) return 0;
  for (const [index, n] of win.buckets) {
    if (seen + n >= rank) {
      const lower = 2 ** ((index - 1) / SCHEMA), upper = 2 ** (index / SCHEMA);
      return lower + (upper - lower) * ((rank - seen) / n);
    }
    seen += n;
  }
  return null;
}

/**
 * The smallest of `bounds` (seconds) that `q` of an upstream's requests
 * stayed under, from its cumulative buckets; null when they took longer
 * than the last bound, undefined without requests.
 */
export function under(upstream, bounds, q) {
  if (!upstream.requests) return undefined;
  const at = upstream.buckets.findIndex((n) => n >= q * upstream.requests);
  return at < 0 ? null : bounds[at];
}

/** A response time for people: "0.4 ms", "12 ms", "1.3 s". */
export function latency(secs) {
  const ms = secs * 1000;
  if (ms < 0.05) return "under 0.1 ms";
  const short = (v, unit) => `${v.toFixed(1).replace(/\.0$/, "")} ${unit}`;
  if (ms < 9.95) return short(ms, "ms");
  if (ms < 999.5) return `${Math.round(ms)} ms`;
  if (secs < 99.95) return short(secs, "s");
  return `${Math.round(secs / 60)} min`;
}
