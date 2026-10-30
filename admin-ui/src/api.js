// The admin API client. The token lives in sessionStorage: it goes with the
// tab, never to disk. Storage can be unavailable (private windows, blocked
// site data): then it lasts for this page only.
const KEY = "rws-admin-token";
let memory = "";

export function token() {
  try {
    return sessionStorage.getItem(KEY) ?? memory;
  } catch {
    return memory;
  }
}

export function setToken(value) {
  memory = value;
  try {
    if (value) sessionStorage.setItem(KEY, value);
    else sessionStorage.removeItem(KEY);
  } catch {
    // Unavailable: kept in memory only.
  }
}

export class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

/** `method path` against /admin/api/v1, JSON in and out. */
export async function api(path, { method = "GET", body } = {}) {
  const response = await fetch(`/admin/api/v1${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token()}`,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
    cache: "no-store",
  });
  const text = await response.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = null;
  }
  if (!response.ok) throw new ApiError(response.status, data?.error ?? `${response.status} ${response.statusText}`);
  return data;
}

/**
 * Follow /logs/stream: `onLine(line)` per event until `signal` aborts.
 * EventSource can't send the token, so this reads the event stream itself.
 */
export async function followLogs(onLine, signal) {
  const response = await fetch("/admin/api/v1/logs/stream", {
    headers: { Authorization: `Bearer ${token()}` },
    signal,
    cache: "no-store",
  });
  if (!response.ok || !response.body) throw new ApiError(response.status, `${response.status} ${response.statusText}`);
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    buffer += value;
    let end;
    while ((end = buffer.indexOf("\n\n")) >= 0) {
      const event = buffer.slice(0, end);
      buffer = buffer.slice(end + 2);
      if (event.split("\n").some((l) => l.startsWith("event:"))) continue; // pings
      const data = event.split("\n").filter((l) => l.startsWith("data:")).map((l) => l.slice(5).replace(/^ /, "")).join("\n");
      if (data) onLine(data);
    }
  }
}
