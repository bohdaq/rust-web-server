---
title: Roadmap
description: Planned features for rust-web-server — each appears as a callout on the relevant docs page.
---

These features are planned. Each appears as a callout on the relevant docs page.

:::caution[Coming Soon]
**Multi-span distributed tracing**

Child spans, baggage propagation, and per-database-query spans within a single inbound request. Currently `OtelLayer` creates one span per request with no nested structure.
:::

:::caution[Coming Soon]
**Admin UI**

`GET /admin` — a browser-based dashboard showing live metrics, current configuration, and a tail of the access log. Requires authentication.
:::

:::caution[Coming Soon]
**Access log rotation**

Built-in log-file rotation (by size or time) so the server can write logs to disk without an external `logrotate` configuration.
:::

:::caution[Coming Soon]
**WebAssembly compile target**

Support for `wasm32-wasi` so the server can run inside a WebAssembly runtime such as Wasmtime or WasmEdge.
:::

---

Track progress and open issues at [github.com/bohdaq/rust-web-server](https://github.com/bohdaq/rust-web-server/issues).
