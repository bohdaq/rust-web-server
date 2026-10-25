---
title: Roadmap
description: Planned features for rust-web-server — each appears as a callout on the relevant docs page.
---

These features are planned. Each appears as a callout on the relevant docs page.

:::caution[Coming Soon]
**Baggage and automatic child spans**

W3C baggage propagation, and spans created automatically for each database query and outbound call. Child spans already work when started by hand with `otel::span` / `otel::client_span` — see [Distributed Tracing](/features/tracing/#child-spans).
:::

:::caution[Coming Soon]
**Admin UI**

`GET /admin` — a browser-based dashboard showing live metrics, current configuration, and a tail of the access log. Requires authentication.
:::

:::caution[Coming Soon]
**Access log rotation**

Built-in log-file rotation (by size or time) so the server can write logs to disk without an external `logrotate` configuration.
:::

---

**WebAssembly guest component** — shipped. `App`/`Router`/middleware now run inside a `wasi:http/proxy` WASM component (Wasmtime, Spin, Fastly Compute) via the separate `rws-wasm-shim` package. See [WASM Guest Component](/deployment/wasm/). Larger follow-up phases (streaming bodies, outbound HTTP from the guest) remain open — tracked in `spec/WASM_SHIM.md` in the repository.

---

Track progress and open issues at [github.com/bohdaq/rust-web-server](https://github.com/bohdaq/rust-web-server/issues).
